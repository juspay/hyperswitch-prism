use std::collections::HashMap;

use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    consts,
    crypto::{GenerateDigest, Sha256},
    types::{AmountConvertor, MinorUnit, StringMajorUnit, StringMinorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        ConnectorMandateReferenceId, MandateReference, MandateReferenceId, NetworkMandateIdRef,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, RefundFlowData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors,
    payment_address::Address,
    payment_method_data::{
        Card, CardDetailsForNetworkTransactionId, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::worldpayraft::WorldpayraftRouterData, utils::truncate_secret_string};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Card type identifier as received in PaymentMethodData (case-insensitive comparison via eq_ignore_ascii_case).
pub(super) const CARD_TYPE_DEBIT: &str = "debit";

/// Connector name used in `IntegrationError::NotSupported`.
const CONNECTOR_NAME: &str = "worldpayraft";

/// Maximum lengths of the RAFT request fields this module fills from free text
/// (tech spec: Billing, shipping and cardholder addresses; Dynamic / soft descriptor;
/// Merchant reference id / order id; Level 2 and Level 3 data).
const MAX_AVS_ZIP: usize = 9;
const MAX_AVS_ADDRESS: usize = 20;
const MAX_ADDRESS_LINE: usize = 40;
const MAX_ADDRESS_CITY: usize = 18;
const MAX_ADDRESS_STATE: usize = 2;
const MAX_ADDRESS_ZIP: usize = 9;
const MAX_CARDHOLDER_FIRST_NAME: usize = 25;
const MAX_CARDHOLDER_LAST_NAME: usize = 30;
const MAX_SD_MERCHANT_NAME: usize = 25;
const MAX_SD_MERCHANT_CITY: usize = 13;
const MAX_CUSTOMER_SERVICE_PHONE: usize = 16;
const MAX_CORRELATION_ID: usize = 25;
const MAX_USER_DATA_1: usize = 35;
const MAX_REF_INVOICE_NUMBER: usize = 20;
const MAX_ECOMMERCE_ORDER_NUM: usize = 13;
const MAX_DS_TRANSACTION_ID: usize = 36;
const MAX_LEVEL3_ITEMS: usize = 25;
const MAX_ITEM_DESCRIPTION: usize = 35;
const MAX_PRODUCT_CODE: usize = 15;
const MAX_UNIT_OF_MEASURE: usize = 12;

/// `ProcFlagsIndicators.EventNotificationIndicator` sent on every original payment message
/// (Authorize, SetupMandate, RepeatPayment). The flag only asks Worldpay to notify: delivery also
/// needs the merchant's Event Notifications subscription and an endpoint registered with the
/// Relationship Manager, so a merchant without them receives nothing and loses nothing. Without
/// the flag no payment webhook can ever fire (tech spec: IncomingWebhook; Known defects in the
/// current UCS implementation, "Event opt-in" row). A per-merchant switch would need a new
/// `ConnectorSpecificConfig` field, a contract change this integration does not make.
const EVENT_NOTIFICATION_OPT_IN: WorldpayraftYesNo = WorldpayraftYesNo::Yes;

/// `APITransactionID` is a fixed 16-digit numeric field (tech spec: Merchant reference id / order id).
const API_TRANSACTION_ID_MODULUS: u64 = 10_000_000_000_000_000;

// -----------------------------------------------------------------------------
// Closed-set wire enums (each value is the exact documented wire spelling)
// -----------------------------------------------------------------------------

/// `TerminalData.EntryMode`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftEntryMode {
    /// Electronic Commerce / In-application 3D secure processing.
    #[serde(rename = "E-COMM")]
    ECommerce,
}

/// `TerminalData.TerminalType`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftTerminalType {
    #[serde(rename = "INTERNET")]
    Internet,
}

/// `TerminalData.POSConditionCode`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftPosConditionCode {
    /// 59 - Electronic Commerce Transaction.
    #[serde(rename = "59")]
    ElectronicCommerce,
    /// 51 - Verification-Only Request, the transaction amount must be zero.
    #[serde(rename = "51")]
    VerificationOnly,
}

/// `TerminalData.TerminalEntryCap`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftTerminalEntryCap {
    /// 0 - Unspecified.
    #[serde(rename = "0")]
    Unspecified,
}

/// `TerminalData.POSEnvironment` (credit only) — the credential-on-file indicator.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftPosEnvironment {
    #[serde(rename = "C")]
    CredentialOnFile,
    #[serde(rename = "R")]
    Recurring,
    #[serde(rename = "I")]
    Installment,
}

/// `E-commerceData.E-commerceIndicator`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftEcommerceIndicator {
    /// 05 - fully authenticated (Verified by Visa / SecureCode with AAV / Discover with CAVV).
    #[serde(rename = "05")]
    Authenticated,
    /// 06 - authentication attempted.
    #[serde(rename = "06")]
    Attempted,
    /// 07 - eCommerce, neither Verified by Visa nor MasterCard SecureCode (NOT authenticated).
    #[serde(rename = "07")]
    NonAuthenticated,
    /// 10 - first transaction of a recurring payment series.
    #[serde(rename = "10")]
    RecurringFirst,
    /// 02 - Recurring Transaction.
    #[serde(rename = "02")]
    Recurring,
    /// 03 - Installment Payment.
    #[serde(rename = "03")]
    Installment,
}

/// Y/N flag used by `ProcFlagsIndicators` and `EncryptionTokenData`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldpayraftYesNo {
    #[serde(rename = "Y")]
    Yes,
    #[serde(rename = "N")]
    No,
}

/// `AuthorizationType` (only FP and RV exist).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftAuthorizationType {
    #[serde(rename = "FP")]
    ForcePost,
    #[serde(rename = "RV")]
    Reversal,
}

/// `ReversalAdviceReasonCd`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftReversalAdviceReasonCode {
    #[serde(rename = "000")]
    NormalReversal,
    #[serde(rename = "006")]
    CustomerCancel,
}

/// `CardVerificationData.Cvv2Cvc2CIDIndicator`: "1" means the value IS present.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftCvvIndicator {
    /// 0 - The CVV2/CVC2/CID value was bypassed or not given.
    #[serde(rename = "0")]
    Bypassed,
    /// 1 - The CVV2/CVC2/CID value is present.
    #[serde(rename = "1")]
    Present,
}

/// `E-commerceData.3DSecureProgramProtocol`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftThreeDsProtocol {
    #[serde(rename = "1")]
    V1,
    #[serde(rename = "2")]
    V2,
}

/// Internal routing: which RAFT API (credit or debit) carries the payment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorldpayraftCardKind {
    Credit,
    Debit,
}

/// Internal: which original message the payment used.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorldpayraftOperation {
    /// Sale: `/credit/purchase` or `/debit/purchase`.
    Purchase,
    /// Pre-authorization: `/credit/authorization` or `/debit/preauth`.
    Authorization,
    /// Zero-amount account verification (POSConditionCode 51) on the authorization endpoint.
    Verification,
}

/// Card brand, persisted so follow-ups know which brand-specific object to send.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorldpayraftCardBrand {
    Visa,
    Mastercard,
    Amex,
    Discover,
    Other,
}

impl From<&common_enums::CardNetwork> for WorldpayraftCardBrand {
    fn from(network: &common_enums::CardNetwork) -> Self {
        match network {
            common_enums::CardNetwork::Visa => Self::Visa,
            common_enums::CardNetwork::Mastercard => Self::Mastercard,
            common_enums::CardNetwork::AmericanExpress => Self::Amex,
            common_enums::CardNetwork::Discover => Self::Discover,
            _ => Self::Other,
        }
    }
}

// =============================================================================
// AUTH TYPE
// =============================================================================

/// Auth credentials for Worldpay Native RAFT.
///
/// - `license`     → sent in the `Authorization: VANTIV license="<license>"` header
/// - `merchant_id` → sent as `WorldPayMerchantID` in every request body
#[derive(Debug, Clone)]
pub struct WorldpayraftAuthType {
    pub license: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for WorldpayraftAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Worldpayraft {
                license,
                merchant_id,
                ..
            } => Ok(Self {
                license: license.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Worldpay RAFT requires ConnectorSpecificConfig::Worldpayraft with license and merchant_id".to_string(),
                        ),
                        suggested_action: Some(
                            "configure the worldpayraft connector with its license and WorldPayMerchantID".to_string(),
                        ),
                        ..Default::default()
                    }
                }
            )),
        }
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================

/// Body of a non-2xx RAFT response.
///
/// Business failures are HTTP 200 and wrapped (`{"<op>response": {...}}`); licence and transport
/// failures (401/403/404/500) carry only `{"fault": {...}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftErrorBody {
    Wrapped(HashMap<String, WorldpayraftResponseInner>),
    Fault { fault: serde_json::Value },
}

/// Published meaning of each `ReturnCode` (tech spec: `ReturnCode` — operation level).
pub(super) fn return_code_meaning(code: &WorldpayraftReturnCode) -> Option<&'static str> {
    match code {
        WorldpayraftReturnCode::Successful => Some("Successful"),
        WorldpayraftReturnCode::EditError => Some("Edit error on input"),
        WorldpayraftReturnCode::LogicError => Some("Logic error"),
        WorldpayraftReturnCode::SystemIssue => Some("System issue"),
        WorldpayraftReturnCode::Unknown => None,
    }
}

/// Published meaning of each `ResponseCode` (tech spec: `ResponseCode` — all 101 published values).
pub(super) fn response_code_meaning(code: &str) -> Option<&'static str> {
    match code {
        "000" => Some("APPROVE"),
        "001" => Some("REFER TO ISSUER"),
        "002" => Some("VOID UNSUCCESSFUL"),
        "003" => Some("HONOR WITH ID"),
        "004" => Some("CARD EXPIRED"),
        "005" => Some("DO NOT HONOR"),
        "006" => Some("PIN TRY LIMIT EXCEEDED"),
        "007" => Some("INVALID MERCHANT ID"),
        "008" => Some("INVALID AMOUNT"),
        "009" => Some("INVALID ACCOUNT"),
        "010" => Some("PARTIAL APPROVAL"),
        "011" => Some("INVALID TRANSACTION"),
        "012" => Some("INVALID PIN"),
        "013" => Some("INVALID CARD SECURITY CODE"),
        "014" => Some("NETWORK UNAVAILABLE"),
        "015" => Some("INVALID CURRENCY CODE"),
        "016" => Some("DECLINE - PICK UP CARD"),
        "017" => Some("DECLINE - PICK UP CARD - FRAUD"),
        "018" => Some("INVALID CARD NUMBER"),
        "019" => Some("SUSPECTED FRAUD - CALL CENTER"),
        "020" => Some("RESTRICTED CARD"),
        "021" => Some("DECLINE - PICK UP LOST CARD"),
        "022" => Some("DECLINE - PICK UP STOLEN CARD"),
        "023" => Some("DECLINED - OVER LIMIT - ACCOUNT"),
        "024" => Some("INVALID TERMINAL ID"),
        "025" => Some("DO NOT HONOR - SUSPECTED FRAUD"),
        "026" => Some("EXCEEDS WITHDRAWAL LIMIT"),
        "027" => Some("NO DATA AVAILABLE"),
        "028" => Some("SECURITY VIOLATION"),
        "029" => Some("ORIGINAL AMOUNT INCORRECT"),
        "030" => Some("FORMAT ERROR"),
        "031" => Some("EXCEEDS WITHDRAWAL COUNT LIMIT"),
        "032" => Some("HARD CAPTURE"),
        "033" => Some("RESPONSE RECEIVED TOO LATE"),
        "034" => Some("UNABLE TO ROUTE TRANSACTION"),
        "035" => Some("DECLINED - TRANSACTION IN VIOLATION OF LAW"),
        "036" => Some("DUPLICATE REQUEST"),
        "037" => Some("DUPLICATE REVERSAL"),
        "038" => Some("NO SUCH ISSUER"),
        "039" => Some("INSUFFICIENT FUNDS"),
        "040" => Some("EXCEEDS PURCHASE LIMITS"),
        "041" => Some("RE-ENTER"),
        "042" => Some("CALL CENTER"),
        "043" => Some("ENTER DOB AND RE-SEND"),
        "044" => Some("CAN'T CONVERT CHECK"),
        "045" => Some("INVALID DATE"),
        "046" => Some("CRYPTOGRAPHIC ERROR FOUND IN PIN OR CVV"),
        "047" => Some("TIME LIMIT FOR A PRE-AUTH IS TOO LONG"),
        "048" => Some("SYSTEM MALFUNCTION"),
        "049" => Some("PIN MISSING"),
        "050" => Some("SWITCH COMMUNICATION ERROR"),
        "051" => Some("UNABLE TO LOCATE A MATCHING ORIGINAL TRANSACTION"),
        "052" => Some("CARD NOT ACTIVATED YET"),
        "053" => Some("CARD ALREADY ACTIVATED"),
        "054" => Some("VELOCITY: EXCEEDS COUNT"),
        "055" => Some("VELOCITY: EXCEEDS AMOUNT"),
        "056" => Some("VELOCITY: EXCEEDS COUNT AND AMOUNT"),
        "057" => Some("VELOCITY: VELOCITY NEGATIVE"),
        "058" => Some("VELOCITY: VELOCITY FRAUD RECORD"),
        "059" => Some("VELOCITY: NO ZIP CODE MATCH"),
        "060" => Some("CARD ESCHEATED"),
        "061" => Some("MERCHANT DEPLETED"),
        "062" => Some("FRAUD SYSTEM DETECTED UNUSUAL ACTIVITY"),
        "063" => Some("EMV MISSING OR INVALID TAG DATA"),
        "064" => Some("LINE TYPE NOT VALID FOR THIS TERMINAL"),
        "065" => Some("DECRYPTION/TOKENIZATION ERROR"),
        "066" => Some("REGISTRATION EVENT"),
        "067" => Some("APPLICATION TRANSACTION COUNTER ERROR"),
        "068" => Some("CARDHOLDER VERIFICATION FAILURE - TVR"),
        "069" => Some("ERROR IDENTIFYING CHIP APPLICATION"),
        "070" => Some("MAC NOT DETECTED"),
        "071" => Some("INTERNAL MAC PROCESSING ERROR"),
        "072" => Some("INVALID MAC DETECTED"),
        "073" => Some("DECRYPTION NOT POSSIBLE - MERCHANT"),
        "074" => Some("DECRYPTION NOT POSSIBLE - WORLDPAY"),
        "075" => Some("PROBLEM CALLING ENCRYPTION"),
        "076" => Some("MALFORMED MESSAGE RECEIVED"),
        "077" => Some("POSSIBLE DECRYPTION FAILURE"),
        "078" => Some("DETOKENIZATION FAILED"),
        "079" => Some("LOW TOKEN CONVERSION ERROR"),
        "080" => Some("RESERVED"),
        "100" => Some("IDEMPOTENCY DETECTED A DUPLICATE REQUEST BUT THERE WAS A MESSAGE TYPE MISMATCH BETWEEN WHAT IT LOCATED AND WHAT WAS SENT IN."),
        "101" => Some("A REQUEST FOR PINLESS CONVERSION FAILED TO FIND A VALID ROUTING OPTION."),
        "102" => Some("ACCOUNT CLOSED."),
        "103" => Some("TRANSACTION FEE NOT PERMITTED OR INVALID."),
        "104" => Some("CASH BACK REQUEST EXCEEDS ISSUER LIMIT."),
        "105" => Some("UNABLE TO LOCATE PREVIOUS TRANSACTION."),
        "106" => Some("PREVIOUS TRANSACTION LOCATED, BUT DATA INCONSISTENT."),
        "107" => Some("DECLINED FIRST USE OF CARD."),
        "108" => Some("TRANSACTION AMOUNT EXCEEDS PREAUTHORIZED AMOUNT."),
        "109" => Some("STOP PAYMENT ORDER."),
        "110" => Some("EXPIRATION DATE MISMATCH."),
        "111" => Some("STALE DATED TRANSACTION."),
        "112" => Some("INVALID ADDRESS VERIFICATION INFORMATION."),
        "113" => Some("CUTOFF IS IN PROGRESS."),
        "114" => Some("REQUEST IN PROGRESS."),
        "115" => Some("INFORMATION NOT ON FILE."),
        "116" => Some("TOKEN LOOKUP FAILURE."),
        "117" => Some("CARDHOLDER DOES NOT PARTICIPATE IN ATTEMPTED PRODUCT."),
        "118" => Some("SPECIAL CONDITIONS."),
        "550" => Some("TRANSACTION DECLINED BY FRAUDSIGHT."),
        _ => None,
    }
}

/// Builds the `ErrorResponse` for a wrapped RAFT failure (tech spec: Rule 5).
pub(super) fn build_business_error(
    inner: &WorldpayraftResponseInner,
    status_code: u16,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    let response_code = inner
        .response_code
        .as_deref()
        .filter(|code| !code.is_empty());
    // The wire spelling of a published ReturnCode (strum Display); an unpublished one has none.
    let return_code = match inner.return_code {
        WorldpayraftReturnCode::Unknown => None,
        WorldpayraftReturnCode::Successful
        | WorldpayraftReturnCode::EditError
        | WorldpayraftReturnCode::LogicError
        | WorldpayraftReturnCode::SystemIssue => Some(inner.return_code.to_string()),
    };
    let code = response_code
        .map(str::to_string)
        .or(return_code)
        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
    let return_text = inner.return_text.clone().filter(|text| !text.is_empty());
    let response_code_text = response_code.and_then(response_code_meaning);
    let message = return_text
        .clone()
        .or_else(|| response_code_text.map(str::to_string))
        .or_else(|| return_code_meaning(&inner.return_code).map(str::to_string))
        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
    let reason = match &inner.error_information {
        Some(info) => match (&info.field_in_error, &info.error_text) {
            (Some(field), Some(text)) => Some(format!("{field}: {text}")),
            (Some(field), None) => Some(field.clone()),
            (None, Some(text)) => Some(text.clone()),
            (None, None) => inner.reason_code.clone(),
        },
        None => inner.reason_code.clone(),
    };
    ErrorResponse {
        status_code,
        code,
        message,
        reason,
        attempt_status,
        connector_transaction_id: inner.api_transaction_id.clone(),
        network_decline_code: response_code.map(str::to_string),
        network_advice_code: inner
            .mcrd_specific_data
            .as_ref()
            .and_then(|mcrd| mcrd.mastercard_merchant_advice_code.clone()),
        network_error_message: return_text.or_else(|| response_code_text.map(str::to_string)),
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_response: None,
        typed_connector_request: None,
    }
}

// =============================================================================
// SHARED HELPERS
// =============================================================================

/// Current local date-time as `YYYY-MM-DDTHH:mm:ss`, the `LocalDateTime` format of every RAFT
/// message. No `common_utils::date_time::DateFormat` variant has this dashed `T`-separated shape
/// without a fractional part or offset, so it is formatted here. A fresh value is sent on every
/// message, follow-ups included.
pub(super) fn local_date_time() -> String {
    let now = common_utils::date_time::now().assume_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

/// Derives the 16-digit `APITransactionID` from a UCS reference id.
///
/// `APITransactionID` is a fixed 16-character field, it is the idempotency key (a repeat within
/// 40 days replays the first response) and the follow-up matching key. UCS references are longer
/// than 16 characters, and a naive truncation makes two references that share a 16-character
/// prefix collide. This takes the first 8 bytes of the SHA-256 of the reference as a big-endian
/// u64, modulo 10^16, zero-padded to 16 digits. It is deterministic, so a retried request replays
/// instead of charging twice, and the Authorize response can recompute the id it sent.
pub fn derive_api_transaction_id(
    reference: &str,
) -> common_utils::errors::CustomResult<String, common_utils::errors::CryptoError> {
    let digest = Sha256.generate_digest(reference.as_bytes())?;
    let value = digest
        .iter()
        .take(8)
        .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte));
    Ok(format!("{:016}", value % API_TRANSACTION_ID_MODULUS))
}

/// G-Authorize-07: a follow-up (Capture, Void) reuses the original `APITransactionID`, which is
/// exactly 16 ASCII digits (`derive_api_transaction_id`). Any other id is not one this connector
/// issued and is refused before a request is built, never sent on the wire.
fn validate_api_transaction_id(
    connector_transaction_id: &str,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    if connector_transaction_id.len() == 16
        && connector_transaction_id
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(error_stack::report!(errors::IntegrationError::InvalidDataFormat {
            field_name: "connector_transaction_id",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "Worldpay RAFT APITransactionID is 16 digits".to_string(),
                ),
                suggested_action: Some(
                    "send the connector_transaction_id returned by the Worldpay RAFT Authorize response unchanged".to_string(),
                ),
                ..Default::default()
            },
        }))
    }
}

/// First `max` characters of a free-text display value (addresses, names, descriptors).
/// Identifiers are never shortened this way; they are omitted when too long.
fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

/// An identifier is sent only when it fits its field; it is never truncated.
fn fit_identifier(value: &str, max: usize) -> Option<String> {
    (!value.is_empty() && value.chars().count() <= max).then(|| value.to_string())
}

fn is_debit_card<T: PaymentMethodDataTypes>(card: &Card<T>) -> bool {
    card.card_type
        .as_deref()
        .map(|card_type| card_type.eq_ignore_ascii_case(CARD_TYPE_DEBIT))
        .unwrap_or(false)
}

fn card_brand<T: PaymentMethodDataTypes>(card: &Card<T>) -> Option<WorldpayraftCardBrand> {
    card.card_network.as_ref().map(WorldpayraftCardBrand::from)
}

/// Selects the RAFT API (credit/debit) and the original message for an Authorize request.
///
/// Shared by `get_url` and the request builder so the endpoint and the wrapper key cannot
/// disagree: zero amount → verification (authorization endpoint, POSConditionCode 51);
/// automatic capture → purchase; otherwise → authorization.
pub fn select_operation<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> (WorldpayraftCardKind, WorldpayraftOperation) {
    let card_kind = match &request.payment_method_data {
        PaymentMethodData::Card(card) if is_debit_card(card) => WorldpayraftCardKind::Debit,
        _ => WorldpayraftCardKind::Credit,
    };
    let operation = if request.minor_amount == MinorUnit::zero() {
        WorldpayraftOperation::Verification
    } else if request.is_auto_capture() {
        WorldpayraftOperation::Purchase
    } else {
        WorldpayraftOperation::Authorization
    };
    (card_kind, operation)
}

/// URL path of an original payment message.
pub fn original_message_path(
    card_kind: WorldpayraftCardKind,
    operation: WorldpayraftOperation,
) -> &'static str {
    match (card_kind, operation) {
        (WorldpayraftCardKind::Credit, WorldpayraftOperation::Purchase) => "credit/purchase",
        (
            WorldpayraftCardKind::Credit,
            WorldpayraftOperation::Authorization | WorldpayraftOperation::Verification,
        ) => "credit/authorization",
        (WorldpayraftCardKind::Debit, WorldpayraftOperation::Purchase) => "debit/purchase",
        (
            WorldpayraftCardKind::Debit,
            WorldpayraftOperation::Authorization | WorldpayraftOperation::Verification,
        ) => "debit/preauth",
    }
}

// =============================================================================
// CONNECTOR FEATURE DATA (connector_metadata round-trip)
// =============================================================================

/// Per-brand network transaction ids returned on the original message.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldpayraftNetworkIds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visa_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcrd_banknet_ref_num: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcrd_banknet_settle_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amex_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_link_id: Option<String>,
}

/// What follow-up messages (Capture, Void, Refund) need from the original message and cannot
/// read from their own request: the original `APITransactionID`, the credit/debit route, the
/// original amount (`PreauthorizedAmount`) and the card token. Stored as the JSON
/// `connector_metadata` of the Authorize response and read back from `connector_feature_data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WorldpayraftFeatureData {
    pub api_transaction_id: String,
    pub operation: WorldpayraftOperation,
    pub card_kind: WorldpayraftCardKind,
    pub authorized_amount: MinorUnit,
    /// Set by a successful Capture (completion); the refund ceiling after a partial capture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_amount: Option<MinorUnit>,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenized_pan: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_ref_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_network: Option<WorldpayraftCardBrand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_transaction_ids: Option<WorldpayraftNetworkIds>,
}

// =============================================================================
// SHARED REQUEST STRUCTS
// =============================================================================

/// `MiscAmountsBalances` (request). Amounts are `ddddddddd.cc` major-unit strings.
#[derive(Debug, Serialize)]
pub struct WorldpayraftAmounts {
    #[serde(rename = "TransactionAmount")]
    pub transaction_amount: StringMajorUnit,
    #[serde(
        rename = "PreauthorizedAmount",
        skip_serializing_if = "Option::is_none"
    )]
    pub preauthorized_amount: Option<StringMajorUnit>,
    #[serde(rename = "SalesTAXAmount", skip_serializing_if = "Option::is_none")]
    pub sales_tax_amount: Option<StringMajorUnit>,
    #[serde(
        rename = "InvoiceDiscountAmount",
        skip_serializing_if = "Option::is_none"
    )]
    pub invoice_discount_amount: Option<StringMajorUnit>,
    #[serde(
        rename = "InvoiceShippingAmount",
        skip_serializing_if = "Option::is_none"
    )]
    pub invoice_shipping_amount: Option<StringMajorUnit>,
}

impl WorldpayraftAmounts {
    /// Amount block carrying only `TransactionAmount`.
    pub fn transaction_only(transaction_amount: StringMajorUnit) -> Self {
        Self {
            transaction_amount,
            preauthorized_amount: None,
            sales_tax_amount: None,
            invoice_discount_amount: None,
            invoice_shipping_amount: None,
        }
    }
}

/// `CardInfo` (request) carrying a clear PAN.
#[derive(Debug, Serialize)]
pub struct WorldpayraftCardInfo<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "PAN", skip_serializing_if = "Option::is_none")]
    pub pan: Option<RawCardNumber<T>>,
    #[serde(rename = "ExpirationDate", skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<Secret<String>>,
}

/// `CardInfo` on a follow-up message that identifies the card by token.
#[derive(Debug, Serialize)]
pub struct WorldpayraftFollowUpCardInfo {
    #[serde(rename = "ExpirationDate")]
    pub expiration_date: Secret<String>,
}

/// `CardVerificationData` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftCardVerificationData {
    #[serde(rename = "Cvv2Cvc2CIDIndicator")]
    pub cvv_indicator: WorldpayraftCvvIndicator,
    #[serde(rename = "Cvv2Cvc2CIDValue", skip_serializing_if = "Option::is_none")]
    pub cvv_value: Option<Secret<String>>,
}

/// `AddressVerificationData` (request, credit only).
#[derive(Debug, Serialize)]
pub struct WorldpayraftAddressVerificationData {
    #[serde(skip_serializing_if = "Option::is_none", rename = "AVSZIPCode")]
    pub avs_zip_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "AVSAddress")]
    pub avs_address: Option<Secret<String>>,
}

/// `TerminalData` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftTerminalData {
    #[serde(rename = "EntryMode")]
    pub entry_mode: WorldpayraftEntryMode,
    #[serde(rename = "TerminalType", skip_serializing_if = "Option::is_none")]
    pub terminal_type: Option<WorldpayraftTerminalType>,
    #[serde(rename = "POSConditionCode")]
    pub pos_condition_code: WorldpayraftPosConditionCode,
    #[serde(rename = "TerminalEntryCap")]
    pub terminal_entry_cap: WorldpayraftTerminalEntryCap,
    #[serde(rename = "POSEnvironment", skip_serializing_if = "Option::is_none")]
    pub pos_environment: Option<WorldpayraftPosEnvironment>,
}

/// `E-commerceData` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftEcommerceData {
    #[serde(rename = "E-commerceIndicator")]
    pub ecommerce_indicator: WorldpayraftEcommerceIndicator,
    #[serde(rename = "3dSecureData", skip_serializing_if = "Option::is_none")]
    pub three_d_secure_data: Option<Secret<String>>,
    #[serde(
        rename = "3DSecureProgramProtocol",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_d_secure_program_protocol: Option<WorldpayraftThreeDsProtocol>,
    #[serde(
        rename = "3DSecureDirectoryServerTransactionID",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_d_secure_ds_transaction_id: Option<Secret<String>>,
    #[serde(
        rename = "E-commerceIPAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub ecommerce_ip_address: Option<Secret<String>>,
    #[serde(rename = "E-commerceOrderNum", skip_serializing_if = "Option::is_none")]
    pub ecommerce_order_num: Option<String>,
}

/// `ProcFlagsIndicators` (request).
#[derive(Debug, Default, Serialize)]
pub struct WorldpayraftProcFlags {
    #[serde(
        rename = "CardholderInitiatedTransaction",
        skip_serializing_if = "Option::is_none"
    )]
    pub cardholder_initiated_transaction: Option<WorldpayraftYesNo>,
    #[serde(
        rename = "MerchantInitiatedTransaction",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_initiated_transaction: Option<WorldpayraftYesNo>,
    #[serde(
        rename = "MastercardAdviceCodeIndicator",
        skip_serializing_if = "Option::is_none"
    )]
    pub mastercard_advice_code_indicator: Option<WorldpayraftYesNo>,
    #[serde(rename = "PartialAllowed", skip_serializing_if = "Option::is_none")]
    pub partial_allowed: Option<WorldpayraftYesNo>,
    #[serde(
        rename = "EventNotificationIndicator",
        skip_serializing_if = "Option::is_none"
    )]
    pub event_notification_indicator: Option<WorldpayraftYesNo>,
    #[serde(rename = "PriorAuth", skip_serializing_if = "Option::is_none")]
    pub prior_auth: Option<WorldpayraftYesNo>,
    #[serde(rename = "RecurringBillPay", skip_serializing_if = "Option::is_none")]
    pub recurring_bill_pay: Option<WorldpayraftYesNo>,
}

/// `EncryptionTokenData` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftEncryptionTokenRequest {
    #[serde(rename = "TokenizedPAN", skip_serializing_if = "Option::is_none")]
    pub tokenized_pan: Option<Secret<String>>,
    #[serde(rename = "WPTokenRequested", skip_serializing_if = "Option::is_none")]
    pub wp_token_requested: Option<WorldpayraftYesNo>,
}

/// `ReferenceTraceNumbers` (request). `SystemTraceNumber` is response-only and never sent.
#[derive(Debug, Default, Serialize)]
pub struct WorldpayraftRequestTraceNumbers {
    #[serde(rename = "CorrelationID", skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(
        rename = "AuthorizationNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub authorization_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber", skip_serializing_if = "Option::is_none")]
    pub retrieval_ref_number: Option<String>,
    #[serde(rename = "RefInvoiceNumber", skip_serializing_if = "Option::is_none")]
    pub ref_invoice_number: Option<String>,
    #[serde(
        rename = "EconomicallyRelatedLinkID",
        skip_serializing_if = "Option::is_none"
    )]
    pub economically_related_link_id: Option<String>,
}

impl WorldpayraftRequestTraceNumbers {
    fn is_empty(&self) -> bool {
        self.correlation_id.is_none()
            && self.authorization_number.is_none()
            && self.retrieval_ref_number.is_none()
            && self.ref_invoice_number.is_none()
            && self.economically_related_link_id.is_none()
    }
}

/// `UserDefinedData` (request). `UserData1` carries the message's own `APITransactionID` and
/// surfaces as `customerFields.field1` in Event Notifications, which is how webhooks are
/// correlated back to the payment.
#[derive(Debug, Serialize)]
pub struct WorldpayraftUserDefinedData {
    #[serde(rename = "UserData1", skip_serializing_if = "Option::is_none")]
    pub user_data_1: Option<String>,
}

/// `*SubsequentTransactionReasonCode` (request only, identical 9-value enum on all four brands).
/// Only `41` is sent: it is the one value a `RepeatPaymentData` input (MIT category
/// Resubmission) selects.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorldpayraftSubsequentTransactionReasonCode {
    /// 41 - Resubmission.
    #[serde(rename = "41")]
    Resubmission,
}

/// `VisaSpecificData` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftVisaSpecificData {
    #[serde(rename = "VisaTransactionId", skip_serializing_if = "Option::is_none")]
    pub visa_transaction_id: Option<String>,
    #[serde(
        rename = "VisaSubsequentTransactionReasonCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub visa_subsequent_transaction_reason_code:
        Option<WorldpayraftSubsequentTransactionReasonCode>,
}

/// `McrdSpecificData` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftMcrdSpecificData {
    #[serde(rename = "McrdBanknetREFNUM", skip_serializing_if = "Option::is_none")]
    pub mcrd_banknet_ref_num: Option<String>,
    #[serde(
        rename = "McrdBanknetSettleDate",
        skip_serializing_if = "Option::is_none"
    )]
    pub mcrd_banknet_settle_date: Option<String>,
    #[serde(
        rename = "McrdSubsequentTransactionReasonCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub mcrd_subsequent_transaction_reason_code:
        Option<WorldpayraftSubsequentTransactionReasonCode>,
}

/// `AmexSpecificData` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftAmexSpecificData {
    #[serde(rename = "AmexTransactionId", skip_serializing_if = "Option::is_none")]
    pub amex_transaction_id: Option<String>,
    #[serde(
        rename = "AmexSubsequentTransactionReasonCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub amex_subsequent_transaction_reason_code:
        Option<WorldpayraftSubsequentTransactionReasonCode>,
}

/// `DiscSpecificData` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftDiscSpecificData {
    #[serde(rename = "DiscTransactionId", skip_serializing_if = "Option::is_none")]
    pub disc_transaction_id: Option<String>,
    #[serde(
        rename = "DiscSubsequentTransactionReasonCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub disc_subsequent_transaction_reason_code:
        Option<WorldpayraftSubsequentTransactionReasonCode>,
}

/// `OnlineBillToAddress` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftOnlineBillToAddress {
    #[serde(
        rename = "OnlineBillToAddressLine1",
        skip_serializing_if = "Option::is_none"
    )]
    pub line1: Option<Secret<String>>,
    #[serde(
        rename = "OnlineBillToAddressLine2",
        skip_serializing_if = "Option::is_none"
    )]
    pub line2: Option<Secret<String>>,
    #[serde(rename = "OnlineBillToCity", skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(rename = "OnlineBillToState", skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(
        rename = "OnlineBillToZipCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub zip_code: Option<Secret<String>>,
    #[serde(
        rename = "OnlineBillToCountry",
        skip_serializing_if = "Option::is_none"
    )]
    pub country: Option<common_enums::CountryAlpha3>,
}

impl WorldpayraftOnlineBillToAddress {
    /// True when no field is set: such an object is not sent (an empty `{}` is not a valid
    /// address).
    fn is_empty(&self) -> bool {
        self.line1.is_none()
            && self.line2.is_none()
            && self.city.is_none()
            && self.state.is_none()
            && self.zip_code.is_none()
            && self.country.is_none()
    }
}

/// `OnlineShipToAddress` (request).
#[derive(Debug, Serialize)]
pub struct WorldpayraftOnlineShipToAddress {
    #[serde(
        rename = "OnlineShipToAddressLine1",
        skip_serializing_if = "Option::is_none"
    )]
    pub line1: Option<Secret<String>>,
    #[serde(
        rename = "OnlineShipToAddressLine2",
        skip_serializing_if = "Option::is_none"
    )]
    pub line2: Option<Secret<String>>,
    #[serde(rename = "OnlineShipToCity", skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(rename = "OnlineShipToState", skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(
        rename = "OnlineShipToZipCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub zip_code: Option<Secret<String>>,
    #[serde(
        rename = "OnlineShipToCountry",
        skip_serializing_if = "Option::is_none"
    )]
    pub country: Option<common_enums::CountryAlpha3>,
}

impl WorldpayraftOnlineShipToAddress {
    /// True when no field is set: such an object is not sent (an empty `{}` is not a valid
    /// address).
    fn is_empty(&self) -> bool {
        self.line1.is_none()
            && self.line2.is_none()
            && self.city.is_none()
            && self.state.is_none()
            && self.zip_code.is_none()
            && self.country.is_none()
    }
}

/// `CustomerInformation` (request, credit only in this integration).
#[derive(Debug, Serialize)]
pub struct WorldpayraftCustomerInformation {
    #[serde(
        rename = "CardholderFirstName",
        skip_serializing_if = "Option::is_none"
    )]
    pub cardholder_first_name: Option<Secret<String>>,
    #[serde(rename = "CardholderLastName", skip_serializing_if = "Option::is_none")]
    pub cardholder_last_name: Option<Secret<String>>,
}

/// `SoftDescriptorData` (request, credit only).
#[derive(Debug, Serialize)]
pub struct WorldpayraftSoftDescriptorData {
    #[serde(rename = "SdMerchantName", skip_serializing_if = "Option::is_none")]
    pub sd_merchant_name: Option<Secret<String>>,
    #[serde(rename = "SdMerchantCity", skip_serializing_if = "Option::is_none")]
    pub sd_merchant_city: Option<Secret<String>>,
}

/// `MerchantSpecificData` (request, credit only).
#[derive(Debug, Serialize)]
pub struct WorldpayraftMerchantSpecificData {
    /// ISO-4217 numeric currency code — the only currency field in the API. Typed as the
    /// currency and written as its numeric code (`serialize_iso_4217_numeric`).
    #[serde(
        rename = "AcquirerCurrencyCode",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_iso_4217_numeric"
    )]
    pub acquirer_currency_code: Option<common_enums::Currency>,
    #[serde(
        rename = "MerchantCustomerServicePhone",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_customer_service_phone: Option<Secret<String>>,
}

/// Writes a currency as its ISO-4217 numeric code (`840` for USD), the RAFT wire form.
fn serialize_iso_4217_numeric<S: serde::Serializer>(
    currency: &Option<common_enums::Currency>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match currency {
        Some(currency) => serializer.serialize_str(currency.iso_4217()),
        None => serializer.serialize_none(),
    }
}

/// One `Level3Data` element (request, credit only). `UnitPrice` carries implied decimals given
/// by `UnitPriceDecimal`.
#[derive(Debug, Serialize)]
pub struct WorldpayraftLevel3Item {
    #[serde(rename = "ItemDescription", skip_serializing_if = "Option::is_none")]
    pub item_description: Option<String>,
    #[serde(rename = "ProductCode", skip_serializing_if = "Option::is_none")]
    pub product_code: Option<String>,
    #[serde(rename = "UnitOfMeasure", skip_serializing_if = "Option::is_none")]
    pub unit_of_measure: Option<String>,
    #[serde(rename = "UnitPrice")]
    pub unit_price: StringMinorUnit,
    #[serde(rename = "UnitPriceDecimal")]
    pub unit_price_decimal: String,
    #[serde(rename = "ItemQuantity")]
    pub item_quantity: String,
    #[serde(rename = "ItemQuantityDecimal")]
    pub item_quantity_decimal: String,
}

// =============================================================================
// SHARED RESPONSE STRUCTS
// =============================================================================

/// `ReturnCode` — operation level (the complete published set). `Display` prints the wire code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, strum::Display)]
pub enum WorldpayraftReturnCode {
    #[serde(rename = "0000")]
    #[strum(serialize = "0000")]
    Successful,
    #[serde(rename = "0004")]
    #[strum(serialize = "0004")]
    EditError,
    #[serde(rename = "0008")]
    #[strum(serialize = "0008")]
    LogicError,
    #[serde(rename = "0012")]
    #[strum(serialize = "0012")]
    SystemIssue,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftErrorInformation {
    #[serde(rename = "FieldInError")]
    pub field_in_error: Option<String>,
    #[serde(rename = "ErrorText")]
    pub error_text: Option<String>,
}

/// `ReferenceTraceNumbers` (response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftResponseReferenceTraceNumbers {
    #[serde(rename = "SystemTraceNumber")]
    pub system_trace_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
    #[serde(rename = "CorrelationID")]
    pub correlation_id: Option<String>,
    #[serde(rename = "AuthorizationNumber")]
    pub authorization_number: Option<String>,
    #[serde(rename = "PaymentAcctREFNumber")]
    pub payment_acct_ref_number: Option<String>,
    #[serde(rename = "NetworkTraceNumber")]
    pub network_trace_number: Option<String>,
    #[serde(rename = "NetworkRefNumber")]
    pub network_ref_number: Option<String>,
    #[serde(rename = "TransactionLinkID")]
    pub transaction_link_id: Option<String>,
}

/// `EncryptionTokenData` (response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftResponseEncryptionTokenData {
    #[serde(rename = "TokenizedPAN")]
    pub tokenized_pan: Option<Secret<String>>,
    #[serde(rename = "PAN-Last4")]
    pub pan_last4: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftAvsResponse {
    #[serde(rename = "AVSResult")]
    pub avs_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftCvvResponse {
    #[serde(rename = "Cvv2Cvc2CIDResult")]
    pub cvv_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftEcommerceResponse {
    #[serde(rename = "3dSecureResult")]
    pub three_d_secure_result: Option<String>,
    #[serde(rename = "ReturnE-commerceIndicator")]
    pub return_ecommerce_indicator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftVisaResponseData {
    #[serde(rename = "VisaTransactionId")]
    pub visa_transaction_id: Option<String>,
    #[serde(rename = "VisaResponseCode")]
    pub visa_response_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftMcrdResponseData {
    #[serde(rename = "McrdBanknetREFNUM")]
    pub mcrd_banknet_ref_num: Option<String>,
    #[serde(rename = "McrdBanknetSettleDate")]
    pub mcrd_banknet_settle_date: Option<String>,
    #[serde(rename = "MastercardMerchantAdviceCode")]
    pub mastercard_merchant_advice_code: Option<String>,
    #[serde(rename = "McrdTranLinkIdValidationIndicator")]
    pub mcrd_tran_link_id_validation_indicator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftAmexResponseData {
    #[serde(rename = "AmexTransactionId")]
    pub amex_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftDiscResponseData {
    #[serde(rename = "DiscTransactionId")]
    pub disc_transaction_id: Option<String>,
    #[serde(rename = "DiscResponseCode")]
    pub disc_response_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftMiscAmountsResponse {
    #[serde(rename = "OriginalAuthAmount")]
    pub original_auth_amount: Option<StringMajorUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftProcFlagsResponse {
    #[serde(rename = "PartiallyAuthorized")]
    pub partially_authorized: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftSettlementData {
    #[serde(rename = "SettlementDate")]
    pub settlement_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftRoutingData {
    #[serde(rename = "NetworkId")]
    pub network_id: Option<String>,
}

/// The single object inside every `<operation>response` wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftResponseInner {
    #[serde(rename = "ReturnCode")]
    pub return_code: WorldpayraftReturnCode,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    #[serde(rename = "ReturnText")]
    pub return_text: Option<String>,
    #[serde(rename = "ResponseCode")]
    pub response_code: Option<String>,
    #[serde(rename = "AuthorizationSource")]
    pub authorization_source: Option<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
    #[serde(rename = "ErrorInformation")]
    pub error_information: Option<WorldpayraftErrorInformation>,
    #[serde(rename = "ReferenceTraceNumbers")]
    pub reference_trace_numbers: Option<WorldpayraftResponseReferenceTraceNumbers>,
    #[serde(rename = "EncryptionTokenData")]
    pub encryption_token_data: Option<WorldpayraftResponseEncryptionTokenData>,
    #[serde(rename = "AddressVerificationData")]
    pub address_verification_data: Option<WorldpayraftAvsResponse>,
    #[serde(rename = "CardVerificationData")]
    pub card_verification_data: Option<WorldpayraftCvvResponse>,
    #[serde(rename = "E-commerceData")]
    pub ecommerce_data: Option<WorldpayraftEcommerceResponse>,
    #[serde(rename = "VisaSpecificData")]
    pub visa_specific_data: Option<WorldpayraftVisaResponseData>,
    #[serde(rename = "McrdSpecificData")]
    pub mcrd_specific_data: Option<WorldpayraftMcrdResponseData>,
    #[serde(rename = "AmexSpecificData")]
    pub amex_specific_data: Option<WorldpayraftAmexResponseData>,
    #[serde(rename = "DiscSpecificData")]
    pub disc_specific_data: Option<WorldpayraftDiscResponseData>,
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: Option<WorldpayraftMiscAmountsResponse>,
    #[serde(rename = "ProcFlagsIndicators")]
    pub proc_flags_indicators: Option<WorldpayraftProcFlagsResponse>,
    #[serde(rename = "SettlementData")]
    pub settlement_data: Option<WorldpayraftSettlementData>,
    #[serde(rename = "WorldPayRoutingData")]
    pub world_pay_routing_data: Option<WorldpayraftRoutingData>,
}

/// Decision classes of a RAFT response (tech spec: Error model; Status Mappings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldpayraftDecision {
    /// ReturnCode 0000 + ResponseCode 000.
    Approved,
    /// ReturnCode 0000 + ResponseCode 010 PARTIAL APPROVAL.
    PartialApproval,
    /// ReturnCode 0000 + ResponseCode 003 HONOR WITH ID (manual review).
    HonorWithId,
    /// ReturnCode 0000 + ResponseCode 114 REQUEST IN PROGRESS (not terminal).
    RequestInProgress,
    /// ReturnCode 0000 + any other or absent ResponseCode: "Any response code not recognized
    /// should be treated as a decline."
    Declined,
    /// ReturnCode other than 0000: the message was rejected before reaching the issuer.
    MessageRejected,
}

pub fn classify(inner: &WorldpayraftResponseInner) -> WorldpayraftDecision {
    match inner.return_code {
        WorldpayraftReturnCode::Successful => match inner.response_code.as_deref() {
            Some("000") => WorldpayraftDecision::Approved,
            Some("010") => WorldpayraftDecision::PartialApproval,
            Some("003") => WorldpayraftDecision::HonorWithId,
            Some("114") => WorldpayraftDecision::RequestInProgress,
            Some(_) | None => WorldpayraftDecision::Declined,
        },
        WorldpayraftReturnCode::EditError
        | WorldpayraftReturnCode::LogicError
        | WorldpayraftReturnCode::SystemIssue
        | WorldpayraftReturnCode::Unknown => WorldpayraftDecision::MessageRejected,
    }
}

impl WorldpayraftResponseInner {
    /// Network transaction id: Visa, then Mastercard (`McrdBanknetREFNUM` + `McrdBanknetSettleDate`,
    /// 13 characters), then Amex, then Discover.
    fn network_transaction_id(&self) -> Option<String> {
        let visa = self
            .visa_specific_data
            .as_ref()
            .and_then(|visa| visa.visa_transaction_id.clone());
        let mastercard = self.mcrd_specific_data.as_ref().and_then(|mcrd| {
            match (&mcrd.mcrd_banknet_ref_num, &mcrd.mcrd_banknet_settle_date) {
                (Some(ref_num), Some(settle_date)) => Some(format!("{ref_num}{settle_date}")),
                _ => None,
            }
        });
        let amex = self
            .amex_specific_data
            .as_ref()
            .and_then(|amex| amex.amex_transaction_id.clone());
        let discover = self
            .disc_specific_data
            .as_ref()
            .and_then(|disc| disc.disc_transaction_id.clone());
        visa.or(mastercard).or(amex).or(discover)
    }

    fn network_ids(&self) -> Option<WorldpayraftNetworkIds> {
        let ids = WorldpayraftNetworkIds {
            visa_transaction_id: self
                .visa_specific_data
                .as_ref()
                .and_then(|visa| visa.visa_transaction_id.clone()),
            mcrd_banknet_ref_num: self
                .mcrd_specific_data
                .as_ref()
                .and_then(|mcrd| mcrd.mcrd_banknet_ref_num.clone()),
            mcrd_banknet_settle_date: self
                .mcrd_specific_data
                .as_ref()
                .and_then(|mcrd| mcrd.mcrd_banknet_settle_date.clone()),
            amex_transaction_id: self
                .amex_specific_data
                .as_ref()
                .and_then(|amex| amex.amex_transaction_id.clone()),
            disc_transaction_id: self
                .disc_specific_data
                .as_ref()
                .and_then(|disc| disc.disc_transaction_id.clone()),
            transaction_link_id: self
                .reference_trace_numbers
                .as_ref()
                .and_then(|trace| trace.transaction_link_id.clone()),
        };
        (ids != WorldpayraftNetworkIds::default()).then_some(ids)
    }

    fn tokenized_pan(&self) -> Option<Secret<String>> {
        self.encryption_token_data
            .as_ref()
            .and_then(|token| token.tokenized_pan.clone())
    }

    fn authorization_number(&self) -> Option<String> {
        self.reference_trace_numbers
            .as_ref()
            .and_then(|trace| trace.authorization_number.clone())
    }

    /// Card `connector_response`: AVS/CVV results, response-side 3DS fields, auth code, network.
    fn card_connector_response(&self) -> Option<ConnectorResponseData> {
        let avs_result = self
            .address_verification_data
            .as_ref()
            .and_then(|avs| avs.avs_result.clone());
        let cvv_result = self
            .card_verification_data
            .as_ref()
            .and_then(|cvv| cvv.cvv_result.clone());
        let payment_checks = (avs_result.is_some() || cvv_result.is_some()).then(|| {
            serde_json::json!({
                "avs_result": avs_result,
                "cvv_result": cvv_result,
            })
        });
        let three_ds_result = self
            .ecommerce_data
            .as_ref()
            .and_then(|ecom| ecom.three_d_secure_result.clone());
        let return_eci = self
            .ecommerce_data
            .as_ref()
            .and_then(|ecom| ecom.return_ecommerce_indicator.clone());
        let authentication_data = (three_ds_result.is_some() || return_eci.is_some()).then(|| {
            serde_json::json!({
                "three_ds_result": three_ds_result,
                "return_eci": return_eci,
            })
        });
        let auth_code = self.authorization_number();
        let card_network = self
            .world_pay_routing_data
            .as_ref()
            .and_then(|routing| routing.network_id.clone());
        (payment_checks.is_some()
            || authentication_data.is_some()
            || auth_code.is_some()
            || card_network.is_some())
        .then(|| {
            ConnectorResponseData::with_additional_payment_method_data(
                AdditionalPaymentMethodConnectorResponse::Card {
                    authentication_data,
                    payment_checks,
                    card_network,
                    domestic_network: None,
                    auth_code,
                },
            )
        })
    }
}

// =============================================================================
// AUTHORIZE REQUEST
// =============================================================================

/// Body of every original payment message (`creditpurchase`, `creditauth`, `debitpurchase`,
/// `debitpreauth`). Debit messages leave the credit-only objects empty.
#[derive(Debug, Serialize)]
pub struct WorldpayraftPaymentInner<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(rename = "CardInfo")]
    pub card_info: WorldpayraftCardInfo<T>,
    #[serde(
        rename = "CardVerificationData",
        skip_serializing_if = "Option::is_none"
    )]
    pub card_verification_data: Option<WorldpayraftCardVerificationData>,
    #[serde(
        rename = "AddressVerificationData",
        skip_serializing_if = "Option::is_none"
    )]
    pub address_verification_data: Option<WorldpayraftAddressVerificationData>,
    #[serde(
        rename = "OnlineBillToAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub online_bill_to_address: Option<WorldpayraftOnlineBillToAddress>,
    #[serde(
        rename = "OnlineShipToAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub online_ship_to_address: Option<WorldpayraftOnlineShipToAddress>,
    #[serde(
        rename = "CustomerInformation",
        skip_serializing_if = "Option::is_none"
    )]
    pub customer_information: Option<WorldpayraftCustomerInformation>,
    #[serde(rename = "TerminalData")]
    pub terminal_data: WorldpayraftTerminalData,
    #[serde(rename = "E-commerceData")]
    pub ecommerce_data: WorldpayraftEcommerceData,
    #[serde(rename = "ProcFlagsIndicators")]
    pub proc_flags_indicators: WorldpayraftProcFlags,
    #[serde(
        rename = "EncryptionTokenData",
        skip_serializing_if = "Option::is_none"
    )]
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenRequest>,
    #[serde(
        rename = "ReferenceTraceNumbers",
        skip_serializing_if = "Option::is_none"
    )]
    pub reference_trace_numbers: Option<WorldpayraftRequestTraceNumbers>,
    #[serde(rename = "UserDefinedData", skip_serializing_if = "Option::is_none")]
    pub user_defined_data: Option<WorldpayraftUserDefinedData>,
    #[serde(rename = "SoftDescriptorData", skip_serializing_if = "Option::is_none")]
    pub soft_descriptor_data: Option<WorldpayraftSoftDescriptorData>,
    #[serde(
        rename = "MerchantSpecificData",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_specific_data: Option<WorldpayraftMerchantSpecificData>,
    #[serde(rename = "Level3Data", skip_serializing_if = "Option::is_none")]
    pub level3_data: Option<Vec<WorldpayraftLevel3Item>>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
    pub local_date_time: String,
}

/// Original payment message, keyed by its operation wrapper.
///
/// `/credit/purchase` → `creditpurchase`, `/credit/authorization` → `creditauth`,
/// `/debit/purchase` → `debitpurchase`, `/debit/preauth` → `debitpreauth`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    CreditPurchase {
        creditpurchase: WorldpayraftPaymentInner<T>,
    },
    CreditAuth {
        creditauth: WorldpayraftPaymentInner<T>,
    },
    DebitPurchase {
        debitpurchase: WorldpayraftPaymentInner<T>,
    },
    DebitPreauth {
        debitpreauth: WorldpayraftPaymentInner<T>,
    },
}

/// Address-derived request objects (credit: all four; debit: bill-to and ship-to only).
struct WorldpayraftAddressObjects {
    address_verification_data: Option<WorldpayraftAddressVerificationData>,
    online_bill_to_address: Option<WorldpayraftOnlineBillToAddress>,
    online_ship_to_address: Option<WorldpayraftOnlineShipToAddress>,
    customer_information: Option<WorldpayraftCustomerInformation>,
}

/// OnlineBillToState / OnlineShipToState are `maxLength: 2` state codes (Native Raft Credit API
/// 1.35.1 OnlineBillToState_Type). A US state or Canadian province name is converted to its code;
/// a value that is still not a code of at most 2 characters is refused, never truncated.
fn to_raft_state_code(
    state: &Secret<String>,
    country: Option<common_enums::CountryAlpha2>,
    field_name: &str,
) -> Result<Option<Secret<String>>, error_stack::Report<errors::IntegrationError>> {
    if state.peek().trim().is_empty() {
        return Ok(None);
    }
    let received_length = state.peek().chars().count();
    crate::utils::get_state_code_for_country(state, country)
        .filter(|code| code.peek().chars().count() <= MAX_ADDRESS_STATE)
        .map(Some)
        .ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MaxFieldLengthViolated {
                connector: CONNECTOR_NAME.to_string(),
                field_name: field_name.to_string(),
                max_length: MAX_ADDRESS_STATE,
                received_length,
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "RAFT takes a 2-character state/province code; the state name could not be converted".to_string(),
                    ),
                    ..Default::default()
                },
            })
        })
}

/// Maps billing and shipping addresses onto AVS, bill-to, ship-to and cardholder objects.
/// Free-text values longer than their documented maximum are truncated (tech spec: Recommended
/// UCS address mapping); the state is a 2-character code, converted from a US/CA name and
/// refused otherwise (see `to_raft_state_code`); an absent source omits the object.
fn build_addresses(
    billing: Option<&Address>,
    shipping: Option<&Address>,
    card_kind: WorldpayraftCardKind,
) -> Result<WorldpayraftAddressObjects, error_stack::Report<errors::IntegrationError>> {
    let is_credit = card_kind == WorldpayraftCardKind::Credit;
    let billing_details = billing.and_then(|address| address.address.as_ref());
    let shipping_details = shipping.and_then(|address| address.address.as_ref());

    let address_verification_data = billing_details.filter(|_| is_credit).and_then(|details| {
        let avs_zip_code = details
            .zip
            .as_ref()
            .map(|zip| truncate_secret_string(zip, MAX_AVS_ZIP));
        let avs_address = details
            .line1
            .as_ref()
            .map(|line1| truncate_secret_string(line1, MAX_AVS_ADDRESS));
        (avs_zip_code.is_some() || avs_address.is_some()).then_some(
            WorldpayraftAddressVerificationData {
                avs_zip_code,
                avs_address,
            },
        )
    });

    let online_bill_to_address = billing_details
        .map(
            |details| -> Result<_, error_stack::Report<errors::IntegrationError>> {
                Ok(WorldpayraftOnlineBillToAddress {
                    line1: details
                        .line1
                        .as_ref()
                        .map(|value| truncate_secret_string(value, MAX_ADDRESS_LINE)),
                    line2: details
                        .line2
                        .as_ref()
                        .map(|value| truncate_secret_string(value, MAX_ADDRESS_LINE)),
                    city: details
                        .city
                        .as_ref()
                        .map(|value| truncate_secret_string(value, MAX_ADDRESS_CITY)),
                    state: details
                        .state
                        .as_ref()
                        .map(|value| {
                            to_raft_state_code(value, details.country, "billing.address.state")
                        })
                        .transpose()?
                        .flatten(),
                    zip_code: details
                        .zip
                        .as_ref()
                        .map(|value| truncate_secret_string(value, MAX_ADDRESS_ZIP)),
                    country: details
                        .country
                        .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
                })
            },
        )
        .transpose()?
        .filter(|address| !address.is_empty());

    let online_ship_to_address = shipping_details
        .map(
            |details| -> Result<_, error_stack::Report<errors::IntegrationError>> {
                Ok(WorldpayraftOnlineShipToAddress {
                    line1: details
                        .line1
                        .as_ref()
                        .map(|value| truncate_secret_string(value, MAX_ADDRESS_LINE)),
                    line2: details
                        .line2
                        .as_ref()
                        .map(|value| truncate_secret_string(value, MAX_ADDRESS_LINE)),
                    city: details
                        .city
                        .as_ref()
                        .map(|value| truncate_secret_string(value, MAX_ADDRESS_CITY)),
                    state: details
                        .state
                        .as_ref()
                        .map(|value| {
                            to_raft_state_code(value, details.country, "shipping.address.state")
                        })
                        .transpose()?
                        .flatten(),
                    zip_code: details
                        .zip
                        .as_ref()
                        .map(|value| truncate_secret_string(value, MAX_ADDRESS_ZIP)),
                    country: details
                        .country
                        .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
                })
            },
        )
        .transpose()?
        .filter(|address| !address.is_empty());

    let customer_information = billing_details.filter(|_| is_credit).and_then(|details| {
        let cardholder_first_name = details
            .first_name
            .as_ref()
            .map(|value| truncate_secret_string(value, MAX_CARDHOLDER_FIRST_NAME));
        let cardholder_last_name = details
            .last_name
            .as_ref()
            .map(|value| truncate_secret_string(value, MAX_CARDHOLDER_LAST_NAME));
        (cardholder_first_name.is_some() || cardholder_last_name.is_some()).then_some(
            WorldpayraftCustomerInformation {
                cardholder_first_name,
                cardholder_last_name,
            },
        )
    });

    Ok(WorldpayraftAddressObjects {
        address_verification_data,
        online_bill_to_address,
        online_ship_to_address,
        customer_information,
    })
}

/// `SoftDescriptorData` and `MerchantSpecificData` (credit only).
fn build_descriptor_and_merchant_data<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
    card_kind: WorldpayraftCardKind,
) -> (
    Option<WorldpayraftSoftDescriptorData>,
    Option<WorldpayraftMerchantSpecificData>,
) {
    if card_kind == WorldpayraftCardKind::Debit {
        return (None, None);
    }
    let descriptor = request.billing_descriptor.as_ref();
    let sd_merchant_name = descriptor
        .and_then(|descriptor| {
            descriptor
                .name
                .clone()
                .or_else(|| descriptor.statement_descriptor.clone().map(Secret::new))
        })
        .map(|name| truncate_secret_string(&name, MAX_SD_MERCHANT_NAME));
    let sd_merchant_city = descriptor
        .and_then(|descriptor| descriptor.city.as_ref())
        .map(|city| truncate_secret_string(city, MAX_SD_MERCHANT_CITY));
    let soft_descriptor_data = (sd_merchant_name.is_some() || sd_merchant_city.is_some())
        .then_some(WorldpayraftSoftDescriptorData {
            sd_merchant_name,
            sd_merchant_city,
        });
    let merchant_specific_data = WorldpayraftMerchantSpecificData {
        acquirer_currency_code: Some(request.currency),
        merchant_customer_service_phone: descriptor
            .and_then(|descriptor| descriptor.phone.as_ref())
            .map(|phone| truncate_secret_string(phone, MAX_CUSTOMER_SERVICE_PHONE)),
    };
    (soft_descriptor_data, Some(merchant_specific_data))
}

/// Level 2 amounts, `RefInvoiceNumber` and `Level3Data` (credit only).
struct WorldpayraftLevel2Level3 {
    sales_tax_amount: Option<StringMajorUnit>,
    invoice_discount_amount: Option<StringMajorUnit>,
    invoice_shipping_amount: Option<StringMajorUnit>,
    ref_invoice_number: Option<String>,
    level3_data: Option<Vec<WorldpayraftLevel3Item>>,
}

fn build_level2_level3<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &WorldpayraftRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
    card_kind: WorldpayraftCardKind,
) -> Result<WorldpayraftLevel2Level3, error_stack::Report<errors::IntegrationError>> {
    let empty = WorldpayraftLevel2Level3 {
        sales_tax_amount: None,
        invoice_discount_amount: None,
        invoice_shipping_amount: None,
        ref_invoice_number: None,
        level3_data: None,
    };
    if card_kind == WorldpayraftCardKind::Debit {
        return Ok(empty);
    }
    let router_data = &item.router_data;
    let Some(l2_l3) = router_data.resource_common_data.l2_l3_data.as_deref() else {
        return Ok(empty);
    };
    let currency = router_data.request.currency;
    let amount_error = || errors::IntegrationError::AmountConversionFailed {
        context: errors::IntegrationErrorContext {
            additional_context: Some(
                "Worldpay RAFT Level 2 amounts are ddddddddd.cc major-unit strings".to_string(),
            ),
            ..Default::default()
        },
    };
    let convert_major = |amount: MinorUnit| {
        item.connector
            .amount_converter
            .convert(amount, currency)
            .change_context(amount_error())
    };
    let order_info = l2_l3.order_info.as_ref();
    let sales_tax_amount = l2_l3
        .tax_info
        .as_ref()
        .and_then(|tax| tax.order_tax_amount)
        .map(convert_major)
        .transpose()?;
    let invoice_discount_amount = order_info
        .and_then(|order| order.discount_amount)
        .map(convert_major)
        .transpose()?;
    let invoice_shipping_amount = order_info
        .and_then(|order| order.shipping_cost)
        .map(convert_major)
        .transpose()?;
    // G-Authorize-05: RefInvoiceNumber is maxLength 20; a longer reference is refused, never
    // dropped or truncated (same rule as E-commerceOrderNum, G-Authorize-04).
    let ref_invoice_number = order_info
        .and_then(|order| order.merchant_order_reference_id.as_deref())
        .filter(|reference| !reference.is_empty())
        .map(|reference| {
            fit_identifier(reference, MAX_REF_INVOICE_NUMBER).ok_or_else(|| {
                error_stack::report!(errors::IntegrationError::MaxFieldLengthViolated {
                    connector: CONNECTOR_NAME.to_string(),
                    field_name: "l2_l3_data.order_info.merchant_order_reference_id".to_string(),
                    max_length: MAX_REF_INVOICE_NUMBER,
                    received_length: reference.chars().count(),
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "ReferenceTraceNumbers.RefInvoiceNumber accepts at most 20 characters"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "send a merchant_order_reference_id of at most 20 characters"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })
            })
        })
        .transpose()?;

    let level3_data = match order_info.and_then(|order| order.order_details.as_ref()) {
        Some(details) if !details.is_empty() => {
            // G-Authorize-06: Level3Data carries at most 25 line items; more are refused, never
            // silently dropped.
            if details.len() > MAX_LEVEL3_ITEMS {
                return Err(error_stack::report!(
                    errors::IntegrationError::MaxFieldLengthViolated {
                        connector: CONNECTOR_NAME.to_string(),
                        field_name: "l2_l3_data.order_info.order_details".to_string(),
                        max_length: MAX_LEVEL3_ITEMS,
                        received_length: details.len(),
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Level3Data accepts at most 25 line items".to_string(),
                            ),
                            suggested_action: Some("send at most 25 order_details".to_string()),
                            ..Default::default()
                        },
                    }
                ));
            }
            let unit_price_decimal = currency
                .number_of_digits_after_decimal_point()
                .change_context(errors::IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Level3Data.UnitPriceDecimal needs the currency's number of decimal digits".to_string(),
                        ),
                        ..Default::default()
                    },
                })?
                .to_string();
            let items = details
                .iter()
                .map(|detail| {
                    let unit_price = item
                        .connector
                        .line_item_amount_converter
                        .convert(detail.amount, currency)
                        .change_context(errors::IntegrationError::AmountConversionFailed {
                            context: errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "Worldpay RAFT Level3Data.UnitPrice uses implied decimals (minor units)".to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;
                    // G-Authorize-06: ProductCode is an identifier (maxLength 15); a longer one
                    // is refused, never truncated.
                    let product_code = detail
                        .product_id
                        .as_deref()
                        .or(detail.sku.as_deref())
                        .filter(|code| !code.is_empty())
                        .map(|code| {
                            fit_identifier(code, MAX_PRODUCT_CODE).ok_or_else(|| {
                                error_stack::report!(
                                    errors::IntegrationError::MaxFieldLengthViolated {
                                        connector: CONNECTOR_NAME.to_string(),
                                        field_name:
                                            "l2_l3_data.order_info.order_details.product_id"
                                                .to_string(),
                                        max_length: MAX_PRODUCT_CODE,
                                        received_length: code.chars().count(),
                                        context: errors::IntegrationErrorContext {
                                            additional_context: Some(
                                                "Level3Data.ProductCode accepts at most 15 characters"
                                                    .to_string(),
                                            ),
                                            suggested_action: Some(
                                                "send a product_id of at most 15 characters"
                                                    .to_string(),
                                            ),
                                            ..Default::default()
                                        },
                                    }
                                )
                            })
                        })
                        .transpose()?;
                    Ok(WorldpayraftLevel3Item {
                        item_description: Some(truncate_chars(
                            &detail.product_name,
                            MAX_ITEM_DESCRIPTION,
                        ))
                        .filter(|description| !description.is_empty()),
                        product_code,
                        unit_of_measure: detail
                            .unit_of_measure
                            .as_deref()
                            .map(|unit| truncate_chars(unit, MAX_UNIT_OF_MEASURE)),
                        unit_price,
                        unit_price_decimal: unit_price_decimal.clone(),
                        item_quantity: detail.quantity.to_string(),
                        item_quantity_decimal: "0".to_string(),
                    })
                })
                .collect::<Result<Vec<_>, error_stack::Report<errors::IntegrationError>>>()?;
            Some(items)
        }
        _ => None,
    };

    Ok(WorldpayraftLevel2Level3 {
        sales_tax_amount,
        invoice_discount_amount,
        invoice_shipping_amount,
        ref_invoice_number,
        level3_data,
    })
}

/// External 3DS results (LEG-COUNT = 0: Native RAFT has no authentication call, so the
/// merchant's 3DS server result rides inside the Authorize message's `E-commerceData`).
struct WorldpayraftExternalThreeDs {
    ecommerce_indicator: WorldpayraftEcommerceIndicator,
    cavv: Secret<String>,
    protocol: Option<WorldpayraftThreeDsProtocol>,
    ds_transaction_id: Option<Secret<String>>,
}

/// Translates the external 3DS server's ECI into RAFT's brand-neutral `05`/`06`: Visa, Amex
/// and Discover report `05`/`06`, Mastercard reports `02`/`01`. Without a recognised ECI the
/// EMV `transStatus` decides (`Y` -> `05`, `A` -> `06`). Anything else is not an authenticated
/// or attempted result and has no RAFT value.
fn external_three_ds_indicator(
    authentication_data: &AuthenticationData,
) -> Option<WorldpayraftEcommerceIndicator> {
    match authentication_data.eci.as_deref().map(str::trim) {
        Some("05") | Some("02") => Some(WorldpayraftEcommerceIndicator::Authenticated),
        Some("06") | Some("01") => Some(WorldpayraftEcommerceIndicator::Attempted),
        _ => match authentication_data.trans_status {
            Some(common_enums::TransactionStatus::Success) => {
                Some(WorldpayraftEcommerceIndicator::Authenticated)
            }
            Some(common_enums::TransactionStatus::NotVerified) => {
                Some(WorldpayraftEcommerceIndicator::Attempted)
            }
            Some(common_enums::TransactionStatus::Failure)
            | Some(common_enums::TransactionStatus::VerificationNotPerformed)
            | Some(common_enums::TransactionStatus::Rejected)
            | Some(common_enums::TransactionStatus::ChallengeRequired)
            | Some(common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication)
            | Some(common_enums::TransactionStatus::InformationOnly)
            | None => None,
        },
    }
}

/// Validates and resolves the external 3DS input of a payment before any request field is
/// built (G-ThreeDS-01..03). `Ok(None)` is a plain non-3DS payment.
fn resolve_external_three_ds(
    auth_type: common_enums::AuthenticationType,
    authentication_data: Option<&AuthenticationData>,
) -> Result<Option<WorldpayraftExternalThreeDs>, error_stack::Report<errors::IntegrationError>> {
    let authentication_data = match authentication_data {
        Some(authentication_data) => authentication_data,
        // G-ThreeDS-01: RAFT cannot authenticate; never downgrade a 3DS request to ECI 07.
        None if auth_type == common_enums::AuthenticationType::ThreeDs => {
            return Err(error_stack::report!(errors::IntegrationError::NotSupported {
                message: "3DS authentication".to_string(),
                connector: CONNECTOR_NAME,
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Native RAFT performs no 3DS authentication (no initiate/challenge/validate call); only external 3DS results are accepted".to_string(),
                    ),
                    suggested_action: Some(
                        "authenticate with an external 3DS server and pass authentication_data".to_string(),
                    ),
                    ..Default::default()
                },
            }));
        }
        None => return Ok(None),
    };

    // G-ThreeDS-03: a failed or unknown authentication result must not be sent as authenticated.
    let ecommerce_indicator = external_three_ds_indicator(authentication_data).ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::InvalidDataFormat {
            field_name: "authentication_data.eci",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "only fully authenticated (05) or attempted (06) external 3DS results can be passed to RAFT; a failed authentication must not be sent as authenticated".to_string(),
                ),
                suggested_action: Some(
                    "pass an authenticated (transStatus Y) or attempted (transStatus A) external 3DS result, or pay without 3DS".to_string(),
                ),
                ..Default::default()
            },
        })
    })?;

    // G-ThreeDS-02: ECI 05/06 must carry the cryptogram in 3dSecureData.
    let cavv = authentication_data.cavv.clone().ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "authentication_data.cavv",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "E-commerceIndicator 05/06 must carry the cryptogram in 3dSecureData"
                        .to_string(),
                ),
                suggested_action: Some(
                    "pass the base64 CAVV/AAV/AEVV returned by the external 3DS server".to_string(),
                ),
                ..Default::default()
            },
        })
    })?;

    let protocol = authentication_data
        .message_version
        .as_ref()
        .and_then(|version| match version.get_major() {
            1 => Some(WorldpayraftThreeDsProtocol::V1),
            2 => Some(WorldpayraftThreeDsProtocol::V2),
            _ => None,
        });
    let ds_transaction_id = authentication_data
        .ds_trans_id
        .as_deref()
        .and_then(|ds_trans_id| fit_identifier(ds_trans_id, MAX_DS_TRANSACTION_ID))
        .map(Secret::new);

    Ok(Some(WorldpayraftExternalThreeDs {
        ecommerce_indicator,
        cavv,
        protocol,
        ds_transaction_id,
    }))
}

/// `E-commerceData` of an original payment message (Authorize and SetupMandate). External 3DS
/// results set `05`/`06` with the cryptogram (debit carries only the indicator and
/// `3dSecureData`). Without them a plain e-commerce payment is `07`, and the first CIT of a
/// recurring series (a stored credential with MIT category Recurring) is `10`.
fn build_ecommerce_data(
    stores_credential: bool,
    mit_category: Option<&common_enums::MitCategory>,
    ip_address: Option<Secret<String>>,
    merchant_order_id: Option<&str>,
    card_kind: WorldpayraftCardKind,
    external_three_ds: Option<WorldpayraftExternalThreeDs>,
) -> Result<WorldpayraftEcommerceData, error_stack::Report<errors::IntegrationError>> {
    let is_credit = card_kind == WorldpayraftCardKind::Credit;
    // E-commerceOrderNum is maxLength 13 (Native Raft Credit API 1.35.1); a longer merchant
    // reference is refused, never dropped or truncated (TH-10).
    let ecommerce_order_num = merchant_order_id
        .filter(|order_id| is_credit && !order_id.is_empty())
        .map(|order_id| {
            let received_length = order_id.chars().count();
            if received_length <= MAX_ECOMMERCE_ORDER_NUM {
                Ok(order_id.to_string())
            } else {
                Err(error_stack::report!(
                    errors::IntegrationError::MaxFieldLengthViolated {
                        connector: CONNECTOR_NAME.to_string(),
                        field_name: "merchant_order_id".to_string(),
                        max_length: MAX_ECOMMERCE_ORDER_NUM,
                        received_length,
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "E-commerceData.E-commerceOrderNum accepts at most 13 characters"
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                ))
            }
        })
        .transpose()?;
    let (ecommerce_indicator, three_d_secure_data, protocol, ds_transaction_id) =
        match external_three_ds {
            Some(three_ds) => (
                three_ds.ecommerce_indicator,
                Some(three_ds.cavv),
                three_ds.protocol,
                three_ds.ds_transaction_id,
            ),
            None => {
                let indicator = if stores_credential
                    && mit_category == Some(&common_enums::MitCategory::Recurring)
                {
                    WorldpayraftEcommerceIndicator::RecurringFirst
                } else {
                    WorldpayraftEcommerceIndicator::NonAuthenticated
                };
                (indicator, None, None, None)
            }
        };
    Ok(WorldpayraftEcommerceData {
        ecommerce_indicator,
        three_d_secure_data,
        three_d_secure_program_protocol: protocol.filter(|_| is_credit),
        three_d_secure_ds_transaction_id: ds_transaction_id.filter(|_| is_credit),
        ecommerce_ip_address: ip_address.filter(|_| is_credit),
        ecommerce_order_num,
    })
}

fn not_card_error(arm: &str) -> error_stack::Report<errors::IntegrationError> {
    error_stack::report!(errors::IntegrationError::NotImplemented(
        format!("{arm} is not supported by Worldpay Native RAFT card messages"),
        errors::IntegrationErrorContext {
            additional_context: Some(
                "Native RAFT credit/debit messages carry card data only; the separate APM API is not integrated".to_string(),
            ),
            suggested_action: Some("use a card payment method".to_string()),
            ..Default::default()
        },
    ))
}

/// Returns the card, or the documented refusal for every other payment method arm.
fn authorize_card<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<&Card<T>, error_stack::Report<errors::IntegrationError>> {
    match payment_method_data {
        PaymentMethodData::Card(card) => Ok(card),
        PaymentMethodData::CardWithNoCvc(_) => {
            Err(error_stack::report!(errors::IntegrationError::NotImplemented(
                "CardWithNoCvc".to_string(),
                errors::IntegrationErrorContext {
                    additional_context: Some(
                        "HS does not send this carrier for worldpayraft (no HS tokenization for this connector)".to_string(),
                    ),
                    suggested_action: Some("send the card with its CVC".to_string()),
                    ..Default::default()
                },
            )))
        }
        PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
            Err(error_stack::report!(errors::IntegrationError::NotSupported {
                message: "network-transaction-id card on Authorize".to_string(),
                connector: CONNECTOR_NAME,
                context: errors::IntegrationErrorContext {
                    suggested_action: Some("use RecurringPaymentService.Charge".to_string()),
                    additional_context: Some(
                        "merchant-initiated charges with a network transaction id go through RepeatPayment".to_string(),
                    ),
                    ..Default::default()
                },
            }))
        }
        PaymentMethodData::MandatePayment => {
            Err(error_stack::report!(errors::IntegrationError::NotSupported {
                message: "MandatePayment on Authorize".to_string(),
                connector: CONNECTOR_NAME,
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "merchant-initiated charges go through RecurringPaymentService.Charge (RepeatPayment)".to_string(),
                    ),
                    additional_context: Some(
                        "Authorize carries cardholder-initiated card payments only".to_string(),
                    ),
                    ..Default::default()
                },
            }))
        }
        PaymentMethodData::NetworkToken(_) => {
            Err(error_stack::report!(errors::IntegrationError::NotImplemented(
                "network token payments".to_string(),
                errors::IntegrationErrorContext {
                    additional_context: Some(
                        "RAFT accepts a network token in CardInfo.PAN with E-commerceData.PaymentTokenAuthenticationCryptogram; not in this change".to_string(),
                    ),
                    suggested_action: Some("use a card payment method".to_string()),
                    ..Default::default()
                },
            )))
        }
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
            Err(not_card_error("DecryptedWalletTokenDetailsForNetworkTransactionId"))
        }
        PaymentMethodData::CardRedirect(_) => Err(not_card_error("CardRedirect")),
        PaymentMethodData::Wallet(_) => Err(not_card_error("Wallet")),
        PaymentMethodData::PayLater(_) => Err(not_card_error("PayLater")),
        PaymentMethodData::BankRedirect(_) => Err(not_card_error("BankRedirect")),
        PaymentMethodData::BankDebit(_) => Err(not_card_error("BankDebit")),
        PaymentMethodData::BankTransfer(_) => Err(not_card_error("BankTransfer")),
        PaymentMethodData::Crypto(_) => Err(not_card_error("Crypto")),
        PaymentMethodData::Reward => Err(not_card_error("Reward")),
        PaymentMethodData::RealTimePayment(_) => Err(not_card_error("RealTimePayment")),
        PaymentMethodData::Upi(_) => Err(not_card_error("Upi")),
        PaymentMethodData::Voucher(_) => Err(not_card_error("Voucher")),
        PaymentMethodData::GiftCard(_) => Err(not_card_error("GiftCard")),
        PaymentMethodData::PaymentMethodToken(_) => Err(not_card_error("PaymentMethodToken")),
        PaymentMethodData::OpenBanking(_) => Err(not_card_error("OpenBanking")),
        PaymentMethodData::MobilePayment(_) => Err(not_card_error("MobilePayment")),
    }
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftPaymentRequest
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayraftRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayraftPaymentRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: WorldpayraftRouterData<
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

        // G-Authorize-01: one completion per authorization in this integration.
        if matches!(
            request.capture_method,
            Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        ) {
            return Err(error_stack::report!(
                errors::IntegrationError::CaptureMethodNotSupported {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Worldpay RAFT supports one completion per authorization in this integration (Automatic/SequentialAutomatic -> purchase, Manual -> authorization)".to_string(),
                        ),
                        suggested_action: Some(
                            "use capture_method AUTOMATIC or MANUAL".to_string(),
                        ),
                        ..Default::default()
                    },
                }
            ));
        }

        // G-ThreeDS-01..03: validated before any request field is built.
        let external_three_ds = resolve_external_three_ds(
            router_data.resource_common_data.auth_type,
            request.authentication_data.as_ref(),
        )?;

        let card = authorize_card(&request.payment_method_data)?;
        let (card_kind, operation) = select_operation(request);
        let is_credit = card_kind == WorldpayraftCardKind::Credit;
        let stores_credential =
            request.setup_future_usage == Some(common_enums::FutureUsage::OffSession);

        // G-Authorize-02: debit messages cannot carry a credential-on-file setup.
        if card_kind == WorldpayraftCardKind::Debit && stores_credential {
            return Err(error_stack::report!(errors::IntegrationError::NotSupported {
                message: "storing a debit card credential".to_string(),
                connector: CONNECTOR_NAME,
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "debit.yaml has no POSEnvironment and no network transaction ids".to_string(),
                    ),
                    suggested_action: Some(
                        "store the credential with a credit card, or pay without setup_future_usage OFF_SESSION".to_string(),
                    ),
                    ..Default::default()
                },
            }));
        }

        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires the payment amount in major currency units (ddddddddd.cc)".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let expiration_date = card.get_expiry_date_as_yymm().change_context(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "card.card_exp_year / card.card_exp_month",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT expects card expiry in YYMM format".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        // CVV: indicator "1" + value only when a CVC was collected; debitpreauth has no
        // CardVerificationData object.
        let card_verification_data = (!card.card_cvc.peek().is_empty()
            && (card_kind != WorldpayraftCardKind::Debit
                || operation == WorldpayraftOperation::Purchase))
            .then(|| WorldpayraftCardVerificationData {
                cvv_indicator: WorldpayraftCvvIndicator::Present,
                cvv_value: Some(card.card_cvc.clone()),
            });

        let addresses = build_addresses(
            router_data.resource_common_data.get_optional_billing(),
            router_data.resource_common_data.get_optional_shipping(),
            card_kind,
        )?;
        let (soft_descriptor_data, merchant_specific_data) =
            build_descriptor_and_merchant_data(request, card_kind);
        let level2_level3 = build_level2_level3(&item, card_kind)?;

        let pos_condition_code = if operation == WorldpayraftOperation::Verification {
            WorldpayraftPosConditionCode::VerificationOnly
        } else {
            WorldpayraftPosConditionCode::ElectronicCommerce
        };
        let terminal_data = WorldpayraftTerminalData {
            entry_mode: WorldpayraftEntryMode::ECommerce,
            terminal_type: Some(WorldpayraftTerminalType::Internet),
            pos_condition_code,
            terminal_entry_cap: WorldpayraftTerminalEntryCap::Unspecified,
            pos_environment: (is_credit && stores_credential)
                .then_some(WorldpayraftPosEnvironment::CredentialOnFile),
        };

        let sends_cit_flag = is_credit || operation == WorldpayraftOperation::Purchase;
        let proc_flags_indicators = WorldpayraftProcFlags {
            cardholder_initiated_transaction: sends_cit_flag.then_some(WorldpayraftYesNo::Yes),
            mastercard_advice_code_indicator: is_credit.then_some(WorldpayraftYesNo::Yes),
            partial_allowed: request.enable_partial_authorization.map(|allowed| {
                if allowed {
                    WorldpayraftYesNo::Yes
                } else {
                    WorldpayraftYesNo::No
                }
            }),
            event_notification_indicator: Some(EVENT_NOTIFICATION_OPT_IN),
            ..Default::default()
        };

        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;
        let api_transaction_id = derive_api_transaction_id(reference).change_context(
            errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "failed to derive the 16-digit APITransactionID from connector_request_reference_id".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        let reference_trace_numbers = WorldpayraftRequestTraceNumbers {
            // CorrelationID is optional, echoed, and used by Worldpay only for reporting and
            // research (tech spec: Merchant reference id / order id): the reference is sent when
            // it fits 25 characters and omitted otherwise. Nothing correlates on it (plan o-audit).
            correlation_id: fit_identifier(reference, MAX_CORRELATION_ID),
            ref_invoice_number: level2_level3.ref_invoice_number,
            ..Default::default()
        };
        // UserData1 surfaces as customerFields.field1 in Event Notifications, which carry no
        // APITransactionID: sending the APITransactionID of this message (16 digits, always fits
        // 35) makes the webhook join key equal the reported connector_transaction_id and never
        // absent, whatever the length of the UCS reference.
        let user_data_1 = fit_identifier(&api_transaction_id, MAX_USER_DATA_1);

        let inner = WorldpayraftPaymentInner {
            misc_amounts_balances: WorldpayraftAmounts {
                transaction_amount,
                preauthorized_amount: None,
                sales_tax_amount: level2_level3.sales_tax_amount,
                invoice_discount_amount: level2_level3.invoice_discount_amount,
                invoice_shipping_amount: level2_level3.invoice_shipping_amount,
            },
            card_info: WorldpayraftCardInfo {
                pan: Some(card.card_number.clone()),
                expiration_date: Some(expiration_date),
            },
            card_verification_data,
            address_verification_data: addresses.address_verification_data,
            online_bill_to_address: addresses.online_bill_to_address,
            online_ship_to_address: addresses.online_ship_to_address,
            customer_information: addresses.customer_information,
            terminal_data,
            ecommerce_data: build_ecommerce_data(
                stores_credential,
                request.mit_category.as_ref(),
                request
                    .get_ip_address_as_optional()
                    .map(|ip| Secret::new(ip.expose())),
                request.merchant_order_id.as_deref(),
                card_kind,
                external_three_ds,
            )?,
            proc_flags_indicators,
            // WPTokenRequested is declared on creditpurchase/creditauth and on
            // debitpurchase/debitpreauth (debit.yaml); both return TokenizedPAN.
            encryption_token_data: Some(WorldpayraftEncryptionTokenRequest {
                tokenized_pan: None,
                wp_token_requested: Some(WorldpayraftYesNo::Yes),
            }),
            reference_trace_numbers: (!reference_trace_numbers.is_empty())
                .then_some(reference_trace_numbers),
            user_defined_data: user_data_1.map(|user_data_1| WorldpayraftUserDefinedData {
                user_data_1: Some(user_data_1),
            }),
            soft_descriptor_data,
            merchant_specific_data,
            level3_data: level2_level3.level3_data,
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id,
            local_date_time: local_date_time(),
        };

        Ok(match (card_kind, operation) {
            (WorldpayraftCardKind::Credit, WorldpayraftOperation::Purchase) => {
                Self::CreditPurchase {
                    creditpurchase: inner,
                }
            }
            (
                WorldpayraftCardKind::Credit,
                WorldpayraftOperation::Authorization | WorldpayraftOperation::Verification,
            ) => Self::CreditAuth { creditauth: inner },
            (WorldpayraftCardKind::Debit, WorldpayraftOperation::Purchase) => Self::DebitPurchase {
                debitpurchase: inner,
            },
            (
                WorldpayraftCardKind::Debit,
                WorldpayraftOperation::Authorization | WorldpayraftOperation::Verification,
            ) => Self::DebitPreauth {
                debitpreauth: inner,
            },
        })
    }
}

// =============================================================================
// AUTHORIZE RESPONSE
// =============================================================================

/// Response of an original payment message, keyed by its operation wrapper.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftPaymentResponse {
    CreditPurchase {
        creditpurchaseresponse: WorldpayraftResponseInner,
    },
    CreditAuth {
        creditauthresponse: WorldpayraftResponseInner,
    },
    DebitPurchase {
        debitpurchaseresponse: WorldpayraftResponseInner,
    },
    DebitPreauth {
        debitpreauthresponse: WorldpayraftResponseInner,
    },
}

impl WorldpayraftPaymentResponse {
    pub fn inner(&self) -> &WorldpayraftResponseInner {
        match self {
            Self::CreditPurchase {
                creditpurchaseresponse,
            } => creditpurchaseresponse,
            Self::CreditAuth { creditauthresponse } => creditauthresponse,
            Self::DebitPurchase {
                debitpurchaseresponse,
            } => debitpurchaseresponse,
            Self::DebitPreauth {
                debitpreauthresponse,
            } => debitpreauthresponse,
        }
    }
}

// =============================================================================
// TryFrom: WorldpayraftPaymentResponse → RouterDataV2
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<WorldpayraftPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let inner = item.response.inner();
        let request = &item.router_data.request;
        let (card_kind, operation) = select_operation(request);
        let decision = classify(inner);

        let partial_amount = match decision {
            // ResponseCode 010: the approved amount IS MiscAmountsBalances.OriginalAuthAmount
            // (tech spec Status Mappings). Without it the approved amount is unknown: fail closed,
            // never record the requested amount as authorized (INV-17).
            WorldpayraftDecision::PartialApproval => {
                let original_auth_amount = inner
                    .misc_amounts_balances
                    .as_ref()
                    .and_then(|amounts| amounts.original_auth_amount.clone())
                    .ok_or_else(|| {
                        error_stack::report!(errors::ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some("partial approval (ResponseCode 010) without MiscAmountsBalances.OriginalAuthAmount: the approved amount is unknown".to_string()),
                        ))
                    })?;
                Some(
                    common_utils::types::StringMajorUnitForConnector
                        .convert_back(original_auth_amount, request.currency)
                        .change_context(errors::ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some("could not convert MiscAmountsBalances.OriginalAuthAmount of a partial approval".to_string()),
                        ))?,
                )
            }
            WorldpayraftDecision::Approved
            | WorldpayraftDecision::HonorWithId
            | WorldpayraftDecision::RequestInProgress
            | WorldpayraftDecision::Declined
            | WorldpayraftDecision::MessageRejected => None,
        };

        let status = match decision {
            WorldpayraftDecision::Approved => match operation {
                WorldpayraftOperation::Purchase | WorldpayraftOperation::Verification => {
                    AttemptStatus::Charged
                }
                WorldpayraftOperation::Authorization => AttemptStatus::Authorized,
            },
            WorldpayraftDecision::PartialApproval => match operation {
                WorldpayraftOperation::Purchase | WorldpayraftOperation::Verification => {
                    AttemptStatus::PartialCharged
                }
                WorldpayraftOperation::Authorization => AttemptStatus::PartiallyAuthorized,
            },
            WorldpayraftDecision::HonorWithId => AttemptStatus::Unresolved,
            WorldpayraftDecision::RequestInProgress => AttemptStatus::Pending,
            WorldpayraftDecision::Declined | WorldpayraftDecision::MessageRejected => {
                AttemptStatus::Failure
            }
        };

        if matches!(
            decision,
            WorldpayraftDecision::Declined | WorldpayraftDecision::MessageRejected
        ) {
            return Ok(Self {
                response: Err(build_business_error(
                    inner,
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // connector_transaction_id is the APITransactionID that was SENT: follow-ups must reuse it.
        let api_transaction_id = derive_api_transaction_id(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        )
        .change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("failed to recompute the APITransactionID sent on Authorize".to_string()),
            ),
        )?;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => Some(card),
            _ => None,
        };
        let expiration_date = card.and_then(|card| card.get_expiry_date_as_yymm().ok());
        let card_network = card.and_then(card_brand);
        let tokenized_pan = inner.tokenized_pan();
        let network_transaction_ids = inner.network_ids();

        let feature_data = WorldpayraftFeatureData {
            api_transaction_id: api_transaction_id.clone(),
            operation,
            card_kind,
            authorized_amount: partial_amount.unwrap_or(request.minor_amount),
            captured_amount: None,
            currency: request.currency,
            tokenized_pan: tokenized_pan.clone(),
            expiration_date: expiration_date.clone(),
            authorization_number: inner.authorization_number(),
            retrieval_ref_number: inner
                .reference_trace_numbers
                .as_ref()
                .and_then(|trace| trace.retrieval_ref_number.clone()),
            card_network,
            network_transaction_ids: network_transaction_ids.clone(),
        };
        let connector_metadata = serde_json::to_value(&feature_data).change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("failed to serialize Worldpay RAFT connector_feature_data".to_string()),
            ),
        )?;

        let stores_credential =
            request.setup_future_usage == Some(common_enums::FutureUsage::OffSession);
        // A CIT approved without a TokenizedPAN has still moved money, so the payment result is
        // kept (reporting a failure would invite a second charge) and only the mandate is not
        // stored (plan o-audit, P-Authorize-17). SetupMandate fails the same case, because
        // nothing was charged there.
        if stores_credential && tokenized_pan.is_none() {
            tracing::warn!(
                connector = CONNECTOR_NAME,
                "CIT approved without TokenizedPAN; mandate not stored"
            );
        }
        // A CIT stores the NTIDs of this message alongside the token, so a later MIT can echo
        // them (tech spec: What UCS MUST persist).
        let mandate_reference = tokenized_pan.filter(|_| stores_credential).map(|token| {
            Box::new(MandateReference {
                connector_mandate_id: Some(token.expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: Some(build_mandate_metadata(
                    expiration_date.as_ref(),
                    card.and_then(card_network_of),
                    network_transaction_ids.as_ref(),
                )),
            })
        });

        // A purchase moves money: a partial approval reports the approved amount as both authorized
        // and captured, and a full approval reports the requested amount as captured. An
        // authorization reports only the authorized amount.
        let (minor_amount_authorized, captured_amount) = match (operation, decision) {
            (WorldpayraftOperation::Authorization, _) => (
                partial_amount.or(item
                    .router_data
                    .resource_common_data
                    .minor_amount_authorized),
                None,
            ),
            (WorldpayraftOperation::Purchase, WorldpayraftDecision::PartialApproval) => (
                partial_amount.or(item
                    .router_data
                    .resource_common_data
                    .minor_amount_authorized),
                partial_amount,
            ),
            (WorldpayraftOperation::Purchase, WorldpayraftDecision::Approved) => (
                item.router_data
                    .resource_common_data
                    .minor_amount_authorized,
                Some(request.minor_amount),
            ),
            (
                WorldpayraftOperation::Purchase,
                WorldpayraftDecision::HonorWithId
                | WorldpayraftDecision::RequestInProgress
                | WorldpayraftDecision::Declined
                | WorldpayraftDecision::MessageRejected,
            )
            | (WorldpayraftOperation::Verification, _) => (
                item.router_data
                    .resource_common_data
                    .minor_amount_authorized,
                None,
            ),
        };
        let minor_amount_captured =
            captured_amount.or(item.router_data.resource_common_data.minor_amount_captured);
        let amount_captured = captured_amount
            .map(MinorUnit::get_amount_as_i64)
            .or(item.router_data.resource_common_data.amount_captured);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(api_transaction_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: Some(connector_metadata),
                network_txn_id: inner.network_transaction_id(),
                network_txn_link_id: inner
                    .reference_trace_numbers
                    .as_ref()
                    .and_then(|trace| trace.transaction_link_id.clone()),
                connector_response_reference_id: Some(api_transaction_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: inner
                    .reference_trace_numbers
                    .as_ref()
                    .and_then(|trace| trace.payment_acct_ref_number.clone()),
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response: inner.card_connector_response(),
                minor_amount_authorized,
                minor_amount_captured,
                amount_captured,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// CAPTURE REQUEST
// =============================================================================

/// Reads the Authorize response's `WorldpayraftFeatureData` back from a Capture request.
///
/// `PreauthorizedAmount` (required on every completion) and the credit/debit route exist only
/// in the Authorize response, so a Capture without them is refused before any request is built.
/// Shared by `get_url` and the request builder so the endpoint and the wrapper key cannot
/// disagree.
pub(super) fn capture_feature_data(
    request: &PaymentsCaptureData,
) -> Result<WorldpayraftFeatureData, error_stack::Report<errors::IntegrationError>> {
    crate::utils::to_connector_meta_from_secret::<WorldpayraftFeatureData>(
        request.connector_feature_data.clone(),
    )
    .change_context(errors::IntegrationError::MissingRequiredField {
        field_name: "connector_feature_data",
        context: errors::IntegrationErrorContext {
            additional_context: Some(
                "PreauthorizedAmount (required on completion) and the credit/debit route come from the Authorize response".to_string(),
            ),
            suggested_action: Some(
                "send the connector_feature_data returned by the Worldpay RAFT Authorize response unchanged".to_string(),
            ),
            ..Default::default()
        },
    })
}

/// URL path of the completion message for the API the original payment used.
pub fn completion_path(card_kind: WorldpayraftCardKind) -> &'static str {
    match card_kind {
        WorldpayraftCardKind::Credit => "credit/completion",
        WorldpayraftCardKind::Debit => "debit/completion",
    }
}

/// Body shared by `creditcompletion` and `debitcompletion`.
#[derive(Debug, Serialize)]
pub struct WorldpayraftCompletionInner {
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(rename = "AuthorizationType")]
    pub authorization_type: WorldpayraftAuthorizationType,
    #[serde(rename = "ProcFlagsIndicators")]
    pub proc_flags_indicators: WorldpayraftProcFlags,
    #[serde(
        rename = "ReferenceTraceNumbers",
        skip_serializing_if = "Option::is_none"
    )]
    pub reference_trace_numbers: Option<WorldpayraftRequestTraceNumbers>,
    #[serde(
        rename = "EncryptionTokenData",
        skip_serializing_if = "Option::is_none"
    )]
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenRequest>,
    #[serde(rename = "CardInfo", skip_serializing_if = "Option::is_none")]
    pub card_info: Option<WorldpayraftFollowUpCardInfo>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
    pub local_date_time: String,
}

/// Completion message, keyed by its operation wrapper.
///
/// `/credit/completion` → `creditcompletion`, `/debit/completion` → `debitcompletion`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftCaptureRequest {
    Credit {
        creditcompletion: WorldpayraftCompletionInner,
    },
    Debit {
        debitcompletion: WorldpayraftCompletionInner,
    },
}

// =============================================================================
// CAPTURE RESPONSE
// =============================================================================

/// Completion response, keyed by its operation wrapper.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftCaptureResponse {
    Credit {
        creditcompletionresponse: WorldpayraftResponseInner,
    },
    Debit {
        debitcompletionresponse: WorldpayraftResponseInner,
    },
}

impl WorldpayraftCaptureResponse {
    pub fn inner(&self) -> &WorldpayraftResponseInner {
        match self {
            Self::Credit {
                creditcompletionresponse,
            } => creditcompletionresponse,
            Self::Debit {
                debitcompletionresponse,
            } => debitcompletionresponse,
        }
    }
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftCaptureRequest
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayraftRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for WorldpayraftCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: WorldpayraftRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        // The ORIGINAL APITransactionID: authorization and completion are one coded sequence.
        let api_transaction_id = request.get_connector_transaction_id()?;
        validate_api_transaction_id(&api_transaction_id)?;
        let feature_data = capture_feature_data(request)?;
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount_to_capture, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires MiscAmountsBalances.TransactionAmount in major currency units (ddddddddd.cc)"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let preauthorized_amount = item
            .connector
            .amount_converter
            .convert(feature_data.authorized_amount, feature_data.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires MiscAmountsBalances.PreauthorizedAmount in major currency units (ddddddddd.cc)"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let reference_trace_numbers = WorldpayraftRequestTraceNumbers {
            authorization_number: feature_data.authorization_number.clone(),
            retrieval_ref_number: feature_data.retrieval_ref_number.clone(),
            ..Default::default()
        };

        // Card identity by token: sent only when the Authorize response returned one.
        let (encryption_token_data, card_info) = match feature_data.tokenized_pan {
            Some(tokenized_pan) => (
                Some(WorldpayraftEncryptionTokenRequest {
                    tokenized_pan: Some(tokenized_pan),
                    wp_token_requested: None,
                }),
                feature_data
                    .expiration_date
                    .map(|expiration_date| WorldpayraftFollowUpCardInfo { expiration_date }),
            ),
            None => (None, None),
        };

        let inner = WorldpayraftCompletionInner {
            misc_amounts_balances: WorldpayraftAmounts {
                preauthorized_amount: Some(preauthorized_amount),
                ..WorldpayraftAmounts::transaction_only(transaction_amount)
            },
            authorization_type: WorldpayraftAuthorizationType::ForcePost,
            proc_flags_indicators: WorldpayraftProcFlags {
                prior_auth: Some(WorldpayraftYesNo::Yes),
                ..Default::default()
            },
            reference_trace_numbers: (!reference_trace_numbers.is_empty())
                .then_some(reference_trace_numbers),
            encryption_token_data,
            card_info,
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id,
            local_date_time: local_date_time(),
        };

        Ok(match feature_data.card_kind {
            WorldpayraftCardKind::Credit => Self::Credit {
                creditcompletion: inner,
            },
            WorldpayraftCardKind::Debit => Self::Debit {
                debitcompletion: inner,
            },
        })
    }
}

// =============================================================================
// TryFrom: WorldpayraftCaptureResponse → RouterDataV2
// =============================================================================

impl TryFrom<ResponseRouterData<WorldpayraftCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let inner = item.response.inner();
        let status = match classify(inner) {
            WorldpayraftDecision::Approved => AttemptStatus::Charged,
            WorldpayraftDecision::RequestInProgress => AttemptStatus::Pending,
            // PartialApproval and HonorWithId are not documented on completion responses; a
            // completion has no partial or manual-review outcome, so both fail the capture.
            WorldpayraftDecision::PartialApproval
            | WorldpayraftDecision::HonorWithId
            | WorldpayraftDecision::Declined
            | WorldpayraftDecision::MessageRejected => {
                return Ok(Self {
                    response: Err(build_business_error(
                        inner,
                        item.http_code,
                        Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
                    )),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::CaptureFailed,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                });
            }
        };

        // One id per payment: the completion reuses the original APITransactionID. The feature
        // data is handed back with the captured amount, so a later Refund keeps the token and is
        // bounded by what was captured (G-Authorize-09).
        let resource_id = item.router_data.request.connector_transaction_id.clone();
        let connector_metadata = item
            .router_data
            .request
            .connector_feature_data
            .clone()
            .map(|feature_data| {
                let mut feature_data = crate::utils::to_connector_meta_from_secret::<
                    WorldpayraftFeatureData,
                >(Some(feature_data))
                .change_context(
                    errors::ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("could not parse connector_feature_data".to_string()),
                    ),
                )?;
                feature_data.captured_amount =
                    Some(item.router_data.request.minor_amount_to_capture);
                serde_json::to_value(&feature_data).change_context(
                    errors::ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("could not serialize connector_feature_data".to_string()),
                    ),
                )
            })
            .transpose()?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: inner.api_transaction_id.clone(),
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
// VOID REQUEST
// =============================================================================

/// Reads the Authorize response's `WorldpayraftFeatureData` back from a Void request.
///
/// RAFT has no void endpoint: a reversal re-sends the original message (same path, same wrapper)
/// with `AuthorizationType` RV, the original `APITransactionID` and the original amount. The
/// operation, card kind and amount exist only in the Authorize response, so a Void without them
/// is refused before any request is built. Shared by `get_url` and the request builder so the
/// endpoint and the wrapper key cannot disagree.
pub(super) fn void_feature_data(
    request: &PaymentVoidData,
) -> Result<WorldpayraftFeatureData, error_stack::Report<errors::IntegrationError>> {
    crate::utils::to_connector_meta_from_secret::<WorldpayraftFeatureData>(
        request.connector_feature_data.clone(),
    )
    .change_context(errors::IntegrationError::MissingRequiredField {
        field_name: "connector_feature_data",
        context: errors::IntegrationErrorContext {
            additional_context: Some(
                "the reversal must replay the original operation, amount and card kind from the Authorize response".to_string(),
            ),
            suggested_action: Some(
                "send the connector_feature_data returned by the Worldpay RAFT Authorize response unchanged".to_string(),
            ),
            ..Default::default()
        },
    })
}

/// Body of a reversal: the original message re-sent with `AuthorizationType` RV.
#[derive(Debug, Serialize)]
pub struct WorldpayraftReversalInner {
    #[serde(rename = "AuthorizationType")]
    pub authorization_type: WorldpayraftAuthorizationType,
    #[serde(rename = "ReversalAdviceReasonCd")]
    pub reversal_reason: WorldpayraftReversalAdviceReasonCode,
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(rename = "ProcFlagsIndicators")]
    pub proc_flags_indicators: WorldpayraftProcFlags,
    #[serde(
        rename = "ReferenceTraceNumbers",
        skip_serializing_if = "Option::is_none"
    )]
    pub reference_trace_numbers: Option<WorldpayraftRequestTraceNumbers>,
    #[serde(
        rename = "EncryptionTokenData",
        skip_serializing_if = "Option::is_none"
    )]
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenRequest>,
    #[serde(rename = "CardInfo", skip_serializing_if = "Option::is_none")]
    pub card_info: Option<WorldpayraftFollowUpCardInfo>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
    pub local_date_time: String,
}

/// Reversal message, keyed by the original message's operation wrapper.
///
/// `/credit/authorization` → `creditauth`, `/credit/purchase` → `creditpurchase`,
/// `/debit/preauth` → `debitpreauth`, `/debit/purchase` → `debitpurchase`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftVoidRequest {
    CreditAuth {
        creditauth: WorldpayraftReversalInner,
    },
    CreditPurchase {
        creditpurchase: WorldpayraftReversalInner,
    },
    DebitPreauth {
        debitpreauth: WorldpayraftReversalInner,
    },
    DebitPurchase {
        debitpurchase: WorldpayraftReversalInner,
    },
}

// =============================================================================
// VOID RESPONSE
// =============================================================================

/// Reversal response, keyed by the original message's operation wrapper.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftVoidResponse {
    CreditAuth {
        creditauthresponse: WorldpayraftResponseInner,
    },
    CreditPurchase {
        creditpurchaseresponse: WorldpayraftResponseInner,
    },
    DebitPreauth {
        debitpreauthresponse: WorldpayraftResponseInner,
    },
    DebitPurchase {
        debitpurchaseresponse: WorldpayraftResponseInner,
    },
}

impl WorldpayraftVoidResponse {
    pub fn inner(&self) -> &WorldpayraftResponseInner {
        match self {
            Self::CreditAuth { creditauthresponse } => creditauthresponse,
            Self::CreditPurchase {
                creditpurchaseresponse,
            } => creditpurchaseresponse,
            Self::DebitPreauth {
                debitpreauthresponse,
            } => debitpreauthresponse,
            Self::DebitPurchase {
                debitpurchaseresponse,
            } => debitpurchaseresponse,
        }
    }
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftVoidRequest
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayraftRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for WorldpayraftVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: WorldpayraftRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        validate_api_transaction_id(&request.connector_transaction_id)?;
        let feature_data = void_feature_data(request)?;
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        // Full reversal: TransactionAmount is the ORIGINAL amount, from the Authorize response
        // only (DispensedAmount, the partial-reversal field, is not sent).
        let transaction_amount = item
            .connector
            .amount_converter
            .convert(feature_data.authorized_amount, feature_data.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires MiscAmountsBalances.TransactionAmount in major currency units (ddddddddd.cc)"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let reversal_reason = match request.cancellation_reason {
            Some(_) => WorldpayraftReversalAdviceReasonCode::CustomerCancel,
            None => WorldpayraftReversalAdviceReasonCode::NormalReversal,
        };

        let reference_trace_numbers = WorldpayraftRequestTraceNumbers {
            authorization_number: feature_data.authorization_number.clone(),
            retrieval_ref_number: feature_data.retrieval_ref_number.clone(),
            ..Default::default()
        };

        // Card identity by token: sent only when the Authorize response returned one.
        let (encryption_token_data, card_info) = match feature_data.tokenized_pan {
            Some(tokenized_pan) => (
                Some(WorldpayraftEncryptionTokenRequest {
                    tokenized_pan: Some(tokenized_pan),
                    wp_token_requested: None,
                }),
                feature_data
                    .expiration_date
                    .map(|expiration_date| WorldpayraftFollowUpCardInfo { expiration_date }),
            ),
            None => (None, None),
        };

        let inner = WorldpayraftReversalInner {
            authorization_type: WorldpayraftAuthorizationType::Reversal,
            reversal_reason,
            misc_amounts_balances: WorldpayraftAmounts::transaction_only(transaction_amount),
            proc_flags_indicators: WorldpayraftProcFlags {
                prior_auth: Some(WorldpayraftYesNo::Yes),
                ..Default::default()
            },
            reference_trace_numbers: (!reference_trace_numbers.is_empty())
                .then_some(reference_trace_numbers),
            encryption_token_data,
            card_info,
            world_pay_merchant_id: auth.merchant_id,
            // The ORIGINAL APITransactionID: RAFT matches the reversal to the original by it.
            api_transaction_id: request.connector_transaction_id.clone(),
            local_date_time: local_date_time(),
        };

        Ok(match (feature_data.card_kind, feature_data.operation) {
            (
                WorldpayraftCardKind::Credit,
                WorldpayraftOperation::Authorization | WorldpayraftOperation::Verification,
            ) => Self::CreditAuth { creditauth: inner },
            (WorldpayraftCardKind::Credit, WorldpayraftOperation::Purchase) => {
                Self::CreditPurchase {
                    creditpurchase: inner,
                }
            }
            (
                WorldpayraftCardKind::Debit,
                WorldpayraftOperation::Authorization | WorldpayraftOperation::Verification,
            ) => Self::DebitPreauth {
                debitpreauth: inner,
            },
            (WorldpayraftCardKind::Debit, WorldpayraftOperation::Purchase) => Self::DebitPurchase {
                debitpurchase: inner,
            },
        })
    }
}

// =============================================================================
// TryFrom: WorldpayraftVoidResponse → RouterDataV2
// =============================================================================

impl TryFrom<ResponseRouterData<WorldpayraftVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let inner = item.response.inner();
        let status = match classify(inner) {
            WorldpayraftDecision::Approved => AttemptStatus::Voided,
            WorldpayraftDecision::RequestInProgress => AttemptStatus::Pending,
            // A reversal has no partial or manual-review outcome; 002 VOID UNSUCCESSFUL,
            // 037 DUPLICATE REVERSAL, 051 no matching original and every other code fail it.
            WorldpayraftDecision::PartialApproval
            | WorldpayraftDecision::HonorWithId
            | WorldpayraftDecision::Declined
            | WorldpayraftDecision::MessageRejected => {
                return Ok(Self {
                    response: Err(build_business_error(
                        inner,
                        item.http_code,
                        Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                    )),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::VoidFailed,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                });
            }
        };

        // The reversal reuses the original APITransactionID: one id per payment.
        let resource_id = ResponseId::ConnectorTransactionId(
            item.router_data.request.connector_transaction_id.clone(),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: inner.api_transaction_id.clone(),
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
// REFUND REQUEST
// =============================================================================

/// Reads the original charge's `WorldpayraftFeatureData` back from a Refund request and returns
/// it with the card token, when the charge returned one.
///
/// The credit/debit route exists only in the feature data, so a Refund without it is refused
/// before any request is built. A credit refund is an independent credit that must identify the
/// card, and the only card identity UCS holds after the charge is the `TokenizedPAN` of the
/// Authorize/Capture response, so a credit refund without it is refused too. `debitrefund`
/// requires only `WorldPayMerchantID`, `APITransactionID` and `LocalDateTime`: a debit refund
/// sends the token when it is present and is otherwise linked to the charge by
/// `ReferenceTraceNumbers` alone. Shared by `get_url` and the request builder so the endpoint and
/// the wrapper key cannot disagree.
pub(super) fn refund_feature_data(
    request: &RefundsData,
) -> Result<
    (WorldpayraftFeatureData, Option<Secret<String>>),
    error_stack::Report<errors::IntegrationError>,
> {
    let missing_token = || {
        errors::IntegrationError::MissingRequiredField {
        field_name: "connector_feature_data.tokenized_pan",
        context: errors::IntegrationErrorContext {
            additional_context: Some(
                "a RAFT refund is an independent credit and must identify the card; the token comes from the original charge response".to_string(),
            ),
            suggested_action: Some(
                "refund a payment authorized through this connector with WPTokenRequested".to_string(),
            ),
            ..Default::default()
        },
    }
    };
    let feature_data = crate::utils::to_connector_meta_from_secret::<WorldpayraftFeatureData>(
        request.connector_feature_data.clone(),
    )
    .change_context(missing_token())?;
    let tokenized_pan = match (feature_data.card_kind, feature_data.tokenized_pan.clone()) {
        (WorldpayraftCardKind::Credit, None) => {
            return Err(error_stack::report!(missing_token()));
        }
        (_, tokenized_pan) => tokenized_pan,
    };
    Ok((feature_data, tokenized_pan))
}

/// URL path of the refund message for the API the original payment used.
pub fn refund_path(card_kind: WorldpayraftCardKind) -> &'static str {
    match card_kind {
        WorldpayraftCardKind::Credit => "credit/refund",
        WorldpayraftCardKind::Debit => "debit/refund",
    }
}

/// Body shared by `creditrefund` and `debitrefund`. `PreauthorizedAmount` is not part of the
/// refund `MiscAmountsBalances`, and `SystemTraceNumber` is response-only, so neither is sent.
#[derive(Debug, Serialize)]
pub struct WorldpayraftRefundInner {
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: WorldpayraftAmounts,
    /// Always present on `creditrefund`; on `debitrefund` only when the charge returned a token.
    #[serde(
        rename = "EncryptionTokenData",
        skip_serializing_if = "Option::is_none"
    )]
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenRequest>,
    #[serde(rename = "CardInfo", skip_serializing_if = "Option::is_none")]
    pub card_info: Option<WorldpayraftFollowUpCardInfo>,
    #[serde(
        rename = "ProcFlagsIndicators",
        skip_serializing_if = "Option::is_none"
    )]
    pub proc_flags_indicators: Option<WorldpayraftProcFlags>,
    #[serde(
        rename = "ReferenceTraceNumbers",
        skip_serializing_if = "Option::is_none"
    )]
    pub reference_trace_numbers: Option<WorldpayraftRequestTraceNumbers>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
    pub local_date_time: String,
}

/// Refund message, keyed by its operation wrapper.
///
/// `/credit/refund` → `creditrefund`, `/debit/refund` → `debitrefund`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftRefundRequest {
    Credit {
        creditrefund: WorldpayraftRefundInner,
    },
    Debit {
        debitrefund: WorldpayraftRefundInner,
    },
}

// =============================================================================
// REFUND RESPONSE
// =============================================================================

/// Refund response, keyed by its operation wrapper.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftRefundResponse {
    Credit {
        creditrefundresponse: WorldpayraftResponseInner,
    },
    Debit {
        debitrefundresponse: WorldpayraftResponseInner,
    },
}

impl WorldpayraftRefundResponse {
    pub fn inner(&self) -> &WorldpayraftResponseInner {
        match self {
            Self::Credit {
                creditrefundresponse,
            } => creditrefundresponse,
            Self::Debit {
                debitrefundresponse,
            } => debitrefundresponse,
        }
    }
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftRefundRequest
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayraftRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for WorldpayraftRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: WorldpayraftRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let (feature_data, tokenized_pan) = refund_feature_data(request)?;

        // A RAFT refund is an unlinked credit: Worldpay does not check it against the charge, so
        // these checks are made here, before any request is built.
        // G-Authorize-08: a zero-amount verification moved no money and has nothing to refund.
        if feature_data.operation == WorldpayraftOperation::Verification {
            return Err(error_stack::report!(errors::IntegrationError::NotSupported {
                message: "refund of a zero-amount verification".to_string(),
                connector: CONNECTOR_NAME,
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "the original message was a verification (POSConditionCode 51) that charged nothing".to_string(),
                    ),
                    suggested_action: Some(
                        "refund only a payment that was charged or captured".to_string(),
                    ),
                    ..Default::default()
                },
            }));
        }
        // G-Authorize-09: never credit more than was captured (the authorized amount of a
        // purchase, or the captured amount after a Capture).
        let refundable_amount = feature_data
            .captured_amount
            .unwrap_or(feature_data.authorized_amount);
        if request.minor_refund_amount > refundable_amount {
            return Err(error_stack::report!(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "refund_amount",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "exceeds the captured amount; a RAFT refund is an unlinked credit"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "refund at most the captured amount of the original payment"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }
            ));
        }

        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        // Partial refund is a smaller TransactionAmount: no flag, no computed amount.
        let transaction_amount = item
            .connector
            .amount_converter
            .convert(request.minor_refund_amount, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires MiscAmountsBalances.TransactionAmount in major currency units (ddddddddd.cc)"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        // A NEW id per refund: a refund is not a coded sequence of the charge, so reusing the
        // charge's APITransactionID would replay the charge's idempotent response.
        let api_transaction_id = derive_api_transaction_id(&request.refund_id).change_context(
            errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "failed to derive the 16-digit APITransactionID from refund_id".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        // Secondary link keys to the original charge.
        let reference_trace_numbers = WorldpayraftRequestTraceNumbers {
            authorization_number: feature_data.authorization_number.clone(),
            retrieval_ref_number: feature_data.retrieval_ref_number.clone(),
            ..Default::default()
        };

        // PriorAuth Y is the reference integration's credit refund; it is not sent on debit.
        let proc_flags_indicators = match feature_data.card_kind {
            WorldpayraftCardKind::Credit => Some(WorldpayraftProcFlags {
                prior_auth: Some(WorldpayraftYesNo::Yes),
                ..Default::default()
            }),
            WorldpayraftCardKind::Debit => None,
        };

        let inner = WorldpayraftRefundInner {
            misc_amounts_balances: WorldpayraftAmounts::transaction_only(transaction_amount),
            encryption_token_data: tokenized_pan.map(|tokenized_pan| {
                WorldpayraftEncryptionTokenRequest {
                    tokenized_pan: Some(tokenized_pan),
                    wp_token_requested: None,
                }
            }),
            card_info: feature_data
                .expiration_date
                .clone()
                .map(|expiration_date| WorldpayraftFollowUpCardInfo { expiration_date }),
            proc_flags_indicators,
            reference_trace_numbers: (!reference_trace_numbers.is_empty())
                .then_some(reference_trace_numbers),
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id,
            local_date_time: local_date_time(),
        };

        Ok(match feature_data.card_kind {
            WorldpayraftCardKind::Credit => Self::Credit {
                creditrefund: inner,
            },
            WorldpayraftCardKind::Debit => Self::Debit { debitrefund: inner },
        })
    }
}

// =============================================================================
// TryFrom: WorldpayraftRefundResponse → RouterDataV2
// =============================================================================

impl TryFrom<ResponseRouterData<WorldpayraftRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let inner = item.response.inner();
        let refund_status = match classify(inner) {
            WorldpayraftDecision::Approved => RefundStatus::Success,
            WorldpayraftDecision::RequestInProgress => RefundStatus::Pending,
            // A refund has no partial or manual-review outcome: 010, 003, every other
            // ResponseCode and every non-0000 ReturnCode fail it.
            WorldpayraftDecision::PartialApproval
            | WorldpayraftDecision::HonorWithId
            | WorldpayraftDecision::Declined
            | WorldpayraftDecision::MessageRejected => {
                return Ok(Self {
                    response: Err(build_business_error(
                        inner,
                        item.http_code,
                        Some(FlowStatus::Refund(RefundStatus::Failure)),
                    )),
                    resource_common_data: RefundFlowData {
                        status: RefundStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                });
            }
        };

        // connector_refund_id is the refund message's own APITransactionID, the one that was
        // SENT (never the 6-character, non-unique AuthorizationNumber).
        let connector_refund_id = derive_api_transaction_id(&item.router_data.request.refund_id)
            .change_context(
                errors::ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("failed to recompute the APITransactionID sent on Refund".to_string()),
                ),
            )?;

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
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
// SETUPMANDATE REQUEST
// =============================================================================
// A zero-amount credit authorization (TransactionAmount 0.00, POSConditionCode 51) sent with
// CardholderInitiatedTransaction Y and WPTokenRequested Y: the cardholder-initiated setup of a
// stored credential. It returns the TokenizedPAN (the mandate id) and the network transaction id
// that later MITs reference (tech spec: SetupMandate; 6. Zero-amount account verification).

/// `/credit/authorization` body of a mandate setup: the same `creditauth` inner as Authorize.
#[derive(Debug, Serialize)]
pub struct WorldpayraftSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub creditauth: WorldpayraftPaymentInner<T>,
}

// =============================================================================
// SETUPMANDATE RESPONSE
// =============================================================================

/// `/credit/authorization` response of a mandate setup.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldpayraftSetupMandateResponse {
    pub creditauthresponse: WorldpayraftResponseInner,
}

/// Returns the card, or the documented refusal for every other payment method arm.
fn setup_mandate_card<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<&Card<T>, error_stack::Report<errors::IntegrationError>> {
    match payment_method_data {
        PaymentMethodData::Card(card) => Ok(card),
        PaymentMethodData::CardWithNoCvc(_) => Err(not_card_error("CardWithNoCvc")),
        PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
            Err(not_card_error("CardDetailsForNetworkTransactionId"))
        }
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => Err(
            not_card_error("DecryptedWalletTokenDetailsForNetworkTransactionId"),
        ),
        PaymentMethodData::CardRedirect(_) => Err(not_card_error("CardRedirect")),
        PaymentMethodData::Wallet(_) => Err(not_card_error("Wallet")),
        PaymentMethodData::PayLater(_) => Err(not_card_error("PayLater")),
        PaymentMethodData::BankRedirect(_) => Err(not_card_error("BankRedirect")),
        PaymentMethodData::BankDebit(_) => Err(not_card_error("BankDebit")),
        PaymentMethodData::BankTransfer(_) => Err(not_card_error("BankTransfer")),
        PaymentMethodData::Crypto(_) => Err(not_card_error("Crypto")),
        PaymentMethodData::MandatePayment => Err(not_card_error("MandatePayment")),
        PaymentMethodData::Reward => Err(not_card_error("Reward")),
        PaymentMethodData::RealTimePayment(_) => Err(not_card_error("RealTimePayment")),
        PaymentMethodData::Upi(_) => Err(not_card_error("Upi")),
        PaymentMethodData::Voucher(_) => Err(not_card_error("Voucher")),
        PaymentMethodData::GiftCard(_) => Err(not_card_error("GiftCard")),
        PaymentMethodData::PaymentMethodToken(_) => Err(not_card_error("PaymentMethodToken")),
        PaymentMethodData::OpenBanking(_) => Err(not_card_error("OpenBanking")),
        PaymentMethodData::NetworkToken(_) => Err(not_card_error("NetworkToken")),
        PaymentMethodData::MobilePayment(_) => Err(not_card_error("MobilePayment")),
    }
}

/// `TerminalData.POSEnvironment` of a stored-credential setup, from the MIT category the
/// credential will serve: Recurring → R, Installment → I, anything else → C (credential on file).
fn setup_mandate_pos_environment(
    mit_category: Option<&common_enums::MitCategory>,
) -> WorldpayraftPosEnvironment {
    match mit_category {
        Some(common_enums::MitCategory::Recurring) => WorldpayraftPosEnvironment::Recurring,
        Some(common_enums::MitCategory::Installment) => WorldpayraftPosEnvironment::Installment,
        Some(common_enums::MitCategory::Unscheduled)
        | Some(common_enums::MitCategory::Resubmission)
        | None => WorldpayraftPosEnvironment::CredentialOnFile,
    }
}

/// Card brand derived from the PAN when the request carries no `card_network`.
fn card_brand_from_issuer(issuer: domain_types::utils::CardIssuer) -> WorldpayraftCardBrand {
    match issuer {
        domain_types::utils::CardIssuer::Visa => WorldpayraftCardBrand::Visa,
        domain_types::utils::CardIssuer::Master => WorldpayraftCardBrand::Mastercard,
        domain_types::utils::CardIssuer::AmericanExpress => WorldpayraftCardBrand::Amex,
        domain_types::utils::CardIssuer::Discover => WorldpayraftCardBrand::Discover,
        domain_types::utils::CardIssuer::Maestro
        | domain_types::utils::CardIssuer::DinersClub
        | domain_types::utils::CardIssuer::JCB
        | domain_types::utils::CardIssuer::CarteBlanche
        | domain_types::utils::CardIssuer::CartesBancaires
        | domain_types::utils::CardIssuer::UnionPay => WorldpayraftCardBrand::Other,
    }
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftSetupMandateRequest
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayraftRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayraftSetupMandateRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: WorldpayraftRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let card = setup_mandate_card(&request.payment_method_data)?;

        // G-SetupMandate-01: the debit API cannot set up a stored credential.
        if is_debit_card(card) {
            return Err(error_stack::report!(errors::IntegrationError::NotSupported {
                message: "mandate setup on a debit card".to_string(),
                connector: CONNECTOR_NAME,
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "debit.yaml has no POSEnvironment, no WPTokenRequested and no NTID fields; RepeatPayment is credit-only".to_string(),
                    ),
                    suggested_action: Some(
                        "set up the mandate with a credit card".to_string(),
                    ),
                    ..Default::default()
                },
            }));
        }

        // G-ThreeDS-01..03: RAFT cannot authenticate; a 3DS mandate setup carries the external
        // result or is refused, never downgraded to ECI 07/10.
        let external_three_ds = resolve_external_three_ds(
            router_data.resource_common_data.auth_type,
            request.authentication_data.as_ref(),
        )?;

        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        // A verification-only request (POSConditionCode 51) must carry a zero amount, whatever
        // amount the setup request names.
        let transaction_amount = item
            .connector
            .amount_converter
            .convert(MinorUnit::zero(), request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT mandate setup sends TransactionAmount 0.00 in major currency units".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let expiration_date = card.get_expiry_date_as_yymm().change_context(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "card.card_exp_year / card.card_exp_month",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT expects card expiry in YYMM format".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        let card_verification_data =
            (!card.card_cvc.peek().is_empty()).then(|| WorldpayraftCardVerificationData {
                cvv_indicator: WorldpayraftCvvIndicator::Present,
                cvv_value: Some(card.card_cvc.clone()),
            });

        let addresses = build_addresses(
            router_data.resource_common_data.get_optional_billing(),
            None,
            WorldpayraftCardKind::Credit,
        )?;

        let terminal_data = WorldpayraftTerminalData {
            entry_mode: WorldpayraftEntryMode::ECommerce,
            terminal_type: Some(WorldpayraftTerminalType::Internet),
            pos_condition_code: WorldpayraftPosConditionCode::VerificationOnly,
            terminal_entry_cap: WorldpayraftTerminalEntryCap::Unspecified,
            pos_environment: Some(setup_mandate_pos_environment(request.mit_category.as_ref())),
        };

        let proc_flags_indicators = WorldpayraftProcFlags {
            cardholder_initiated_transaction: Some(WorldpayraftYesNo::Yes),
            mastercard_advice_code_indicator: Some(WorldpayraftYesNo::Yes),
            event_notification_indicator: Some(EVENT_NOTIFICATION_OPT_IN),
            ..Default::default()
        };

        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;
        let api_transaction_id = derive_api_transaction_id(reference).change_context(
            errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "failed to derive the 16-digit APITransactionID from connector_request_reference_id".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        let reference_trace_numbers = WorldpayraftRequestTraceNumbers {
            // CorrelationID is optional, echoed, and used by Worldpay only for reporting and
            // research (tech spec: Merchant reference id / order id): the reference is sent when
            // it fits 25 characters and omitted otherwise. Nothing correlates on it (plan o-audit).
            correlation_id: fit_identifier(reference, MAX_CORRELATION_ID),
            ..Default::default()
        };
        // UserData1 surfaces as customerFields.field1 in Event Notifications, which carry no
        // APITransactionID: sending the APITransactionID of this message (16 digits, always fits
        // 35) makes the webhook join key equal the reported connector_transaction_id and never
        // absent, whatever the length of the UCS reference.
        let user_data_1 = fit_identifier(&api_transaction_id, MAX_USER_DATA_1);

        Ok(Self {
            creditauth: WorldpayraftPaymentInner {
                misc_amounts_balances: WorldpayraftAmounts::transaction_only(transaction_amount),
                card_info: WorldpayraftCardInfo {
                    pan: Some(card.card_number.clone()),
                    expiration_date: Some(expiration_date),
                },
                card_verification_data,
                address_verification_data: addresses.address_verification_data,
                online_bill_to_address: addresses.online_bill_to_address,
                online_ship_to_address: None,
                customer_information: None,
                terminal_data,
                ecommerce_data: build_ecommerce_data(
                    true,
                    request.mit_category.as_ref(),
                    None,
                    None,
                    WorldpayraftCardKind::Credit,
                    external_three_ds,
                )?,
                proc_flags_indicators,
                encryption_token_data: Some(WorldpayraftEncryptionTokenRequest {
                    tokenized_pan: None,
                    wp_token_requested: Some(WorldpayraftYesNo::Yes),
                }),
                reference_trace_numbers: (!reference_trace_numbers.is_empty())
                    .then_some(reference_trace_numbers),
                user_defined_data: user_data_1.map(|user_data_1| WorldpayraftUserDefinedData {
                    user_data_1: Some(user_data_1),
                }),
                soft_descriptor_data: None,
                merchant_specific_data: None,
                level3_data: None,
                world_pay_merchant_id: auth.merchant_id,
                api_transaction_id,
                local_date_time: local_date_time(),
            },
        })
    }
}

// =============================================================================
// TryFrom: WorldpayraftSetupMandateResponse → RouterDataV2
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<WorldpayraftSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let inner = &item.response.creditauthresponse;
        let request = &item.router_data.request;

        let failure = |error: ErrorResponse, router_data: Self| Self {
            response: Err(error),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Failure,
                ..router_data.resource_common_data
            },
            ..router_data
        };

        // Only an approval that returned the token sets up a mandate; a verification is never a
        // partial or reviewed approval, so those classes fail like a decline.
        let (status, tokenized_pan) = match (classify(inner), inner.tokenized_pan()) {
            (WorldpayraftDecision::Approved, Some(token)) => (AttemptStatus::Charged, Some(token)),
            (WorldpayraftDecision::Approved, None) => {
                let error = ErrorResponse {
                    status_code: item.http_code,
                    code: consts::NO_ERROR_CODE.to_string(),
                    message: "no TokenizedPAN returned".to_string(),
                    reason: Some(
                        "Worldpay RAFT approved the verification without EncryptionTokenData.TokenizedPAN, so no mandate can be stored".to_string(),
                    ),
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: inner.api_transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_response: None,
                    typed_connector_request: None,
                };
                return Ok(failure(error, item.router_data));
            }
            (WorldpayraftDecision::RequestInProgress, token) => (AttemptStatus::Pending, token),
            (
                WorldpayraftDecision::PartialApproval
                | WorldpayraftDecision::HonorWithId
                | WorldpayraftDecision::Declined
                | WorldpayraftDecision::MessageRejected,
                _,
            ) => {
                let error = build_business_error(
                    inner,
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
                );
                return Ok(failure(error, item.router_data));
            }
        };

        // connector_transaction_id is the APITransactionID that was SENT.
        let api_transaction_id = derive_api_transaction_id(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        )
        .change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("failed to recompute the APITransactionID sent on SetupMandate".to_string()),
            ),
        )?;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => Some(card),
            _ => None,
        };
        let expiration_date = card.and_then(|card| card.get_expiry_date_as_yymm().ok());
        let card_network = card.and_then(card_network_of);
        let network_transaction_ids = inner.network_ids();

        let feature_data = WorldpayraftFeatureData {
            api_transaction_id: api_transaction_id.clone(),
            operation: WorldpayraftOperation::Verification,
            card_kind: WorldpayraftCardKind::Credit,
            authorized_amount: MinorUnit::zero(),
            captured_amount: None,
            currency: request.currency,
            tokenized_pan: tokenized_pan.clone(),
            expiration_date: expiration_date.clone(),
            authorization_number: inner.authorization_number(),
            retrieval_ref_number: inner
                .reference_trace_numbers
                .as_ref()
                .and_then(|trace| trace.retrieval_ref_number.clone()),
            card_network,
            network_transaction_ids: network_transaction_ids.clone(),
        };
        let connector_metadata = serde_json::to_value(&feature_data).change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("failed to serialize Worldpay RAFT connector_feature_data".to_string()),
            ),
        )?;

        // mandate_metadata carries what a later MIT cannot read from RepeatPaymentData: the
        // expiry (CardInfo.ExpirationDate), the brand (which brand object to send) and the NTIDs.
        let mandate_reference = tokenized_pan.map(|token| {
            Box::new(MandateReference {
                connector_mandate_id: Some(token.expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: Some(build_mandate_metadata(
                    expiration_date.as_ref(),
                    card_network,
                    network_transaction_ids.as_ref(),
                )),
            })
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(api_transaction_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: Some(connector_metadata),
                network_txn_id: inner.network_transaction_id(),
                network_txn_link_id: inner
                    .reference_trace_numbers
                    .as_ref()
                    .and_then(|trace| trace.transaction_link_id.clone()),
                connector_response_reference_id: Some(api_transaction_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: inner
                    .reference_trace_numbers
                    .as_ref()
                    .and_then(|trace| trace.payment_acct_ref_number.clone()),
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response: inner.card_connector_response(),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REPEAT PAYMENT (MIT) REQUEST
// =============================================================================
// No dedicated MIT endpoint: a merchant-initiated charge is `/credit/purchase` (auto capture) or
// `/credit/authorization` (Manual) carrying the stored-credential fields —
// MerchantInitiatedTransaction Y, POSEnvironment, the card identity (the stored TokenizedPAN, or the
// clear card of a network-transaction-id MIT), CardInfo.ExpirationDate and the per-brand NTID
// (tech spec: RepeatPayment; D14). Debit has none of these fields, so a MIT is always credit.

/// Original message of a merchant-initiated charge: auto capture → purchase, Manual →
/// authorization. ManualMultiple and Scheduled are refused by the request builder
/// (G-RepeatPayment-05). Shared by `get_url`, the request builder and the response handler so the
/// endpoint, the wrapper key and the status mapping cannot disagree.
pub fn repeat_payment_operation<T: PaymentMethodDataTypes>(
    request: &RepeatPaymentData<T>,
) -> WorldpayraftOperation {
    if request.is_auto_capture() {
        WorldpayraftOperation::Purchase
    } else {
        WorldpayraftOperation::Authorization
    }
}

/// `mandate_metadata` written by SetupMandate / a CIT Authorize: what a later MIT cannot read from
/// `RepeatPaymentData` (the card expiry, the brand and the NTIDs of the credential-setting message).
#[derive(Debug, Deserialize)]
struct WorldpayraftMandateMetadata {
    expiration_date: Option<Secret<String>>,
    card_network: Option<WorldpayraftCardBrand>,
    network_transaction_ids: Option<WorldpayraftNetworkIds>,
}

/// Brand of a clear card: its `card_network`, else the brand derived from the PAN.
fn card_network_of<T: PaymentMethodDataTypes>(card: &Card<T>) -> Option<WorldpayraftCardBrand> {
    card_brand(card).or_else(|| {
        domain_types::utils::get_card_issuer(card.card_number.peek())
            .ok()
            .map(card_brand_from_issuer)
    })
}

/// The one writer of `mandate_metadata` (`WorldpayraftMandateMetadata`), shared by a CIT
/// Authorize, SetupMandate and an off-session RepeatPayment refresh, so the three cannot drift.
fn build_mandate_metadata(
    expiration_date: Option<&Secret<String>>,
    card_network: Option<WorldpayraftCardBrand>,
    network_transaction_ids: Option<&WorldpayraftNetworkIds>,
) -> Secret<serde_json::Value> {
    Secret::new(serde_json::json!({
        "expiration_date": expiration_date.map(|date| date.peek().clone()),
        "card_network": card_network,
        "network_transaction_ids": network_transaction_ids,
    }))
}

/// Card identity of a merchant-initiated charge.
struct WorldpayraftMitCredential {
    /// `EncryptionTokenData.TokenizedPAN` (connector mandate MIT).
    tokenized_pan: Option<Secret<String>>,
    /// `CardInfo.PAN` (network-transaction-id MIT).
    pan: Option<cards::CardNumber>,
    /// `CardInfo.ExpirationDate`, YYMM, sent on every MIT.
    expiration_date: Secret<String>,
    card_network: Option<WorldpayraftCardBrand>,
    network_ids: Option<WorldpayraftNetworkIds>,
}

fn network_token_mit_error() -> error_stack::Report<errors::IntegrationError> {
    error_stack::report!(errors::IntegrationError::NotImplemented(
        "NetworkTokenWithNTI MIT".to_string(),
        errors::IntegrationErrorContext {
            additional_context: Some(
                "network-token MIT needs PaymentTokenAuthenticationCryptogram handling not in this change".to_string(),
            ),
            suggested_action: Some(
                "charge with a connector mandate id, or a network transaction id with card details"
                    .to_string(),
            ),
            ..Default::default()
        },
    ))
}

/// Brand whose NTIDs were stored when `mandate_metadata` carries no `card_network`.
fn brand_from_network_ids(ids: &WorldpayraftNetworkIds) -> Option<WorldpayraftCardBrand> {
    if ids.visa_transaction_id.is_some() {
        Some(WorldpayraftCardBrand::Visa)
    } else if ids.mcrd_banknet_ref_num.is_some() {
        Some(WorldpayraftCardBrand::Mastercard)
    } else if ids.amex_transaction_id.is_some() {
        Some(WorldpayraftCardBrand::Amex)
    } else if ids.disc_transaction_id.is_some() {
        Some(WorldpayraftCardBrand::Discover)
    } else {
        None
    }
}

/// Connector mandate MIT: the stored TokenizedPAN plus the expiry, brand and NTIDs kept in
/// `mandate_metadata` (D14c, D14e).
fn connector_mandate_credential(
    mandate: &ConnectorMandateReferenceId,
) -> Result<WorldpayraftMitCredential, error_stack::Report<errors::IntegrationError>> {
    // G-RepeatPayment-01: the TokenizedPAN is the RAFT stored credential.
    let tokenized_pan = mandate
        .get_connector_mandate_id()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingConnectorMandateID {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "the TokenizedPAN returned by SetupMandate/CIT Authorize is the RAFT stored credential".to_string(),
                    ),
                    suggested_action: Some(
                        "send the connector_mandate_id returned by the Worldpay RAFT mandate setup".to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?;

    // A malformed metadata value is treated like an absent one: G-RepeatPayment-02 then names it.
    let metadata = mandate.get_mandate_metadata().and_then(|metadata| {
        serde_json::from_value::<WorldpayraftMandateMetadata>(metadata.expose()).ok()
    });

    // G-RepeatPayment-02: CardInfo.ExpirationDate is sent on every token-initiated MIT.
    let expiration_date = metadata
        .as_ref()
        .and_then(|metadata| metadata.expiration_date.clone())
        .filter(|date| !date.peek().is_empty())
        .ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingConnectorMandateMetadata {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "CardInfo.ExpirationDate is required when no track data is sent and Worldpay requires it on every token-initiated transaction (Discover)".to_string(),
                    ),
                    suggested_action: Some(
                        "send the mandate_metadata returned by the Worldpay RAFT mandate setup unchanged".to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?;

    let network_ids = metadata
        .as_ref()
        .and_then(|metadata| metadata.network_transaction_ids.clone());
    let card_network = metadata
        .and_then(|metadata| metadata.card_network)
        .or_else(|| network_ids.as_ref().and_then(brand_from_network_ids));

    Ok(WorldpayraftMitCredential {
        tokenized_pan: Some(Secret::new(tokenized_pan)),
        pan: None,
        expiration_date,
        card_network,
        network_ids,
    })
}

/// Network-transaction-id MIT: the clear card plus the NTID routed into its brand's object.
fn network_mandate_credential(
    card: &CardDetailsForNetworkTransactionId,
    network_mandate: &NetworkMandateIdRef,
) -> Result<WorldpayraftMitCredential, error_stack::Report<errors::IntegrationError>> {
    // G-RepeatPayment-04: RAFT has no generic NTID field, so the brand must be known.
    let brand_error = || {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "payment_method_data.card_network",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "RAFT carries the NTID in a per-brand object; there is no generic NetworkTransactionId field".to_string(),
                ),
                suggested_action: Some(
                    "send the card_network (Visa, Mastercard, American Express or Discover) of the card".to_string(),
                ),
                ..Default::default()
            },
        })
    };
    let brand = match card.card_network.as_ref() {
        Some(network) => WorldpayraftCardBrand::from(network),
        None => card
            .get_card_issuer()
            .map(card_brand_from_issuer)
            .map_err(|_| brand_error())?,
    };

    let ntid = network_mandate.network_transaction_id.clone();
    let network_ids = match brand {
        WorldpayraftCardBrand::Visa => WorldpayraftNetworkIds {
            visa_transaction_id: Some(ntid),
            ..Default::default()
        },
        WorldpayraftCardBrand::Mastercard => {
            // G-RepeatPayment-03: the Mastercard NTID is McrdBanknetREFNUM (9) followed by
            // McrdBanknetSettleDate (MMDD, 4), as Authorize/SetupMandate store it (UD-06); both
            // halves must be echoed.
            if ntid.chars().count() != 13 {
                return Err(error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                    field_name: "mandate_reference.network_transaction_id",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Mastercard MIT needs both McrdBanknetREFNUM (9) and McrdBanknetSettleDate (MMDD)".to_string(),
                        ),
                        suggested_action: Some(
                            "send the 13-character network_transaction_id (McrdBanknetREFNUM followed by McrdBanknetSettleDate) returned on the CIT".to_string(),
                        ),
                        ..Default::default()
                    },
                }));
            }
            WorldpayraftNetworkIds {
                mcrd_banknet_ref_num: Some(ntid.chars().take(9).collect()),
                mcrd_banknet_settle_date: Some(ntid.chars().skip(9).collect()),
                transaction_link_id: network_mandate.transaction_link_id.clone(),
                ..Default::default()
            }
        }
        WorldpayraftCardBrand::Amex => WorldpayraftNetworkIds {
            amex_transaction_id: Some(ntid),
            ..Default::default()
        },
        WorldpayraftCardBrand::Discover => WorldpayraftNetworkIds {
            disc_transaction_id: Some(ntid),
            ..Default::default()
        },
        WorldpayraftCardBrand::Other => return Err(brand_error()),
    };

    let expiration_date = card
        .get_expiry_date_as_yymm()
        .map_err(error_stack::Report::new)
        .change_context(errors::IntegrationError::InvalidDataFormat {
            field_name: "payment_method_data.card_exp_year / card_exp_month",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "Worldpay RAFT expects card expiry in YYMM format".to_string(),
                ),
                ..Default::default()
            },
        })?;

    Ok(WorldpayraftMitCredential {
        tokenized_pan: None,
        pan: Some(card.card_number.clone()),
        expiration_date,
        card_network: Some(brand),
        network_ids: Some(network_ids),
    })
}

/// Resolves the card identity of a MIT from its payment method and mandate reference, or the
/// documented refusal. Card and MandatePayment charge a connector mandate (the stored
/// TokenizedPAN); CardDetailsForNetworkTransactionId charges a network transaction id.
fn mit_credential<T: PaymentMethodDataTypes>(
    request: &RepeatPaymentData<T>,
) -> Result<WorldpayraftMitCredential, error_stack::Report<errors::IntegrationError>> {
    match &request.payment_method_data {
        PaymentMethodData::Card(_) | PaymentMethodData::MandatePayment => {
            match &request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(mandate) => {
                    connector_mandate_credential(mandate)
                }
                MandateReferenceId::NetworkMandateId(_) => {
                    Err(error_stack::report!(errors::IntegrationError::NotSupported {
                        message: "network transaction id MIT without CardDetailsForNetworkTransactionId".to_string(),
                        connector: CONNECTOR_NAME,
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "a network-transaction-id MIT sends the clear card in CardInfo, which only CardDetailsForNetworkTransactionId carries".to_string(),
                            ),
                            suggested_action: Some(
                                "send payment_method_data CardDetailsForNetworkTransactionId with the network_mandate_id".to_string(),
                            ),
                            ..Default::default()
                        },
                    }))
                }
                MandateReferenceId::NetworkTokenWithNTI(_) => Err(network_token_mit_error()),
            }
        }
        PaymentMethodData::CardDetailsForNetworkTransactionId(card) => {
            match &request.mandate_reference {
                MandateReferenceId::NetworkMandateId(network_mandate) => {
                    network_mandate_credential(card, network_mandate)
                }
                MandateReferenceId::ConnectorMandateId(_) => {
                    Err(error_stack::report!(errors::IntegrationError::NotSupported {
                        message: "CardDetailsForNetworkTransactionId with a connector mandate id".to_string(),
                        connector: CONNECTOR_NAME,
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "a connector-mandate MIT identifies the card by its TokenizedPAN; card details go with a network transaction id".to_string(),
                            ),
                            suggested_action: Some(
                                "send the network_mandate_id with these card details, or MandatePayment with the connector mandate id".to_string(),
                            ),
                            ..Default::default()
                        },
                    }))
                }
                MandateReferenceId::NetworkTokenWithNTI(_) => Err(network_token_mit_error()),
            }
        }
        PaymentMethodData::NetworkToken(_) => Err(network_token_mit_error()),
        PaymentMethodData::CardWithNoCvc(_) => Err(not_card_error("CardWithNoCvc")),
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => Err(
            not_card_error("DecryptedWalletTokenDetailsForNetworkTransactionId"),
        ),
        PaymentMethodData::CardRedirect(_) => Err(not_card_error("CardRedirect")),
        PaymentMethodData::Wallet(_) => Err(not_card_error("Wallet")),
        PaymentMethodData::PayLater(_) => Err(not_card_error("PayLater")),
        PaymentMethodData::BankRedirect(_) => Err(not_card_error("BankRedirect")),
        PaymentMethodData::BankDebit(_) => Err(not_card_error("BankDebit")),
        PaymentMethodData::BankTransfer(_) => Err(not_card_error("BankTransfer")),
        PaymentMethodData::Crypto(_) => Err(not_card_error("Crypto")),
        PaymentMethodData::Reward => Err(not_card_error("Reward")),
        PaymentMethodData::RealTimePayment(_) => Err(not_card_error("RealTimePayment")),
        PaymentMethodData::Upi(_) => Err(not_card_error("Upi")),
        PaymentMethodData::Voucher(_) => Err(not_card_error("Voucher")),
        PaymentMethodData::GiftCard(_) => Err(not_card_error("GiftCard")),
        PaymentMethodData::PaymentMethodToken(_) => Err(not_card_error("PaymentMethodToken")),
        PaymentMethodData::OpenBanking(_) => Err(not_card_error("OpenBanking")),
        PaymentMethodData::MobilePayment(_) => Err(not_card_error("MobilePayment")),
    }
}

/// `E-commerceData.E-commerceIndicator` of a MIT by MIT category (UD-05): Recurring → 02,
/// Installment → 03, anything else → 07.
fn mit_ecommerce_indicator(
    mit_category: Option<&common_enums::MitCategory>,
) -> WorldpayraftEcommerceIndicator {
    match mit_category {
        Some(common_enums::MitCategory::Recurring) => WorldpayraftEcommerceIndicator::Recurring,
        Some(common_enums::MitCategory::Installment) => WorldpayraftEcommerceIndicator::Installment,
        Some(common_enums::MitCategory::Unscheduled)
        | Some(common_enums::MitCategory::Resubmission)
        | None => WorldpayraftEcommerceIndicator::NonAuthenticated,
    }
}

/// `CardInfo` of a MIT: the clear PAN only on a network-transaction-id MIT; the expiry always.
#[derive(Debug, Serialize)]
pub struct WorldpayraftMitCardInfo {
    #[serde(rename = "PAN", skip_serializing_if = "Option::is_none")]
    pub pan: Option<cards::CardNumber>,
    #[serde(rename = "ExpirationDate")]
    pub expiration_date: Secret<String>,
}

/// Body of a merchant-initiated `creditpurchase` / `creditauth`.
#[derive(Debug, Serialize)]
pub struct WorldpayraftMitInner {
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(rename = "CardInfo")]
    pub card_info: WorldpayraftMitCardInfo,
    #[serde(rename = "TerminalData")]
    pub terminal_data: WorldpayraftTerminalData,
    #[serde(rename = "E-commerceData")]
    pub ecommerce_data: WorldpayraftEcommerceData,
    #[serde(rename = "ProcFlagsIndicators")]
    pub proc_flags_indicators: WorldpayraftProcFlags,
    #[serde(
        rename = "EncryptionTokenData",
        skip_serializing_if = "Option::is_none"
    )]
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenRequest>,
    #[serde(rename = "VisaSpecificData", skip_serializing_if = "Option::is_none")]
    pub visa_specific_data: Option<WorldpayraftVisaSpecificData>,
    #[serde(rename = "McrdSpecificData", skip_serializing_if = "Option::is_none")]
    pub mcrd_specific_data: Option<WorldpayraftMcrdSpecificData>,
    #[serde(rename = "AmexSpecificData", skip_serializing_if = "Option::is_none")]
    pub amex_specific_data: Option<WorldpayraftAmexSpecificData>,
    #[serde(rename = "DiscSpecificData", skip_serializing_if = "Option::is_none")]
    pub disc_specific_data: Option<WorldpayraftDiscSpecificData>,
    #[serde(
        rename = "ReferenceTraceNumbers",
        skip_serializing_if = "Option::is_none"
    )]
    pub reference_trace_numbers: Option<WorldpayraftRequestTraceNumbers>,
    #[serde(rename = "UserDefinedData", skip_serializing_if = "Option::is_none")]
    pub user_defined_data: Option<WorldpayraftUserDefinedData>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
    pub local_date_time: String,
}

/// Merchant-initiated charge, keyed by its operation wrapper.
///
/// `/credit/purchase` → `creditpurchase`, `/credit/authorization` → `creditauth`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftRepeatPaymentRequest {
    CreditPurchase {
        creditpurchase: WorldpayraftMitInner,
    },
    CreditAuth {
        creditauth: WorldpayraftMitInner,
    },
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftRepeatPaymentRequest
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayraftRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayraftRepeatPaymentRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: WorldpayraftRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // G-RepeatPayment-05 (every arm): one completion per authorization.
        if matches!(
            request.capture_method,
            Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        ) {
            return Err(error_stack::report!(
                errors::IntegrationError::CaptureMethodNotSupported {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "MIT supports Automatic (purchase) or Manual (authorization)"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "use capture_method AUTOMATIC or MANUAL".to_string(),
                        ),
                        ..Default::default()
                    },
                }
            ));
        }

        let credential = mit_credential(request)?;
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires the MIT amount in major currency units (ddddddddd.cc)".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let mit_category = request.mit_category.as_ref();
        let terminal_data = WorldpayraftTerminalData {
            entry_mode: WorldpayraftEntryMode::ECommerce,
            terminal_type: None,
            pos_condition_code: WorldpayraftPosConditionCode::ElectronicCommerce,
            terminal_entry_cap: WorldpayraftTerminalEntryCap::Unspecified,
            // Same category → POSEnvironment mapping as the credential-setting message (UD-05).
            pos_environment: Some(setup_mandate_pos_environment(mit_category)),
        };
        let ecommerce_data = WorldpayraftEcommerceData {
            ecommerce_indicator: mit_ecommerce_indicator(mit_category),
            three_d_secure_data: None,
            three_d_secure_program_protocol: None,
            three_d_secure_ds_transaction_id: None,
            ecommerce_ip_address: None,
            ecommerce_order_num: None,
        };
        let is_recurring = mit_category == Some(&common_enums::MitCategory::Recurring);
        let proc_flags_indicators = WorldpayraftProcFlags {
            merchant_initiated_transaction: Some(WorldpayraftYesNo::Yes),
            // D14d: a recurring bill payment only when the MIT category says so.
            recurring_bill_pay: is_recurring.then_some(WorldpayraftYesNo::Yes),
            mastercard_advice_code_indicator: Some(WorldpayraftYesNo::Yes),
            event_notification_indicator: Some(EVENT_NOTIFICATION_OPT_IN),
            ..Default::default()
        };

        // UD-19: the only reason code a RepeatPaymentData input selects.
        let reason_code = (mit_category == Some(&common_enums::MitCategory::Resubmission))
            .then_some(WorldpayraftSubsequentTransactionReasonCode::Resubmission);
        let brand = credential.card_network;
        let ids = credential.network_ids.as_ref();
        let visa_specific_data = (brand == Some(WorldpayraftCardBrand::Visa))
            .then(|| WorldpayraftVisaSpecificData {
                visa_transaction_id: ids.and_then(|ids| ids.visa_transaction_id.clone()),
                visa_subsequent_transaction_reason_code: reason_code,
            })
            .filter(|data| {
                data.visa_transaction_id.is_some()
                    || data.visa_subsequent_transaction_reason_code.is_some()
            });
        // Mastercard needs both Banknet fields; one without the other is not echoed.
        let (mcrd_banknet_ref_num, mcrd_banknet_settle_date) = ids
            .and_then(|ids| {
                ids.mcrd_banknet_ref_num
                    .clone()
                    .zip(ids.mcrd_banknet_settle_date.clone())
            })
            .map_or((None, None), |(ref_num, settle_date)| {
                (Some(ref_num), Some(settle_date))
            });
        let mcrd_specific_data = (brand == Some(WorldpayraftCardBrand::Mastercard))
            .then_some(WorldpayraftMcrdSpecificData {
                mcrd_banknet_ref_num,
                mcrd_banknet_settle_date,
                mcrd_subsequent_transaction_reason_code: reason_code,
            })
            .filter(|data| {
                data.mcrd_banknet_ref_num.is_some()
                    || data.mcrd_subsequent_transaction_reason_code.is_some()
            });
        let amex_specific_data = (brand == Some(WorldpayraftCardBrand::Amex))
            .then(|| WorldpayraftAmexSpecificData {
                amex_transaction_id: ids.and_then(|ids| ids.amex_transaction_id.clone()),
                amex_subsequent_transaction_reason_code: reason_code,
            })
            .filter(|data| {
                data.amex_transaction_id.is_some()
                    || data.amex_subsequent_transaction_reason_code.is_some()
            });
        let disc_specific_data = (brand == Some(WorldpayraftCardBrand::Discover))
            .then(|| WorldpayraftDiscSpecificData {
                disc_transaction_id: ids.and_then(|ids| ids.disc_transaction_id.clone()),
                disc_subsequent_transaction_reason_code: reason_code,
            })
            .filter(|data| {
                data.disc_transaction_id.is_some()
                    || data.disc_subsequent_transaction_reason_code.is_some()
            });

        // A new APITransactionID per MIT, derived like Authorize's.
        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;
        let api_transaction_id = derive_api_transaction_id(reference).change_context(
            errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "failed to derive the 16-digit APITransactionID from connector_request_reference_id".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        let reference_trace_numbers = WorldpayraftRequestTraceNumbers {
            // CorrelationID is optional, echoed, and used by Worldpay only for reporting and
            // research (tech spec: Merchant reference id / order id): the reference is sent when
            // it fits 25 characters and omitted otherwise. Nothing correlates on it (plan o-audit).
            correlation_id: fit_identifier(reference, MAX_CORRELATION_ID),
            // Mastercard/Maestro lifecycle linking: the CIT's TransactionLinkID.
            economically_related_link_id: (brand == Some(WorldpayraftCardBrand::Mastercard))
                .then(|| ids.and_then(|ids| ids.transaction_link_id.clone()))
                .flatten(),
            ..Default::default()
        };
        // UserData1 surfaces as customerFields.field1 in Event Notifications, which carry no
        // APITransactionID: sending the APITransactionID of this message (16 digits, always fits
        // 35) makes the webhook join key equal the reported connector_transaction_id and never
        // absent, whatever the length of the UCS reference.
        let user_data_1 = fit_identifier(&api_transaction_id, MAX_USER_DATA_1);

        let inner = WorldpayraftMitInner {
            misc_amounts_balances: WorldpayraftAmounts::transaction_only(transaction_amount),
            card_info: WorldpayraftMitCardInfo {
                pan: credential.pan,
                expiration_date: credential.expiration_date,
            },
            terminal_data,
            ecommerce_data,
            proc_flags_indicators,
            // D14e: a stored Worldpay token goes in EncryptionTokenData, never in CardInfo.PAN.
            encryption_token_data: credential.tokenized_pan.map(|tokenized_pan| {
                WorldpayraftEncryptionTokenRequest {
                    tokenized_pan: Some(tokenized_pan),
                    wp_token_requested: None,
                }
            }),
            visa_specific_data,
            mcrd_specific_data,
            amex_specific_data,
            disc_specific_data,
            reference_trace_numbers: (!reference_trace_numbers.is_empty())
                .then_some(reference_trace_numbers),
            user_defined_data: user_data_1.map(|user_data_1| WorldpayraftUserDefinedData {
                user_data_1: Some(user_data_1),
            }),
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id,
            local_date_time: local_date_time(),
        };

        Ok(match repeat_payment_operation(request) {
            WorldpayraftOperation::Purchase => Self::CreditPurchase {
                creditpurchase: inner,
            },
            WorldpayraftOperation::Authorization | WorldpayraftOperation::Verification => {
                Self::CreditAuth { creditauth: inner }
            }
        })
    }
}

// =============================================================================
// REPEAT PAYMENT (MIT) RESPONSE
// =============================================================================

/// Response of a merchant-initiated charge, keyed by its operation wrapper.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftRepeatPaymentResponse {
    CreditPurchase {
        creditpurchaseresponse: WorldpayraftResponseInner,
    },
    CreditAuth {
        creditauthresponse: WorldpayraftResponseInner,
    },
}

impl WorldpayraftRepeatPaymentResponse {
    pub fn inner(&self) -> &WorldpayraftResponseInner {
        match self {
            Self::CreditPurchase {
                creditpurchaseresponse,
            } => creditpurchaseresponse,
            Self::CreditAuth { creditauthresponse } => creditauthresponse,
        }
    }
}

// =============================================================================
// TryFrom: WorldpayraftRepeatPaymentResponse → RouterDataV2
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<WorldpayraftRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let inner = item.response.inner();
        let request = &item.router_data.request;
        let operation = repeat_payment_operation(request);
        let decision = classify(inner);

        let status = match decision {
            WorldpayraftDecision::Approved => match operation {
                WorldpayraftOperation::Purchase | WorldpayraftOperation::Verification => {
                    AttemptStatus::Charged
                }
                WorldpayraftOperation::Authorization => AttemptStatus::Authorized,
            },
            WorldpayraftDecision::PartialApproval => match operation {
                WorldpayraftOperation::Purchase | WorldpayraftOperation::Verification => {
                    AttemptStatus::PartialCharged
                }
                WorldpayraftOperation::Authorization => AttemptStatus::PartiallyAuthorized,
            },
            WorldpayraftDecision::RequestInProgress => AttemptStatus::Pending,
            // HONOR WITH ID needs the cardholder to show ID; no cardholder is present on a MIT.
            WorldpayraftDecision::HonorWithId
            | WorldpayraftDecision::Declined
            | WorldpayraftDecision::MessageRejected => {
                return Ok(Self {
                    response: Err(build_business_error(
                        inner,
                        item.http_code,
                        Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    )),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                });
            }
        };

        let partial_amount = match decision {
            // ResponseCode 010: the approved amount IS MiscAmountsBalances.OriginalAuthAmount
            // (tech spec Status Mappings). Without it the approved amount is unknown: fail closed,
            // never record the requested amount as authorized (INV-17).
            WorldpayraftDecision::PartialApproval => {
                let original_auth_amount = inner
                    .misc_amounts_balances
                    .as_ref()
                    .and_then(|amounts| amounts.original_auth_amount.clone())
                    .ok_or_else(|| {
                        error_stack::report!(errors::ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some("partial approval (ResponseCode 010) without MiscAmountsBalances.OriginalAuthAmount: the approved amount is unknown".to_string()),
                        ))
                    })?;
                Some(
                    common_utils::types::StringMajorUnitForConnector
                        .convert_back(original_auth_amount, request.currency)
                        .change_context(errors::ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some("could not convert MiscAmountsBalances.OriginalAuthAmount of a partial approval".to_string()),
                        ))?,
                )
            }
            WorldpayraftDecision::Approved
            | WorldpayraftDecision::HonorWithId
            | WorldpayraftDecision::RequestInProgress
            | WorldpayraftDecision::Declined
            | WorldpayraftDecision::MessageRejected => None,
        };

        // connector_transaction_id is the APITransactionID that was SENT: follow-ups reuse it.
        let api_transaction_id = derive_api_transaction_id(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        )
        .change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("failed to recompute the APITransactionID sent on RepeatPayment".to_string()),
            ),
        )?;

        // The card identity the request was built from (already validated there).
        let credential = mit_credential(request).change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some(
                    "failed to re-read the card identity of the RepeatPayment request".to_string(),
                ),
            ),
        )?;
        let response_token = inner.tokenized_pan();
        let network_transaction_ids = inner.network_ids();

        // A refund of a MIT needs the card token: the one returned, else the mandate token.
        let feature_data = WorldpayraftFeatureData {
            api_transaction_id: api_transaction_id.clone(),
            operation,
            card_kind: WorldpayraftCardKind::Credit,
            authorized_amount: partial_amount.unwrap_or(request.minor_amount),
            captured_amount: None,
            currency: request.currency,
            tokenized_pan: response_token
                .clone()
                .or_else(|| credential.tokenized_pan.clone()),
            expiration_date: Some(credential.expiration_date.clone()),
            authorization_number: inner.authorization_number(),
            retrieval_ref_number: inner
                .reference_trace_numbers
                .as_ref()
                .and_then(|trace| trace.retrieval_ref_number.clone()),
            card_network: credential.card_network,
            network_transaction_ids: network_transaction_ids.clone(),
        };
        let connector_metadata = serde_json::to_value(&feature_data).change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("failed to serialize Worldpay RAFT connector_feature_data".to_string()),
            ),
        )?;

        // An off-session MIT that returned a token refreshes the stored credential, in the same
        // mandate_metadata shape SetupMandate writes. The NTIDs of the credential-setting CIT
        // (echoed on this MIT) are kept: the MIT's own ids are stored only when none were, and
        // stay in connector_feature_data either way.
        let stored_network_ids = credential
            .network_ids
            .clone()
            .or_else(|| network_transaction_ids.clone());
        let mandate_reference = response_token
            .filter(|_| request.off_session == Some(true))
            .map(|token| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(token.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: Some(build_mandate_metadata(
                        Some(&credential.expiration_date),
                        credential.card_network,
                        stored_network_ids.as_ref(),
                    )),
                })
            });

        // A purchase moves money: a partial approval reports the approved amount as both authorized
        // and captured, and a full approval reports the requested amount as captured. An
        // authorization reports only the authorized amount.
        let (minor_amount_authorized, captured_amount) = match (operation, decision) {
            (WorldpayraftOperation::Authorization, _) => (
                partial_amount.or(item
                    .router_data
                    .resource_common_data
                    .minor_amount_authorized),
                None,
            ),
            (WorldpayraftOperation::Purchase, WorldpayraftDecision::PartialApproval) => (
                partial_amount.or(item
                    .router_data
                    .resource_common_data
                    .minor_amount_authorized),
                partial_amount,
            ),
            (WorldpayraftOperation::Purchase, WorldpayraftDecision::Approved) => (
                item.router_data
                    .resource_common_data
                    .minor_amount_authorized,
                Some(request.minor_amount),
            ),
            (
                WorldpayraftOperation::Purchase,
                WorldpayraftDecision::HonorWithId
                | WorldpayraftDecision::RequestInProgress
                | WorldpayraftDecision::Declined
                | WorldpayraftDecision::MessageRejected,
            )
            | (WorldpayraftOperation::Verification, _) => (
                item.router_data
                    .resource_common_data
                    .minor_amount_authorized,
                None,
            ),
        };
        let minor_amount_captured =
            captured_amount.or(item.router_data.resource_common_data.minor_amount_captured);
        let amount_captured = captured_amount
            .map(MinorUnit::get_amount_as_i64)
            .or(item.router_data.resource_common_data.amount_captured);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(api_transaction_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: Some(connector_metadata),
                network_txn_id: inner.network_transaction_id(),
                network_txn_link_id: inner
                    .reference_trace_numbers
                    .as_ref()
                    .and_then(|trace| trace.transaction_link_id.clone()),
                connector_response_reference_id: Some(api_transaction_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: inner
                    .reference_trace_numbers
                    .as_ref()
                    .and_then(|trace| trace.payment_acct_ref_number.clone()),
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response: inner.card_connector_response(),
                minor_amount_authorized,
                minor_amount_captured,
                amount_captured,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// WEBHOOKS — Worldpay Event Notifications
// =============================================================================
// The Native RAFT API pushes nothing itself; `ProcFlagsIndicators.EventNotificationIndicator = "Y"`
// opts a transaction into the separate Worldpay Event Notifications product, whose envelope is
// identical across the four payment/dispute event specs (`authorizations.created` 1.0.42,
// `transaction.settlements.created` 1.0.11-beta, `transaction.disputecases.created` and
// `transaction.disputecases.status.updated` 2.0.1-beta). Only the fields the UCS webhook mapping
// consumes are deserialised; card, cardholder and address data in the payload are never read.
// `APITransactionID` is not a field of any Event Notifications payload, so payment events correlate
// on `customerFields.field1` (= `UserDefinedData.UserData1`, which every payment request fills with
// its own `APITransactionID`) and dispute events on the case id.

/// Issuers whose RS256 JWTs authenticate Event Notifications deliveries (cert and prod).
pub(super) const WEBHOOK_JWT_ISSUERS: [&str; 2] = [
    "https://apis.cert.worldpay.com/authentication",
    "https://apis.worldpay.com/authentication",
];

/// `Authorization: Bearer {jwt}` (Standard configuration) header name.
pub(super) const WEBHOOK_AUTH_HEADER: &str = "Authorization";
/// `AuthorizationToken: Bearer {jwt}` (Salesforce configuration) header name.
pub(super) const WEBHOOK_AUTH_HEADER_SALESFORCE: &str = "AuthorizationToken";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftWebhookEnvelope {
    pub event_type: WorldpayraftEventType,
    pub notification_id: String,
    pub event_count: Option<i64>,
    pub version: Option<String>,
    pub created_at: Option<String>,
    pub data: WorldpayraftWebhookData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldpayraftEventType {
    #[serde(rename = "authorizations.created")]
    AuthorizationsCreated,
    #[serde(rename = "transaction.settlements.created")]
    SettlementsCreated,
    #[serde(rename = "transaction.disputecases.created")]
    DisputeCasesCreated,
    #[serde(rename = "transaction.disputecases.status.updated")]
    DisputeCasesStatusUpdated,
    /// Gift settlements, account, product, embedded-finance and equipment events, and any
    /// event type Worldpay adds later: not payment or dispute events for this connector.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftWebhookData {
    pub authorizations: Option<Vec<WorldpayraftAuthorizationInfo>>,
    pub settlements: Option<Vec<WorldpayraftSettlementInfo>>,
    pub case_details: Option<WorldpayraftDisputeCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftAuthorizationInfo {
    pub transaction_status: Option<WorldpayraftWebhookTransactionStatus>,
    pub pre_auth_indicator: Option<bool>,
    pub customer_fields: Option<WorldpayraftWebhookCustomerFields>,
    pub auth_code: Option<String>,
    pub trace_number: Option<String>,
    pub card_network_fields: Option<WorldpayraftWebhookCardNetworkFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftWebhookTransactionStatus {
    pub code: Option<String>,
    pub short_description: Option<String>,
    /// "The authorization ISO response code that is only present in the case of a denial".
    pub denial: Option<WorldpayraftWebhookDenial>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftWebhookDenial {
    pub code: Option<String>,
    pub short_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftWebhookCustomerFields {
    /// Echo of `UserDefinedData.UserData1` (the connector request reference id).
    pub field1: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftWebhookCardNetworkFields {
    pub network_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftSettlementInfo {
    pub customer_fields: Option<WorldpayraftWebhookCustomerFields>,
    /// The settlement spec publishes no closed shape for `denial`; its presence is the signal.
    pub denial: Option<serde_json::Value>,
    pub draft_locator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayraftDisputeCase {
    pub id: String,
    pub status: WorldpayraftDisputeCaseStatus,
    pub stage: WorldpayraftDisputeStageInfo,
    pub action: WorldpayraftDisputeActionInfo,
    pub original_transaction_amount: Option<common_utils::types::FloatMajorUnit>,
    pub original_transaction_amount_currency_type: Option<common_enums::Currency>,
    pub reason: Option<WorldpayraftDisputeReason>,
    pub reply_by_date: Option<String>,
    pub result: Option<String>,
    pub source_system_case_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldpayraftDisputeCaseStatus {
    #[serde(rename = "OPEN")]
    Open,
    #[serde(rename = "CLOSED")]
    Closed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftDisputeStageInfo {
    pub code: WorldpayraftDisputeStageCode,
    pub description: Option<String>,
}

/// `CaseDetails.stage.code` — the 8 values published on `transaction.disputecases.status.updated`
/// (the 3 on `created` are a subset).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldpayraftDisputeStageCode {
    /// Compliance
    #[serde(rename = "ACF")]
    Acf,
    /// Pre-Compliance
    #[serde(rename = "APC")]
    Apc,
    /// Arbitration
    #[serde(rename = "ARB")]
    Arb,
    /// First Chargeback
    #[serde(rename = "CH1")]
    Ch1,
    /// Second Chargeback
    #[serde(rename = "CH2")]
    Ch2,
    /// Pre-arbitration
    #[serde(rename = "PAB")]
    Pab,
    /// Draft Retrieval
    #[serde(rename = "REQ")]
    Req,
    /// Representment
    #[serde(rename = "RE2")]
    Re2,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftDisputeActionInfo {
    pub code: WorldpayraftDisputeAction,
    pub description: Option<String>,
    pub amount: Option<common_utils::types::FloatMajorUnit>,
}

/// `CaseDetails.action.code` — the 28 values published on
/// `transaction.disputecases.status.updated` (the 4 on `created` are a subset).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldpayraftDisputeAction {
    /// Acquirer Liable
    #[serde(rename = "AQCO")]
    Aqco,
    /// Acquirer Write-Off
    #[serde(rename = "AQWO")]
    Aqwo,
    /// Second Chargeback
    #[serde(rename = "SCHG")]
    Schg,
    /// Incoming Retrieval Request
    #[serde(rename = "RREQ")]
    Rreq,
    /// Charge Merchant
    #[serde(rename = "CHGM")]
    Chgm,
    /// Credit Merchant
    #[serde(rename = "CRMR")]
    Crmr,
    /// Incoming Compliance
    #[serde(rename = "IACF")]
    Iacf,
    /// Issuer Accepts Liability
    #[serde(rename = "IACP")]
    Iacp,
    /// Incoming Arbitration
    #[serde(rename = "IARB")]
    Iarb,
    /// First Chargeback
    #[serde(rename = "FCHG")]
    Fchg,
    /// Issuer Declined Pre-Arbitration Request
    #[serde(rename = "IDCL")]
    Idcl,
    /// Case Decided in Issuers Favor
    #[serde(rename = "IFAV")]
    Ifav,
    /// Incoming Pre-Arbitration
    #[serde(rename = "IPAB")]
    Ipab,
    /// Representment
    #[serde(rename = "IREP")]
    Irep,
    /// Merchant Accepts Liability
    #[serde(rename = "MACP")]
    Macp,
    /// Merchant Accepts Liability
    #[serde(rename = "EACP")]
    Eacp,
    /// Incoming Request Declined
    #[serde(rename = "MDCL")]
    Mdcl,
    /// Case Decided in Merchant Favor
    #[serde(rename = "MFAV")]
    Mfav,
    /// Outgoing Arbitration
    #[serde(rename = "OARB")]
    Oarb,
    /// Outgoing Pre-Arbitration
    #[serde(rename = "OPAB")]
    Opab,
    /// Credit Clearing
    #[serde(rename = "PCHC")]
    Pchc,
    /// Prenote Credit
    #[serde(rename = "PCHP")]
    Pchp,
    /// Incoming Pre-Compliance
    #[serde(rename = "PCMP")]
    Pcmp,
    /// Pre-Arbitration Recalled
    #[serde(rename = "RCAL")]
    Rcal,
    /// Retrieval Refusal
    #[serde(rename = "RRER")]
    Rrer,
    /// Retrieval Request Response
    #[serde(rename = "RRSP")]
    Rrsp,
    /// Updated Representment
    #[serde(rename = "UPDT")]
    Updt,
    /// Case denied supply additional documentation
    #[serde(rename = "VDNL")]
    Vdnl,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftDisputeReason {
    pub code: Option<String>,
    pub description: Option<String>,
}

/// How a dispute action code maps onto a UCS dispute status (tech spec "Suggested UCS
/// dispute-status mapping"). The opening actions fix their own stage; the later actions take the
/// stage from `CaseDetails.stage.code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorldpayraftDisputeActionOutcome {
    Opened(common_enums::DisputeStage),
    Challenged,
    Won,
    Lost,
    /// No published UCS mapping (IDCL, RRER, PCHC, PCHP, RCAL) or an unrecognised code.
    Unmapped,
}

impl WorldpayraftDisputeAction {
    pub(super) fn outcome(self) -> WorldpayraftDisputeActionOutcome {
        use common_enums::DisputeStage;
        match self {
            Self::Rreq => WorldpayraftDisputeActionOutcome::Opened(DisputeStage::PreDispute),
            Self::Fchg | Self::Chgm => {
                WorldpayraftDisputeActionOutcome::Opened(DisputeStage::Dispute)
            }
            Self::Pcmp
            | Self::Schg
            | Self::Iarb
            | Self::Oarb
            | Self::Ipab
            | Self::Opab
            | Self::Iacf => WorldpayraftDisputeActionOutcome::Opened(DisputeStage::PreArbitration),
            Self::Irep | Self::Updt | Self::Rrsp => WorldpayraftDisputeActionOutcome::Challenged,
            Self::Mfav | Self::Aqco | Self::Iacp | Self::Crmr => {
                WorldpayraftDisputeActionOutcome::Won
            }
            Self::Ifav | Self::Macp | Self::Eacp | Self::Mdcl | Self::Vdnl | Self::Aqwo => {
                WorldpayraftDisputeActionOutcome::Lost
            }
            Self::Idcl | Self::Rrer | Self::Pchc | Self::Pchp | Self::Rcal | Self::Unknown => {
                WorldpayraftDisputeActionOutcome::Unmapped
            }
        }
    }
}

impl WorldpayraftDisputeStageCode {
    /// UCS has no separate arbitration stage: compliance and arbitration stages map to
    /// `PreArbitration`. An unrecognised stage has no mapping.
    pub(super) fn dispute_stage(self) -> Option<common_enums::DisputeStage> {
        use common_enums::DisputeStage;
        match self {
            Self::Req => Some(DisputeStage::PreDispute),
            Self::Ch1 | Self::Ch2 | Self::Re2 => Some(DisputeStage::Dispute),
            Self::Apc | Self::Acf | Self::Arb | Self::Pab => Some(DisputeStage::PreArbitration),
            Self::Unknown => None,
        }
    }
}

/// Parse the Event Notifications envelope from the raw webhook body.
pub(super) fn parse_webhook_envelope(
    body: &[u8],
) -> Result<WorldpayraftWebhookEnvelope, error_stack::Report<errors::WebhookError>> {
    serde_json::from_slice::<WorldpayraftWebhookEnvelope>(body)
        .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("Worldpay Event Notifications body is not a valid notification envelope")
}

/// Payment result carried by a payment event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorldpayraftPaymentEventOutcome {
    Denied,
    PreAuthorized,
    Charged,
}

impl WorldpayraftPaymentEventOutcome {
    pub(super) fn attempt_status(self) -> AttemptStatus {
        match self {
            Self::Denied => AttemptStatus::Failure,
            Self::PreAuthorized => AttemptStatus::Authorized,
            Self::Charged => AttemptStatus::Charged,
        }
    }

    fn event_type(self) -> domain_types::connector_types::EventType {
        use domain_types::connector_types::EventType;
        match self {
            Self::Denied => EventType::PaymentIntentFailure,
            Self::PreAuthorized => EventType::PaymentIntentAuthorizationSuccess,
            Self::Charged => EventType::PaymentIntentSuccess,
        }
    }
}

/// The payment item an `authorizations.created` / `transaction.settlements.created` event carries.
pub(super) enum WorldpayraftPaymentEventItem<'a> {
    Authorization(&'a WorldpayraftAuthorizationInfo),
    Settlement(&'a WorldpayraftSettlementInfo),
}

impl WorldpayraftPaymentEventItem<'_> {
    /// `customerFields.field1`: the payment's own `APITransactionID`, sent as `UserData1` — the
    /// same id Authorize reports as `connector_transaction_id`. Absent → the event cannot be tied
    /// to a payment (`WebhookReferenceIdNotFound`).
    pub(super) fn api_transaction_id(
        &self,
    ) -> Result<String, error_stack::Report<errors::WebhookError>> {
        let customer_fields = match self {
            Self::Authorization(authorization) => authorization.customer_fields.as_ref(),
            Self::Settlement(settlement) => settlement.customer_fields.as_ref(),
        };
        customer_fields
            .and_then(|fields| fields.field1.clone())
            .ok_or_else(|| {
                error_stack::report!(errors::WebhookError::WebhookReferenceIdNotFound)
                    .attach_printable(
                        "Worldpay payment notification carries no customerFields.field1 (UserData1 APITransactionID)",
                    )
            })
    }

    fn is_denied(&self) -> bool {
        match self {
            Self::Authorization(authorization) => authorization
                .transaction_status
                .as_ref()
                .is_some_and(|status| status.denial.is_some()),
            Self::Settlement(settlement) => settlement.denial.is_some(),
        }
    }

    /// What the event reports: a denial fails the payment; an approved authorization is a
    /// pre-authorization when `preAuthIndicator` is true and a sale otherwise; a settlement without
    /// a denial is a completed charge.
    pub(super) fn outcome(&self) -> WorldpayraftPaymentEventOutcome {
        if self.is_denied() {
            return WorldpayraftPaymentEventOutcome::Denied;
        }
        match self {
            Self::Authorization(authorization) => {
                // authorizations.created is emitted at authorization time and can be delivered
                // after a Capture: PreAuthorized maps to Authorized, and the consumer must not
                // downgrade a Charged payment on it (settlement events are the terminal truth).
                if authorization.pre_auth_indicator == Some(true) {
                    WorldpayraftPaymentEventOutcome::PreAuthorized
                } else {
                    WorldpayraftPaymentEventOutcome::Charged
                }
            }
            Self::Settlement(_) => WorldpayraftPaymentEventOutcome::Charged,
        }
    }

    /// Denial code and message; `None` when the event carries no denial.
    pub(super) fn denial_details(&self) -> Option<(String, String)> {
        match self {
            Self::Authorization(authorization) => authorization
                .transaction_status
                .as_ref()
                .and_then(|status| status.denial.as_ref())
                .map(|denial| {
                    (
                        denial
                            .code
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                        denial
                            .short_description
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                    )
                }),
            Self::Settlement(settlement) => settlement.denial.as_ref().map(|denial| {
                let text = |key: &str| {
                    denial
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                };
                (
                    text("code").unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                    text("shortDescription")
                        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                )
            }),
        }
    }

    pub(super) fn network_transaction_id(&self) -> Option<String> {
        match self {
            Self::Authorization(authorization) => authorization
                .card_network_fields
                .as_ref()
                .and_then(|fields| fields.network_transaction_id.clone()),
            Self::Settlement(_) => None,
        }
    }
}

impl WorldpayraftWebhookEnvelope {
    /// The first payment item of a payment event; `None` for every other event type.
    pub(super) fn payment_event_item(&self) -> Option<WorldpayraftPaymentEventItem<'_>> {
        match self.event_type {
            WorldpayraftEventType::AuthorizationsCreated => self
                .data
                .authorizations
                .as_ref()
                .and_then(|items| items.first())
                .map(WorldpayraftPaymentEventItem::Authorization),
            WorldpayraftEventType::SettlementsCreated => self
                .data
                .settlements
                .as_ref()
                .and_then(|items| items.first())
                .map(WorldpayraftPaymentEventItem::Settlement),
            WorldpayraftEventType::DisputeCasesCreated
            | WorldpayraftEventType::DisputeCasesStatusUpdated
            | WorldpayraftEventType::Other => None,
        }
    }

    /// The dispute case of a dispute event; `None` for every other event type.
    pub(super) fn dispute_case(&self) -> Option<&WorldpayraftDisputeCase> {
        match self.event_type {
            WorldpayraftEventType::DisputeCasesCreated
            | WorldpayraftEventType::DisputeCasesStatusUpdated => self.data.case_details.as_ref(),
            WorldpayraftEventType::AuthorizationsCreated
            | WorldpayraftEventType::SettlementsCreated
            | WorldpayraftEventType::Other => None,
        }
    }
}

/// Map an Event Notifications envelope onto a UCS webhook event type.
pub(super) fn get_webhook_event_type(
    envelope: &WorldpayraftWebhookEnvelope,
) -> Result<domain_types::connector_types::EventType, error_stack::Report<errors::WebhookError>> {
    use domain_types::connector_types::EventType;
    match envelope.event_type {
        WorldpayraftEventType::AuthorizationsCreated
        | WorldpayraftEventType::SettlementsCreated => {
            let item = envelope.payment_event_item().ok_or_else(|| {
                error_stack::report!(errors::WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("payment event carries no authorizations/settlements item")
            })?;
            Ok(item.outcome().event_type())
        }
        WorldpayraftEventType::DisputeCasesCreated
        | WorldpayraftEventType::DisputeCasesStatusUpdated => {
            let case = envelope.dispute_case().ok_or_else(|| {
                error_stack::report!(errors::WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("dispute event carries no caseDetails")
            })?;
            Ok(match case.action.code.outcome() {
                WorldpayraftDisputeActionOutcome::Opened(_) => EventType::DisputeOpened,
                WorldpayraftDisputeActionOutcome::Challenged => EventType::DisputeChallenged,
                WorldpayraftDisputeActionOutcome::Won => EventType::DisputeWon,
                WorldpayraftDisputeActionOutcome::Lost => EventType::DisputeLost,
                WorldpayraftDisputeActionOutcome::Unmapped => {
                    EventType::IncomingWebhookEventUnspecified
                }
            })
        }
        WorldpayraftEventType::Other => Ok(EventType::IncomingWebhookEventUnspecified),
    }
}
