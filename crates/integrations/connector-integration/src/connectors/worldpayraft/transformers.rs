use common_enums::{AttemptStatus, CardNetwork, RefundStatus};
use common_utils::{
    consts,
    types::{MinorUnit, StringMajorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, RepeatPayment, SetupMandate},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, RefundFlowData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors,
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    utils::{get_card_issuer, CardIssuer},
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::worldpayraft::WorldpayraftRouterData, types::ResponseRouterData};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Card type identifier as received in `PaymentMethodData` (compared case-insensitively).
pub(super) const CARD_TYPE_DEBIT: &str = "debit";

/// Prefix embedded in `connector_transaction_id` to identify debit transactions.
const TXN_TYPE_DEBIT: &str = "D";
/// Prefix embedded in `connector_transaction_id` to identify credit transactions.
const TXN_TYPE_CREDIT: &str = "C";
/// Number of `|`-separated segments in the composite `connector_transaction_id`.
const TXN_REFERENCE_SEGMENTS: usize = 6;

/// `ReturnCode` value meaning "the message itself was processed" (operation level).
const RETURN_CODE_SUCCESS: &str = "0000";
/// `ResponseCode` value meaning "approved" (issuer level).
const RESPONSE_CODE_APPROVED: &str = "000";
/// `ResponseCode` `010` — PARTIAL APPROVAL. The spec classifies this as an approval,
/// not a decline; the approved amount is echoed in `MiscAmountsBalances.OriginalAuthAmount`.
const RESPONSE_CODE_PARTIAL_APPROVAL: &str = "010";
/// `ResponseCode` `003` — HONOR WITH ID. Manual review, not a terminal decline.
const RESPONSE_CODE_HONOR_WITH_ID: &str = "003";
/// `ResponseCode` `114` — REQUEST IN PROGRESS. Non-terminal; the outcome is not yet known.
const RESPONSE_CODE_REQUEST_IN_PROGRESS: &str = "114";

/// Separator joining `McrdBanknetREFNUM` and `McrdBanknetSettleDate` inside the single
/// `network_txn_id` string. Neither field can contain `:` (both are numeric).
const MCRD_NTID_SEPARATOR: char = ':';

/// `TerminalData.EntryMode` for an e-commerce channel transaction.
const ENTRY_MODE_ECOMM: &str = "E-COMM";
/// `TerminalData.POSConditionCode` for an e-commerce channel transaction.
const POS_CONDITION_CODE_ECOMM: &str = "59";
/// `TerminalData.TerminalEntryCap` — not applicable for e-commerce.
const TERMINAL_ENTRY_CAP_DEFAULT: &str = "0";
/// `E-commerceData.E-commerceIndicator` `07` — e-commerce, no 3-D Secure.
const ECOMMERCE_INDICATOR_ECOMM_NO_3DS: &str = "07";

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
                            "Worldpay RAFT expects ConnectorSpecificConfig::Worldpayraft { license, merchant_id }"
                                .to_string(),
                        ),
                        ..Default::default()
                    }
                }
            )),
        }
    }
}

// =============================================================================
// CLOSED VALUE SETS
// =============================================================================

/// The `Y`/`N` flag shape used by every `ProcFlagsIndicators` member.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WorldpayraftFlag {
    #[serde(rename = "Y")]
    Yes,
    #[serde(rename = "N")]
    No,
}

/// `CardVerificationData.Cvv2Cvc2CIDIndicator` — the CVV2/CVC2/CID **presence** indicator.
///
/// Note the polarity: `1` means the value IS present, `0` means it was bypassed or not
/// given. (An earlier revision of this connector had these two inverted.)
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WorldpayraftCvvIndicator {
    /// `0` — the CVV2/CVC2/CID value was bypassed or not given.
    #[serde(rename = "0")]
    BypassedOrNotGiven,
    /// `1` — the CVV2/CVC2/CID value is present.
    #[serde(rename = "1")]
    Present,
    /// `2` — the CVV2/CVC2/CID value is illegible.
    #[serde(rename = "2")]
    Illegible,
    /// `9` — the CVV2/CVC2/CID value is not on the card.
    #[serde(rename = "9")]
    NotOnCard,
}

/// `TerminalData.POSEnvironment` — the stored-credential / credential-on-file indicator.
///
/// Request-only and credit-only: `POSEnvironment` does not exist in the debit spec.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WorldpayraftPosEnvironment {
    /// `C` — Credential on File transaction.
    #[serde(rename = "C")]
    CredentialOnFile,
    /// `I` — Installment transaction for Visa.
    #[serde(rename = "I")]
    Installment,
    /// `R` — Recurring for Visa/Discover, Standing Order for Mastercard (amount may vary).
    #[serde(rename = "R")]
    Recurring,
    /// `S` — Subscription for Mastercard, Recurring for Visa/Discover (fixed amount).
    #[serde(rename = "S")]
    Subscription,
    /// `U` — Unscheduled card on file (merchant-initiated).
    #[serde(rename = "U")]
    UnscheduledCardOnFile,
}

impl From<common_enums::MitCategory> for WorldpayraftPosEnvironment {
    fn from(category: common_enums::MitCategory) -> Self {
        match category {
            common_enums::MitCategory::Installment => Self::Installment,
            common_enums::MitCategory::Recurring => Self::Subscription,
            common_enums::MitCategory::Unscheduled => Self::UnscheduledCardOnFile,
            // A retry of a previously declined MIT re-uses the stored credential without
            // establishing a new schedule, so the generic credential-on-file value applies.
            common_enums::MitCategory::Resubmission => Self::CredentialOnFile,
        }
    }
}

/// `*SubsequentTransactionReasonCode` — the identical nine-value enum carried by all four
/// `*SpecificData` objects. Request only, `maxLength: 2`.
///
/// The published set has no value for a scheduled recurring / installment / unscheduled
/// card-on-file MIT, so this connector only emits the one value it can derive without
/// inventing a meaning: `Resubmission`, for `MitCategory::Resubmission`.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WorldpayraftSubsequentTransactionReasonCode {
    /// `13` — Below floor limit.
    #[serde(rename = "13")]
    BelowFloorLimit,
    /// `17` — VRU Approved.
    #[serde(rename = "17")]
    VruApproved,
    /// `40` — Incremental Authorization.
    #[serde(rename = "40")]
    IncrementalAuthorization,
    /// `41` — Resubmission.
    #[serde(rename = "41")]
    Resubmission,
    /// `42` — Delayed Charge.
    #[serde(rename = "42")]
    DelayedCharge,
    /// `43` — Reauthorization.
    #[serde(rename = "43")]
    Reauthorization,
    /// `44` — No Show.
    #[serde(rename = "44")]
    NoShow,
    /// `45` — Deferred Authorization.
    #[serde(rename = "45")]
    DeferredAuthorization,
    /// `50` — Offline Chip Approval.
    #[serde(rename = "50")]
    OfflineChipApproval,
}

impl TryFrom<common_enums::MitCategory> for WorldpayraftSubsequentTransactionReasonCode {
    type Error = ();

    fn try_from(category: common_enums::MitCategory) -> Result<Self, Self::Error> {
        match category {
            common_enums::MitCategory::Resubmission => Ok(Self::Resubmission),
            // No published RAFT reason code describes a scheduled recurring, installment or
            // unscheduled card-on-file MIT; the field is optional, so it is omitted instead.
            common_enums::MitCategory::Installment
            | common_enums::MitCategory::Recurring
            | common_enums::MitCategory::Unscheduled => Err(()),
        }
    }
}

/// The four card networks for which Native RAFT publishes a network transaction id.
///
/// None of the four `*SpecificData` objects exists in the debit spec, so NTIDs are a
/// credit-path concept only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldpayraftCardBrand {
    Visa,
    Mastercard,
    Amex,
    Discover,
}

impl WorldpayraftCardBrand {
    fn from_card_network(network: &CardNetwork) -> Option<Self> {
        match network {
            CardNetwork::Visa => Some(Self::Visa),
            CardNetwork::Mastercard | CardNetwork::Maestro => Some(Self::Mastercard),
            CardNetwork::AmericanExpress => Some(Self::Amex),
            CardNetwork::Discover => Some(Self::Discover),
            CardNetwork::JCB
            | CardNetwork::DinersClub
            | CardNetwork::CartesBancaires
            | CardNetwork::UnionPay
            | CardNetwork::Interac
            | CardNetwork::RuPay
            | CardNetwork::Star
            | CardNetwork::Pulse
            | CardNetwork::Accel
            | CardNetwork::Nyce
            | CardNetwork::Prop
            | CardNetwork::PrivateLabel
            | CardNetwork::Dinacard => None,
        }
    }

    fn from_card_issuer(issuer: CardIssuer) -> Option<Self> {
        match issuer {
            CardIssuer::Visa => Some(Self::Visa),
            CardIssuer::Master | CardIssuer::Maestro => Some(Self::Mastercard),
            CardIssuer::AmericanExpress => Some(Self::Amex),
            CardIssuer::Discover => Some(Self::Discover),
            CardIssuer::DinersClub
            | CardIssuer::JCB
            | CardIssuer::CarteBlanche
            | CardIssuer::CartesBancaires
            | CardIssuer::UnionPay => None,
        }
    }

    /// Resolve the brand for the per-brand NTID echo. The caller-supplied `card_network`
    /// is authoritative; when it is absent the brand is derived from the PAN's BIN range
    /// using the shared `domain_types::utils::get_card_issuer` helper.
    fn resolve<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>(
        card: &Card<T>,
    ) -> Option<Self> {
        card.card_network
            .as_ref()
            .and_then(Self::from_card_network)
            .or_else(|| {
                get_card_issuer(card.card_number.peek())
                    .ok()
                    .and_then(Self::from_card_issuer)
            })
    }
}

// =============================================================================
// RESPONSE CODE TABLES
// =============================================================================

/// Published meaning of a `ReturnCode` (operation level). The set is closed at four values.
fn return_code_meaning(code: &str) -> Option<&'static str> {
    match code {
        "0000" => Some("SUCCESSFUL"),
        "0004" => Some("EDIT ERROR ON INPUT"),
        "0008" => Some("LOGIC ERROR"),
        "0012" => Some("SYSTEM ISSUE"),
        _ => None,
    }
}

/// Published meaning of a `ResponseCode` (issuer / decision level).
///
/// The set is explicitly open — "Worldpay can add response codes at any time. Any response
/// code not recognized should be treated as a decline" — so this is a lookup returning
/// `None` for an unknown code rather than an enum.
fn response_code_meaning(code: &str) -> Option<&'static str> {
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
        "100" => Some("IDEMPOTENCY DETECTED A DUPLICATE REQUEST BUT THERE WAS A MESSAGE TYPE MISMATCH BETWEEN WHAT IT LOCATED AND WHAT WAS SENT IN"),
        "101" => Some("A REQUEST FOR PINLESS CONVERSION FAILED TO FIND A VALID ROUTING OPTION"),
        "102" => Some("ACCOUNT CLOSED"),
        "103" => Some("TRANSACTION FEE NOT PERMITTED OR INVALID"),
        "104" => Some("CASH BACK REQUEST EXCEEDS ISSUER LIMIT"),
        "105" => Some("UNABLE TO LOCATE PREVIOUS TRANSACTION"),
        "106" => Some("PREVIOUS TRANSACTION LOCATED, BUT DATA INCONSISTENT"),
        "107" => Some("DECLINED FIRST USE OF CARD"),
        "108" => Some("TRANSACTION AMOUNT EXCEEDS PREAUTHORIZED AMOUNT"),
        "109" => Some("STOP PAYMENT ORDER"),
        "110" => Some("EXPIRATION DATE MISMATCH"),
        "111" => Some("STALE DATED TRANSACTION"),
        "112" => Some("INVALID ADDRESS VERIFICATION INFORMATION"),
        "113" => Some("CUTOFF IS IN PROGRESS"),
        "114" => Some("REQUEST IN PROGRESS"),
        "115" => Some("INFORMATION NOT ON FILE"),
        "116" => Some("TOKEN LOOKUP FAILURE"),
        "117" => Some("CARDHOLDER DOES NOT PARTICIPATE IN ATTEMPTED PRODUCT"),
        "118" => Some("SPECIAL CONDITIONS"),
        "550" => Some("TRANSACTION DECLINED BY FRAUDSIGHT"),
        _ => None,
    }
}

// =============================================================================
// SHARED HELPERS
// =============================================================================

/// Returns the current UTC datetime formatted as `YYYY-MM-DDTHH:MM:SS` (`LocalDateTime`).
fn get_local_datetime() -> String {
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

/// Truncates a string to at most 16 characters (`APITransactionID` `maxLength`).
fn truncate_api_transaction_id(id: &str) -> String {
    id.chars().take(16).collect()
}

/// `true` when the card is flagged as a debit card, which routes the request onto the
/// `/debit/*` family of endpoints.
pub(super) fn is_debit_card<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
) -> bool {
    matches!(
        payment_method_data,
        PaymentMethodData::Card(card) if card.card_type.as_deref()
            .is_some_and(|card_type| card_type.eq_ignore_ascii_case(CARD_TYPE_DEBIT))
    )
}

/// Decide between the auto-capture endpoint (`/credit/purchase`, `/debit/purchase`) and the
/// auth-only endpoint (`/credit/authorization`, `/debit/preauth`).
///
/// Delegates to the shared [`PaymentsAuthorizeData::is_auto_capture`] helper, which maps
/// `Automatic | SequentialAutomatic | None` to `true` and `Manual` to `false`.
/// `ManualMultiple` and `Scheduled` have no RAFT endpoint and are rejected up front rather
/// than silently falling through to the auth-only path.
pub(super) fn resolve_auto_capture<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<bool, error_stack::Report<errors::IntegrationError>> {
    reject_unsupported_capture_method(request.capture_method)?;
    Ok(request.is_auto_capture())
}

/// The RepeatPayment counterpart of [`resolve_auto_capture`]. A merchant-initiated
/// transaction is still a purchase or an auth-only message, chosen the same way.
pub(super) fn resolve_repeat_auto_capture<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &RepeatPaymentData<T>,
) -> Result<bool, error_stack::Report<errors::IntegrationError>> {
    reject_unsupported_capture_method(request.capture_method)?;
    Ok(request.is_auto_capture())
}

/// `ManualMultiple` and `Scheduled` have no RAFT endpoint. Rejecting them here keeps
/// `is_auto_capture()` — which reports both as "not automatic" — from silently routing
/// them onto the auth-only path.
fn reject_unsupported_capture_method(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::ManualMultiple)
        | Some(common_enums::CaptureMethod::Scheduled) => Err(error_stack::report!(
            errors::IntegrationError::CaptureMethodNotSupported {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(format!(
                        "Worldpay RAFT expresses capture mode by endpoint choice and has no \
                         endpoint for capture_method {capture_method:?}; only Automatic, \
                         SequentialAutomatic and Manual are supported"
                    )),
                    ..Default::default()
                },
            }
        )),
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | Some(common_enums::CaptureMethod::Manual)
        | None => Ok(()),
    }
}

// =============================================================================
// COMPOSITE CONNECTOR TRANSACTION ID
// =============================================================================

/// Everything a follow-up message (capture, refund, reversal) needs in order to be matched
/// back to its original transaction by Worldpay.
///
/// **Why this is packed into `connector_transaction_id` rather than `connector_metadata`:**
/// RAFT matches follow-ups on the *request-side* `APITransactionID` that the acquirer sent
/// on the original message — not on anything Worldpay mints — and it additionally requires
/// the original `LocalDateTime` and, for `creditcompletion`, the originally authorized
/// amount as `PreauthorizedAmount`. `PaymentsCaptureData` and `RefundsData` carry no
/// `connector_metadata` field, so `connector_transaction_id` is the only channel that
/// reaches a follow-up flow. The same record is *also* published on the authorize response
/// as structured `connector_metadata` for observability and webhook correlation.
///
/// Wire format (six `|`-separated segments, none of which can contain `|`):
/// `{C|D}|{APITransactionID}|{LocalDateTime}|{authorized minor amount}|{AuthorizationNumber}|{RetrievalREFNumber}`
#[derive(Debug, Clone)]
pub(super) struct WorldpayraftTransactionReference {
    /// Routes the follow-up to `/debit/*` instead of `/credit/*`.
    pub is_debit: bool,
    /// The `APITransactionID` **sent** on the original message; replayed verbatim.
    pub api_transaction_id: String,
    /// The merchant-local timestamp recorded for the original message and replayed on
    /// every follow-up.
    ///
    /// RAFT does not echo `LocalDateTime`, and `RouterDataV2` does not carry the serialized
    /// request into the response transformer, so this is regenerated when the original
    /// response is handled — one round trip after the value that actually went on the wire.
    /// `APITransactionID` is the primary matching key; `LocalDateTime` is secondary.
    pub local_date_time: String,
    /// The originally authorized amount in minor units, needed as `PreauthorizedAmount`.
    /// Empty for operations that carry no amount (tokenization).
    pub authorized_minor_amount: Option<i64>,
    /// `ReferenceTraceNumbers.AuthorizationNumber` from the original response — the 6-char
    /// issuer approval code, used as a secondary matching key.
    pub authorization_number: Option<String>,
    /// `ReferenceTraceNumbers.RetrievalREFNumber` from the original response — the
    /// request-side lifecycle trace id, used as a secondary matching key.
    pub retrieval_ref_number: Option<String>,
}

impl WorldpayraftTransactionReference {
    fn encode(&self) -> String {
        let prefix = if self.is_debit {
            TXN_TYPE_DEBIT
        } else {
            TXN_TYPE_CREDIT
        };
        format!(
            "{}|{}|{}|{}|{}|{}",
            prefix,
            self.api_transaction_id,
            self.local_date_time,
            self.authorized_minor_amount
                .map(|amount| amount.to_string())
                .unwrap_or_default(),
            self.authorization_number.as_deref().unwrap_or_default(),
            self.retrieval_ref_number.as_deref().unwrap_or_default(),
        )
    }

    pub(super) fn parse(raw: &str) -> Result<Self, error_stack::Report<errors::IntegrationError>> {
        let invalid = || {
            error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                field_name: "connector_transaction_id",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(format!(
                        "Worldpay RAFT expects the composite reference \
                         '[C|D]|APITransactionID|LocalDateTime|authorized_minor_amount|\
                         AuthorizationNumber|RetrievalREFNumber', got {raw:?}"
                    )),
                    ..Default::default()
                },
            })
        };

        let segments: Vec<&str> = raw.splitn(TXN_REFERENCE_SEGMENTS, '|').collect();
        if segments.len() != TXN_REFERENCE_SEGMENTS {
            return Err(invalid());
        }
        let is_debit = match segments[0] {
            TXN_TYPE_DEBIT => true,
            TXN_TYPE_CREDIT => false,
            _ => return Err(invalid()),
        };
        if segments[1].is_empty() || segments[2].is_empty() {
            return Err(invalid());
        }
        let authorized_minor_amount = match segments[3] {
            "" => None,
            amount => Some(amount.parse::<i64>().map_err(|_| invalid())?),
        };

        Ok(Self {
            is_debit,
            api_transaction_id: segments[1].to_string(),
            local_date_time: segments[2].to_string(),
            authorized_minor_amount,
            authorization_number: non_empty(segments[4]),
            retrieval_ref_number: non_empty(segments[5]),
        })
    }

    /// The request-side `ReferenceTraceNumbers` for a follow-up message. Only real values
    /// received from Worldpay are replayed; `None` when neither is known, so the object is
    /// omitted rather than sent with empty strings.
    fn follow_up_trace_numbers(&self) -> Option<WorldpayraftRequestTraceNumbers> {
        if self.authorization_number.is_none() && self.retrieval_ref_number.is_none() {
            return None;
        }
        Some(WorldpayraftRequestTraceNumbers {
            retrieval_ref_number: self.retrieval_ref_number.clone(),
            authorization_number: self.authorization_number.clone(),
            economically_related_link_id: None,
        })
    }
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

// =============================================================================
// SHARED REQUEST STRUCTS
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftAmounts {
    /// `MiscAmountsBalances.TransactionAmount` — required on every financial operation.
    pub transaction_amount: StringMajorUnit,
    /// `MiscAmountsBalances.PreauthorizedAmount` — the amount the original authorization
    /// was approved for. **Required** on `creditcompletion` / `debitcompletion`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preauthorized_amount: Option<StringMajorUnit>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCardInfo<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "PAN")]
    pub pan: RawCardNumber<T>,
    pub expiration_date: Secret<String>,
}

/// `CardInfo` for flows that carry an already-materialised PAN or network token string
/// rather than a generic `RawCardNumber<T>`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftPlainCardInfo {
    #[serde(skip_serializing_if = "Option::is_none", rename = "PAN")]
    pub pan: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftCardVerificationData {
    /// `CardVerificationData.Cvv2Cvc2CIDIndicator` — presence indicator (`1` = present).
    #[serde(rename = "Cvv2Cvc2CIDIndicator")]
    pub cvv_indicator: WorldpayraftCvvIndicator,
    /// `CardVerificationData.Cvv2Cvc2CIDValue` — the security code itself, `maxLength: 4`.
    /// (There is no field named `CVV2CVC2` anywhere in the Native RAFT specifications.)
    #[serde(rename = "Cvv2Cvc2CIDValue")]
    pub cvv2_cvc2_cid_value: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftAddressVerificationData {
    #[serde(skip_serializing_if = "Option::is_none", rename = "AVSZIPCode")]
    pub avs_zip_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "AVSAddress")]
    pub avs_address: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftTerminalData {
    pub entry_mode: String,
    #[serde(rename = "POSConditionCode")]
    pub pos_condition_code: String,
    pub terminal_entry_cap: String,
    /// `TerminalData.POSEnvironment` — stored-credential indicator. Request-only and
    /// credit-only; omitted for one-off payments and for every `/debit/*` endpoint.
    #[serde(skip_serializing_if = "Option::is_none", rename = "POSEnvironment")]
    pub pos_environment: Option<WorldpayraftPosEnvironment>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftEcommerceData {
    #[serde(rename = "E-commerceIndicator")]
    pub ecommerce_indicator: String,
}

/// Request-side `ReferenceTraceNumbers`.
///
/// `SystemTraceNumber` is deliberately absent: the spec declares it response-only
/// ("This value will be generated by Worldpay") and it is not in the request schema of any
/// credit or debit operation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftRequestTraceNumbers {
    /// `RetrievalREFNumber` — merchant-generated lifecycle id, `maxLength: 12`. Worldpay
    /// generates one when the acquirer omits it, so it is only sent on a follow-up, where
    /// the value received on the original response is replayed.
    #[serde(skip_serializing_if = "Option::is_none", rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
    /// `AuthorizationNumber` — the 6-char **issuer approval code** from the original
    /// response. Never a connector transaction id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_number: Option<String>,
    /// `EconomicallyRelatedLinkID` — request only, `maxLength: 36`. Carries the
    /// Mastercard/Maestro `TransactionLinkID` received on the original CIT so the network
    /// can tie a subsequent merchant-initiated transaction back to it.
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "EconomicallyRelatedLinkID"
    )]
    pub economically_related_link_id: Option<String>,
}

/// Request-side `ProcFlagsIndicators` — only the members this connector sets.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftProcFlagsIndicators {
    /// `MastercardAdviceCodeIndicator` — opt-in for
    /// `McrdSpecificData.MastercardMerchantAdviceCode`. Without it the advice code is
    /// never returned. Credit-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mastercard_advice_code_indicator: Option<WorldpayraftFlag>,
    /// `CardholderInitiatedTransaction` — the CIT that establishes a stored credential.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_initiated_transaction: Option<WorldpayraftFlag>,
    /// `MerchantInitiatedTransaction` — required on every MIT.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_initiated_transaction: Option<WorldpayraftFlag>,
    /// `RecurringBillPay` — the transaction is a recurring bill payment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_bill_pay: Option<WorldpayraftFlag>,
}

/// Request-side `VisaSpecificData` — the NTID echo on a subsequent MIT.
#[derive(Debug, Serialize)]
pub struct WorldpayraftVisaRequestData {
    /// `VisaTransactionId`, `maxLength: 15` — the reference number assigned by Visa on the
    /// original CIT.
    #[serde(rename = "VisaTransactionId")]
    pub visa_transaction_id: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "VisaSubsequentTransactionReasonCode"
    )]
    pub visa_subsequent_transaction_reason_code:
        Option<WorldpayraftSubsequentTransactionReasonCode>,
}

/// Request-side `McrdSpecificData` — the NTID echo on a subsequent MIT.
///
/// Mastercard requires **both** Banknet fields; the reference number alone is not enough.
#[derive(Debug, Serialize)]
pub struct WorldpayraftMcrdRequestData {
    /// `McrdBanknetREFNUM`, `maxLength: 9`. The all-uppercase `REFNUM` is the spec spelling.
    #[serde(rename = "McrdBanknetREFNUM")]
    pub mcrd_banknet_refnum: String,
    /// `McrdBanknetSettleDate`, `maxLength: 4`, `MMDD`.
    #[serde(rename = "McrdBanknetSettleDate")]
    pub mcrd_banknet_settle_date: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "McrdSubsequentTransactionReasonCode"
    )]
    pub mcrd_subsequent_transaction_reason_code:
        Option<WorldpayraftSubsequentTransactionReasonCode>,
}

/// Request-side `AmexSpecificData` — the NTID echo on a subsequent MIT.
#[derive(Debug, Serialize)]
pub struct WorldpayraftAmexRequestData {
    /// `AmexTransactionId`, `maxLength: 15`.
    #[serde(rename = "AmexTransactionId")]
    pub amex_transaction_id: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "AmexSubsequentTransactionReasonCode"
    )]
    pub amex_subsequent_transaction_reason_code:
        Option<WorldpayraftSubsequentTransactionReasonCode>,
}

/// Request-side `DiscSpecificData` — the NTID echo on a subsequent MIT.
#[derive(Debug, Serialize)]
pub struct WorldpayraftDiscRequestData {
    /// `DiscTransactionId`, `maxLength: 15`.
    #[serde(rename = "DiscTransactionId")]
    pub disc_transaction_id: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "DiscSubsequentTransactionReasonCode"
    )]
    pub disc_subsequent_transaction_reason_code:
        Option<WorldpayraftSubsequentTransactionReasonCode>,
}

/// The per-brand `*SpecificData` blocks that carry the network transaction id on a
/// subsequent merchant-initiated transaction. Exactly one is populated.
#[derive(Debug, Default, Serialize)]
pub struct WorldpayraftBrandRequestData {
    #[serde(skip_serializing_if = "Option::is_none", rename = "VisaSpecificData")]
    pub visa_specific_data: Option<WorldpayraftVisaRequestData>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "McrdSpecificData")]
    pub mcrd_specific_data: Option<WorldpayraftMcrdRequestData>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "AmexSpecificData")]
    pub amex_specific_data: Option<WorldpayraftAmexRequestData>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "DiscSpecificData")]
    pub disc_specific_data: Option<WorldpayraftDiscRequestData>,
}

/// `EncryptionTokenData` on a request — carries a stored Worldpay token.
#[derive(Debug, Serialize)]
pub struct WorldpayraftEncryptionTokenRequestData {
    /// `TokenizedPAN` — a Worldpay token goes here, **not** in `CardInfo.PAN`.
    #[serde(rename = "TokenizedPAN")]
    pub tokenized_pan: Secret<String>,
}

// =============================================================================
// SHARED RESPONSE STRUCTS
// =============================================================================

/// `ErrorInformation` — present when `ReturnCode` is a non-zero (edit/logic) error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftErrorInformation {
    /// `FieldInError` — the field flagged in error during transaction processing.
    pub field_in_error: Option<String>,
    /// `ErrorText` — the portion of the field data detected as being in error.
    pub error_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftResponseTraceNumbers {
    /// `AuthorizationNumber` — 6-char issuer approval code.
    pub authorization_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
    /// `SystemTraceNumber` — Worldpay-generated, response only. Persisted for reporting
    /// and webhook correlation; never echoed on a request.
    pub system_trace_number: Option<String>,
    /// `NetworkTraceNumber` — the trace number used between Worldpay and the network.
    pub network_trace_number: Option<String>,
    /// `NetworkRefNumber` — the retrieval reference number used between Worldpay and the
    /// network.
    pub network_ref_number: Option<String>,
    /// `TransactionLinkID` — Mastercard/Maestro lifecycle id.
    #[serde(rename = "TransactionLinkID")]
    pub transaction_link_id: Option<String>,
    /// `PaymentAcctREFNumber` — the PAR (Payment Account Reference).
    #[serde(rename = "PaymentAcctREFNumber")]
    pub payment_acct_ref_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftResponseAmounts {
    /// `OriginalAuthAmount` — on a partial approval (`ResponseCode 010`) this is the amount
    /// actually authorized.
    pub original_auth_amount: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftVisaResponseData {
    /// `VisaTransactionId` — Visa's network transaction id.
    #[serde(rename = "VisaTransactionId")]
    pub visa_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftMcrdResponseData {
    /// `McrdBanknetREFNUM` — Mastercard Banknet Reference Number.
    #[serde(rename = "McrdBanknetREFNUM")]
    pub mcrd_banknet_refnum: Option<String>,
    /// `McrdBanknetSettleDate` — Mastercard settlement date, `MMDD`.
    #[serde(rename = "McrdBanknetSettleDate")]
    pub mcrd_banknet_settle_date: Option<String>,
    /// `MastercardMerchantAdviceCode` — DE48 SE84 merchant advice code. Response-only,
    /// Mastercard-only, credit-only, and returned **only** when the request carried
    /// `ProcFlagsIndicators.MastercardAdviceCodeIndicator = "Y"`.
    #[serde(rename = "MastercardMerchantAdviceCode")]
    pub mastercard_merchant_advice_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftAmexResponseData {
    /// `AmexTransactionId` — American Express network transaction id.
    #[serde(rename = "AmexTransactionId")]
    pub amex_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftDiscResponseData {
    /// `DiscTransactionId` — Discover reference number.
    #[serde(rename = "DiscTransactionId")]
    pub disc_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftEncryptionTokenData {
    #[serde(rename = "TokenizedPAN")]
    pub tokenized_pan: Option<String>,
    #[serde(rename = "PAN-Last4")]
    pub pan_last4: Option<String>,
}

/// The body of any Native RAFT financial or tokenization response.
///
/// Every RAFT response is wrapped in a single operation key (`creditauthresponse`,
/// `creditpurchaseresponse`, …) and the payloads are structurally identical supersets of
/// one another, so one struct serves every flow. Only `ReturnCode` is required by the
/// spec; everything else — `ResponseCode` included — can be absent when the message was
/// rejected before it reached the issuer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftResponseInner {
    /// `ReturnCode` — operation level. Any non-`0000` value means the message failed.
    pub return_code: String,
    /// `ReasonCode` — an opaque internal pinpoint code with no published enumeration.
    /// Never surfaced as the merchant-facing message.
    pub reason_code: Option<String>,
    /// `ResponseCode` — issuer / decision level. Absent when `ReturnCode != "0000"`.
    pub response_code: Option<String>,
    /// `ReturnText` — the human-readable failure description. Present only when
    /// `ReturnCode` is non-zero.
    pub return_text: Option<String>,
    pub error_information: Option<WorldpayraftErrorInformation>,
    pub reference_trace_numbers: Option<WorldpayraftResponseTraceNumbers>,
    pub misc_amounts_balances: Option<WorldpayraftResponseAmounts>,
    #[serde(rename = "VisaSpecificData")]
    pub visa_specific_data: Option<WorldpayraftVisaResponseData>,
    #[serde(rename = "McrdSpecificData")]
    pub mcrd_specific_data: Option<WorldpayraftMcrdResponseData>,
    #[serde(rename = "AmexSpecificData")]
    pub amex_specific_data: Option<WorldpayraftAmexResponseData>,
    #[serde(rename = "DiscSpecificData")]
    pub disc_specific_data: Option<WorldpayraftDiscResponseData>,
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenData>,
    /// `APITransactionID` — echoed back, left zero-padded to 16 characters.
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
}

impl WorldpayraftResponseInner {
    /// `ReturnCode == "0000"` AND `ResponseCode` is an approval (`000`, or `010` partial).
    fn is_success(&self) -> bool {
        self.return_code == RETURN_CODE_SUCCESS
            && matches!(
                self.response_code.as_deref(),
                Some(RESPONSE_CODE_APPROVED) | Some(RESPONSE_CODE_PARTIAL_APPROVAL)
            )
    }

    /// `ErrorResponse.code`: the issuer decision code when the message reached the issuer,
    /// otherwise the operation-level `ReturnCode`.
    fn error_code(&self) -> String {
        self.response_code
            .as_deref()
            .filter(|code| !code.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                if self.return_code.is_empty() {
                    consts::NO_ERROR_CODE.to_string()
                } else {
                    self.return_code.clone()
                }
            })
    }

    /// `ErrorResponse.message` / `network_error_message`: `ReturnText` when Worldpay sent
    /// one, otherwise the published meaning of the code that failed. `ReasonCode` is never
    /// used here — it has no published enumeration.
    fn error_message(&self) -> String {
        self.return_text
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .or_else(|| {
                self.response_code
                    .as_deref()
                    .and_then(response_code_meaning)
                    .map(str::to_string)
            })
            .or_else(|| return_code_meaning(&self.return_code).map(str::to_string))
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string())
    }

    /// `ErrorResponse.reason`: the `ErrorInformation` pair when present — that is the only
    /// field that pinpoints the offending request data — else the opaque `ReasonCode`.
    fn error_reason(&self) -> Option<String> {
        match self.error_information.as_ref() {
            Some(info) => match (info.field_in_error.as_deref(), info.error_text.as_deref()) {
                (Some(field), Some(text)) => Some(format!("{field}: {text}")),
                (Some(field), None) => Some(field.to_string()),
                (None, Some(text)) => Some(text.to_string()),
                (None, None) => self.reason_code.clone(),
            },
            None => self.reason_code.clone(),
        }
    }

    /// `McrdSpecificData.MastercardMerchantAdviceCode` — the Mastercard retry advice code.
    fn network_advice_code(&self) -> Option<String> {
        self.mcrd_specific_data
            .as_ref()
            .and_then(|mcrd| mcrd.mastercard_merchant_advice_code.clone())
    }

    /// The network transaction id for a future MIT, in brand precedence order
    /// (Visa → Mastercard → Amex → Discover).
    ///
    /// Mastercard is the exception to "one id": a subsequent MIT must echo **both**
    /// `McrdBanknetREFNUM` and `McrdBanknetSettleDate`, and `network_txn_id` is a single
    /// string. The two are therefore joined as `<McrdBanknetREFNUM>:<McrdBanknetSettleDate>`
    /// and split apart again when the MIT request is built. Both also appear separately in
    /// `connector_metadata`.
    fn network_transaction_id(&self) -> Option<String> {
        self.visa_specific_data
            .as_ref()
            .and_then(|visa| visa.visa_transaction_id.clone())
            .or_else(|| {
                self.mcrd_specific_data.as_ref().and_then(|mcrd| {
                    match (
                        mcrd.mcrd_banknet_refnum.as_deref(),
                        mcrd.mcrd_banknet_settle_date.as_deref(),
                    ) {
                        (Some(refnum), Some(settle_date)) => {
                            Some(format!("{refnum}{MCRD_NTID_SEPARATOR}{settle_date}"))
                        }
                        // A Banknet reference number without its settlement date cannot be
                        // replayed on a Mastercard MIT, so it is not offered as an NTID.
                        (Some(_), None) | (None, Some(_)) | (None, None) => None,
                    }
                })
            })
            .or_else(|| {
                self.amex_specific_data
                    .as_ref()
                    .and_then(|amex| amex.amex_transaction_id.clone())
            })
            .or_else(|| {
                self.disc_specific_data
                    .as_ref()
                    .and_then(|disc| disc.disc_transaction_id.clone())
            })
    }

    /// `ReferenceTraceNumbers.TransactionLinkID` — the Mastercard/Maestro lifecycle id.
    fn transaction_link_id(&self) -> Option<String> {
        self.reference_trace_numbers
            .as_ref()
            .and_then(|trace| trace.transaction_link_id.clone())
    }

    /// `ReferenceTraceNumbers.PaymentAcctREFNumber` — the PAR.
    fn payment_account_reference(&self) -> Option<String> {
        self.reference_trace_numbers
            .as_ref()
            .and_then(|trace| trace.payment_acct_ref_number.clone())
    }

    /// The terminal `AttemptStatus` for a declined payment-side message.
    ///
    /// `terminal` is the flow-appropriate hard-failure status; two `ResponseCode`s are
    /// explicitly non-terminal and override it.
    fn payment_decline_status(&self, terminal: AttemptStatus) -> AttemptStatus {
        if self.return_code != RETURN_CODE_SUCCESS {
            // The message was rejected before the issuer saw it: correctable and terminal
            // for this attempt.
            return terminal;
        }
        match self.response_code.as_deref() {
            Some(RESPONSE_CODE_HONOR_WITH_ID) => AttemptStatus::AuthenticationPending,
            Some(RESPONSE_CODE_REQUEST_IN_PROGRESS) => AttemptStatus::Pending,
            _ => terminal,
        }
    }

    /// The terminal `RefundStatus` for a declined refund message.
    fn refund_decline_status(&self) -> RefundStatus {
        if self.return_code == RETURN_CODE_SUCCESS
            && self.response_code.as_deref() == Some(RESPONSE_CODE_REQUEST_IN_PROGRESS)
        {
            RefundStatus::Pending
        } else {
            RefundStatus::Failure
        }
    }

    /// Build the `ErrorResponse` for a body-level decline. RAFT answers HTTP 200 for
    /// approvals, declines and validation errors alike, so this — not
    /// `build_error_response` — is where every payment failure is surfaced.
    fn to_error_response(&self, status_code: u16, attempt_status: FlowStatus) -> ErrorResponse {
        ErrorResponse {
            status_code,
            code: self.error_code(),
            message: self.error_message(),
            reason: self.error_reason(),
            attempt_status: Some(attempt_status),
            connector_transaction_id: self.api_transaction_id.clone(),
            // The issuer decision code is the closest thing RAFT publishes to a raw
            // network decline code.
            network_decline_code: self.response_code.clone(),
            network_advice_code: self.network_advice_code(),
            network_error_message: Some(self.error_message()),
            ..Default::default()
        }
    }

    /// The structured record of everything worth persisting from this response, published
    /// as `connector_metadata` for observability, reconciliation and webhook correlation.
    fn to_connector_metadata(
        &self,
        reference: &WorldpayraftTransactionReference,
    ) -> serde_json::Value {
        let trace = self.reference_trace_numbers.as_ref();
        serde_json::json!({
            "api_transaction_id": reference.api_transaction_id,
            "local_date_time": reference.local_date_time,
            "is_debit": reference.is_debit,
            "authorized_minor_amount": reference.authorized_minor_amount,
            "authorization_number": reference.authorization_number,
            "retrieval_ref_number": reference.retrieval_ref_number,
            "system_trace_number": trace.and_then(|t| t.system_trace_number.clone()),
            "network_trace_number": trace.and_then(|t| t.network_trace_number.clone()),
            "network_ref_number": trace.and_then(|t| t.network_ref_number.clone()),
            "transaction_link_id": self.transaction_link_id(),
            "payment_acct_ref_number": self.payment_account_reference(),
            "visa_transaction_id": self.visa_specific_data.as_ref()
                .and_then(|v| v.visa_transaction_id.clone()),
            "mcrd_banknet_ref_num": self.mcrd_specific_data.as_ref()
                .and_then(|m| m.mcrd_banknet_refnum.clone()),
            "mcrd_banknet_settle_date": self.mcrd_specific_data.as_ref()
                .and_then(|m| m.mcrd_banknet_settle_date.clone()),
            "amex_transaction_id": self.amex_specific_data.as_ref()
                .and_then(|a| a.amex_transaction_id.clone()),
            "disc_transaction_id": self.disc_specific_data.as_ref()
                .and_then(|d| d.disc_transaction_id.clone()),
        })
    }

    /// Rebuild the composite reference from the values that were **sent** plus the trace
    /// numbers Worldpay returned.
    fn to_transaction_reference(
        &self,
        is_debit: bool,
        api_transaction_id: String,
        local_date_time: String,
        authorized_minor_amount: Option<i64>,
    ) -> WorldpayraftTransactionReference {
        let trace = self.reference_trace_numbers.as_ref();
        WorldpayraftTransactionReference {
            is_debit,
            api_transaction_id,
            local_date_time,
            authorized_minor_amount,
            authorization_number: trace
                .and_then(|t| t.authorization_number.clone())
                .and_then(|value| non_empty(value.trim())),
            retrieval_ref_number: trace
                .and_then(|t| t.retrieval_ref_number.clone())
                .and_then(|value| non_empty(value.trim())),
        }
    }
}

// =============================================================================
// TRANSPORT-LEVEL ERROR RESPONSE
// =============================================================================

/// `{"fault": {...}}` — the shape RAFT returns for a transport / licence problem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftFault {
    #[serde(rename = "faultType")]
    pub fault_type: Option<String>,
    #[serde(rename = "faultDescription")]
    pub fault_description: Option<String>,
}

/// A RAFT error body.
///
/// Business failures are wrapped in the operation key (`{"creditauthresponse": {…}}`), so
/// nothing useful lives at the JSON root — a deserializer that reads `ReturnCode` from the
/// root fails on every RAFT response. Transport failures use a different, unwrapped shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftErrorResponse {
    Fault {
        fault: WorldpayraftFault,
    },
    /// The single-entry `{"<operation>response": {…}}` envelope, keyed by whichever
    /// operation was called.
    Wrapped(std::collections::HashMap<String, WorldpayraftResponseInner>),
}

impl WorldpayraftErrorResponse {
    /// Build an `ErrorResponse` from whichever shape came back.
    ///
    /// `attempt_status` is never left unset: a 5xx leaves the payment state genuinely
    /// unknown, so it stays non-terminal, while a 4xx (licence, routing) means the
    /// transaction never happened.
    pub fn to_error_response(&self, status_code: u16) -> ErrorResponse {
        let attempt_status = if status_code >= 500 {
            FlowStatus::Payment(AttemptStatus::Pending)
        } else {
            FlowStatus::Payment(AttemptStatus::Failure)
        };
        match self {
            Self::Wrapped(envelope) => match envelope.values().next() {
                Some(inner) => inner.to_error_response(status_code, attempt_status),
                None => ErrorResponse {
                    status_code,
                    code: consts::NO_ERROR_CODE.to_string(),
                    message: consts::NO_ERROR_MESSAGE.to_string(),
                    reason: Some("Worldpay RAFT returned an empty response envelope".to_string()),
                    attempt_status: Some(attempt_status),
                    ..Default::default()
                },
            },
            Self::Fault { fault } => ErrorResponse {
                status_code,
                code: fault
                    .fault_type
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: fault
                    .fault_description
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: fault.fault_type.clone(),
                attempt_status: Some(attempt_status),
                ..Default::default()
            },
        }
    }
}

// =============================================================================
// AUTHORIZE REQUEST
// =============================================================================

/// Inner fields shared by the four authorize-family operations
/// (`creditpurchase`, `creditauth`, `debitpurchase`, `debitpreauth`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCardAuthInner<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub misc_amounts_balances: WorldpayraftAmounts,
    pub card_info: WorldpayraftCardInfo<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_verification_data: Option<WorldpayraftCardVerificationData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_verification_data: Option<WorldpayraftAddressVerificationData>,
    pub terminal_data: WorldpayraftTerminalData,
    #[serde(rename = "E-commerceData")]
    pub ecommerce_data: WorldpayraftEcommerceData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proc_flags_indicators: Option<WorldpayraftProcFlagsIndicators>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    pub local_date_time: String,
}

/// Outer wrapper for authorize requests. The wrapper key names the endpoint:
///
/// | capture | card   | endpoint                 | wrapper        |
/// |---------|--------|--------------------------|----------------|
/// | auto    | credit | `POST /credit/purchase`  | `creditpurchase` |
/// | manual  | credit | `POST /credit/authorization` | `creditauth` |
/// | auto    | debit  | `POST /debit/purchase`   | `debitpurchase` |
/// | manual  | debit  | `POST /debit/preauth`    | `debitpreauth` |
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    CreditPurchase {
        creditpurchase: WorldpayraftCardAuthInner<T>,
    },
    CreditAuth {
        creditauth: WorldpayraftCardAuthInner<T>,
    },
    DebitPurchase {
        debitpurchase: WorldpayraftCardAuthInner<T>,
    },
    DebitPreauth {
        debitpreauth: WorldpayraftCardAuthInner<T>,
    },
}

/// Outer wrapper for authorize responses, one variant per authorize-family endpoint.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftAuthorizeResponse {
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

impl WorldpayraftAuthorizeResponse {
    /// `(inner, is_debit, is_auto_capture)` — the wrapper key identifies the endpoint that
    /// answered, and therefore both the card family and the capture mode.
    fn parts(&self) -> (&WorldpayraftResponseInner, bool, bool) {
        match self {
            Self::CreditPurchase {
                creditpurchaseresponse,
            } => (creditpurchaseresponse, false, true),
            Self::CreditAuth { creditauthresponse } => (creditauthresponse, false, false),
            Self::DebitPurchase {
                debitpurchaseresponse,
            } => (debitpurchaseresponse, true, true),
            Self::DebitPreauth {
                debitpreauthresponse,
            } => (debitpreauthresponse, true, false),
        }
    }
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftAuthorizeRequest
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
    > for WorldpayraftAuthorizeRequest<T>
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
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires the payment amount in major currency units (e.g. USD dollars)".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let card: &Card<T> = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "Only Card payment method is supported for Worldpay RAFT Authorize"
                            .to_string(),
                        errors::IntegrationErrorContext::default(),
                    )
                ))
            }
        };

        let is_debit = is_debit_card(&router_data.request.payment_method_data);
        let is_auto_capture = resolve_auto_capture(&router_data.request)?;

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

        // `Cvv2Cvc2CIDIndicator` is a presence indicator: "1" when a value accompanies it.
        // When no CVC was collected the whole `CardVerificationData` object is omitted.
        let card_verification_data = if card.card_cvc.peek().is_empty() {
            None
        } else {
            Some(WorldpayraftCardVerificationData {
                cvv_indicator: WorldpayraftCvvIndicator::Present,
                cvv2_cvc2_cid_value: card.card_cvc.clone(),
            })
        };

        // AVS is only sent for credit cards (debit does not carry AddressVerificationData).
        let address_verification_data = if is_debit {
            None
        } else {
            router_data
                .resource_common_data
                .get_optional_billing()
                .and_then(|billing| billing.address.as_ref())
                .and_then(|address| {
                    let avs_zip_code = address.zip.clone();
                    let avs_address = address.line1.clone();
                    if avs_zip_code.is_none() && avs_address.is_none() {
                        None
                    } else {
                        Some(WorldpayraftAddressVerificationData {
                            avs_zip_code,
                            avs_address,
                        })
                    }
                })
        };

        // `POSEnvironment` is request-only and credit-only. A payment that is establishing
        // a credential for later merchant-initiated use is flagged as credential-on-file.
        let is_storing_credential =
            router_data.request.setup_future_usage == Some(common_enums::FutureUsage::OffSession);
        let pos_environment = if is_debit || !is_storing_credential {
            None
        } else {
            Some(WorldpayraftPosEnvironment::CredentialOnFile)
        };

        // `MastercardAdviceCodeIndicator` is what opts this transaction into receiving
        // `McrdSpecificData.MastercardMerchantAdviceCode` on a decline. Credit only — the
        // debit spec has no advice code at all.
        let proc_flags_indicators = if is_debit && !is_storing_credential {
            None
        } else {
            Some(WorldpayraftProcFlagsIndicators {
                mastercard_advice_code_indicator: (!is_debit).then_some(WorldpayraftFlag::Yes),
                cardholder_initiated_transaction: is_storing_credential
                    .then_some(WorldpayraftFlag::Yes),
                ..Default::default()
            })
        };

        let api_transaction_id = truncate_api_transaction_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        let inner = WorldpayraftCardAuthInner {
            misc_amounts_balances: WorldpayraftAmounts {
                transaction_amount,
                preauthorized_amount: None,
            },
            card_info: WorldpayraftCardInfo {
                pan: card.card_number.clone(),
                expiration_date,
            },
            card_verification_data,
            address_verification_data,
            terminal_data: WorldpayraftTerminalData {
                entry_mode: ENTRY_MODE_ECOMM.to_string(),
                pos_condition_code: POS_CONDITION_CODE_ECOMM.to_string(),
                terminal_entry_cap: TERMINAL_ENTRY_CAP_DEFAULT.to_string(),
                pos_environment,
            },
            ecommerce_data: WorldpayraftEcommerceData {
                ecommerce_indicator: ECOMMERCE_INDICATOR_ECOMM_NO_3DS.to_string(),
            },
            proc_flags_indicators,
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id,
            local_date_time: get_local_datetime(),
        };

        Ok(match (is_debit, is_auto_capture) {
            (false, true) => Self::CreditPurchase {
                creditpurchase: inner,
            },
            (false, false) => Self::CreditAuth { creditauth: inner },
            (true, true) => Self::DebitPurchase {
                debitpurchase: inner,
            },
            (true, false) => Self::DebitPreauth {
                debitpreauth: inner,
            },
        })
    }
}

// =============================================================================
// TryFrom: WorldpayraftAuthorizeResponse → RouterDataV2
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<WorldpayraftAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (inner, is_debit, is_auto_capture) = item.response.parts();

        // Replay-critical values: what UCS *sent*, not what came back, is the matching key.
        let api_transaction_id = truncate_api_transaction_id(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        );
        let authorized_minor_amount = inner
            .is_success()
            .then(|| item.router_data.request.minor_amount.get_amount_as_i64());

        if !inner.is_success() {
            let status = inner.payment_decline_status(AttemptStatus::AuthorizationFailed);
            return Ok(Self {
                response: Err(inner.to_error_response(item.http_code, FlowStatus::Payment(status))),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let reference = inner.to_transaction_reference(
            is_debit,
            api_transaction_id,
            get_local_datetime(),
            authorized_minor_amount,
        );
        let status = if is_auto_capture {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Authorized
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(reference.encode()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(inner.to_connector_metadata(&reference)),
                network_txn_id: inner.network_transaction_id(),
                network_txn_link_id: inner.transaction_link_id(),
                connector_response_reference_id: inner.api_transaction_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: inner.payment_account_reference(),
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
// CAPTURE
// =============================================================================

/// Inner fields for `creditcompletion` / `debitcompletion`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCompletionInner {
    /// `MiscAmountsBalances` — `PreauthorizedAmount` is **required** here alongside
    /// `TransactionAmount`; a completion without it cannot be matched to its preauth.
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_trace_numbers: Option<WorldpayraftRequestTraceNumbers>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    /// The **original** authorization's `APITransactionID`, replayed verbatim.
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    /// The `LocalDateTime` recorded for the original authorization, replayed here.
    pub local_date_time: String,
}

/// Outer wrapper for capture requests.
///
/// Credit: `{ "creditcompletion": { … } }` → `POST /credit/completion`
/// Debit:  `{ "debitcompletion": { … } }` → `POST /debit/completion`
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
    fn inner(&self) -> &WorldpayraftResponseInner {
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
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let reference = WorldpayraftTransactionReference::parse(
            &router_data.request.get_connector_transaction_id()?,
        )?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires the capture amount in major currency units"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        // `PreauthorizedAmount` is what the original authorization was approved for; it is
        // carried on the composite connector_transaction_id because `PaymentsCaptureData`
        // has no channel for connector metadata.
        let authorized_minor_amount = reference.authorized_minor_amount.ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "PreauthorizedAmount",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "The originally authorized amount is required on a Worldpay RAFT \
                         completion and is not present in the connector transaction reference"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?;
        let preauthorized_amount = item
            .connector
            .amount_converter
            .convert(
                MinorUnit::new(authorized_minor_amount),
                router_data.request.currency,
            )
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires PreauthorizedAmount in major currency units"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let inner = WorldpayraftCompletionInner {
            misc_amounts_balances: WorldpayraftAmounts {
                transaction_amount,
                preauthorized_amount: Some(preauthorized_amount),
            },
            reference_trace_numbers: reference.follow_up_trace_numbers(),
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id: reference.api_transaction_id.clone(),
            local_date_time: reference.local_date_time.clone(),
        };

        Ok(if reference.is_debit {
            Self::Debit {
                debitcompletion: inner,
            }
        } else {
            Self::Credit {
                creditcompletion: inner,
            }
        })
    }
}

impl TryFrom<ResponseRouterData<WorldpayraftCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let inner = item.response.inner();

        if !inner.is_success() {
            let status = inner.payment_decline_status(AttemptStatus::CaptureFailed);
            return Ok(Self {
                response: Err(inner.to_error_response(item.http_code, FlowStatus::Payment(status))),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Preserve the composite reference from the authorize leg so a later refund still
        // has the original APITransactionID, LocalDateTime and trace numbers.
        let connector_transaction_id = item
            .router_data
            .request
            .get_connector_transaction_id()
            .change_context(errors::ConnectorError::ResponseHandlingFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "Worldpay RAFT completion succeeded but the originating connector \
                         transaction reference is no longer available"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: inner.network_transaction_id(),
                network_txn_link_id: inner.transaction_link_id(),
                connector_response_reference_id: inner.api_transaction_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: inner.payment_account_reference(),
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
// REFUND
// =============================================================================

/// Inner fields for `creditrefund` / `debitrefund`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftRefundInner {
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_trace_numbers: Option<WorldpayraftRequestTraceNumbers>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    /// The **original** payment's `APITransactionID`, replayed so Worldpay can match the
    /// refund back to it.
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    /// The `LocalDateTime` recorded for the original payment, replayed for the same reason.
    pub local_date_time: String,
}

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
    fn inner(&self) -> &WorldpayraftResponseInner {
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
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let reference =
            WorldpayraftTransactionReference::parse(&router_data.request.connector_transaction_id)?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires the refund amount in major currency units"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let inner = WorldpayraftRefundInner {
            misc_amounts_balances: WorldpayraftAmounts {
                transaction_amount,
                preauthorized_amount: None,
            },
            reference_trace_numbers: reference.follow_up_trace_numbers(),
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id: reference.api_transaction_id.clone(),
            local_date_time: reference.local_date_time.clone(),
        };

        Ok(if reference.is_debit {
            Self::Debit { debitrefund: inner }
        } else {
            Self::Credit {
                creditrefund: inner,
            }
        })
    }
}

impl TryFrom<ResponseRouterData<WorldpayraftRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let inner = item.response.inner();

        if !inner.is_success() {
            let refund_status = inner.refund_decline_status();
            return Ok(Self {
                response: Err(
                    inner.to_error_response(item.http_code, FlowStatus::Refund(refund_status))
                ),
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // The refund is identified by the same composite reference shape as the payment, so
        // the trace numbers minted for the refund leg travel with it. `AuthorizationNumber`
        // is deliberately not used as the id: it is a 6-char issuer approval code and is
        // not unique across transactions.
        let reference = WorldpayraftTransactionReference::parse(
            &item.router_data.request.connector_transaction_id,
        )
        .change_context(errors::ConnectorError::ResponseHandlingFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(item.http_code),
                additional_context: Some(
                    "Worldpay RAFT refund could not re-encode its connector transaction reference"
                        .to_string(),
                ),
                ..Default::default()
            },
        })?;
        let refund_reference = inner.to_transaction_reference(
            reference.is_debit,
            reference.api_transaction_id.clone(),
            reference.local_date_time.clone(),
            reference.authorized_minor_amount,
        );

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: refund_reference.encode(),
                refund_status: RefundStatus::Success,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: RefundStatus::Success,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// SETUP MANDATE (card tokenization)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftSetupMandateInner {
    pub card_info: WorldpayraftPlainCardInfo,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    pub local_date_time: String,
}

/// Wrapper for the `tokenize` request body → `POST /tokenization/token`.
#[derive(Debug, Serialize)]
pub struct WorldpayraftSetupMandateRequest {
    pub tokenize: WorldpayraftSetupMandateInner,
}

/// Wrapper for the `tokenizeresponse` body.
#[derive(Debug, Deserialize, Serialize)]
pub struct WorldpayraftSetupMandateResponse {
    pub tokenizeresponse: WorldpayraftResponseInner,
}

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
    > for WorldpayraftSetupMandateRequest
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
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let card: &Card<T> = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "Only Card payment method is supported for Worldpay RAFT SetupMandate"
                            .to_string(),
                        errors::IntegrationErrorContext::default(),
                    )
                ))
            }
        };

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

        Ok(Self {
            tokenize: WorldpayraftSetupMandateInner {
                card_info: WorldpayraftPlainCardInfo {
                    pan: Some(Secret::new(card.card_number.peek().to_string())),
                    expiration_date: Some(expiration_date),
                },
                world_pay_merchant_id: auth.merchant_id,
                api_transaction_id: truncate_api_transaction_id(
                    &router_data
                        .resource_common_data
                        .connector_request_reference_id,
                ),
                local_date_time: get_local_datetime(),
            },
        })
    }
}

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
        let inner = &item.response.tokenizeresponse;

        if !inner.is_success() {
            let status = inner.payment_decline_status(AttemptStatus::Failure);
            return Ok(Self {
                response: Err(inner.to_error_response(item.http_code, FlowStatus::Payment(status))),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let tokenized_pan = inner
            .encryption_token_data
            .as_ref()
            .and_then(|token_data| token_data.tokenized_pan.clone())
            .ok_or_else(|| {
                error_stack::report!(errors::ConnectorError::ResponseHandlingFailed {
                    context: errors::ResponseTransformationErrorContext {
                        http_status_code: Some(item.http_code),
                        additional_context: Some(
                            "Worldpay RAFT tokenizeresponse approved but carried no \
                             EncryptionTokenData.TokenizedPAN to store as the mandate reference"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })
            })?;

        // Tokenization is always a credit-path operation and carries no amount, so the
        // composite reference has no PreauthorizedAmount to record.
        let reference = inner.to_transaction_reference(
            false,
            truncate_api_transaction_id(
                &item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
            get_local_datetime(),
            None,
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(reference.encode()),
                redirection_data: None,
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id: Some(tokenized_pan),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })),
                connector_metadata: Some(inner.to_connector_metadata(&reference)),
                network_txn_id: inner.network_transaction_id(),
                network_txn_link_id: inner.transaction_link_id(),
                connector_response_reference_id: inner.api_transaction_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: inner.payment_account_reference(),
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
// REPEAT PAYMENT (merchant-initiated transaction)
// =============================================================================

/// Inner fields for a merchant-initiated `creditauth`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftRepeatCreditAuth {
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_info: Option<WorldpayraftPlainCardInfo>,
    /// A stored Worldpay token belongs in `EncryptionTokenData.TokenizedPAN`, not in
    /// `CardInfo.PAN`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenRequestData>,
    pub terminal_data: WorldpayraftTerminalData,
    #[serde(rename = "E-commerceData")]
    pub ecommerce_data: WorldpayraftEcommerceData,
    pub proc_flags_indicators: WorldpayraftProcFlagsIndicators,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_trace_numbers: Option<WorldpayraftRequestTraceNumbers>,
    /// The per-brand `*SpecificData` blocks carrying the network transaction id.
    #[serde(flatten)]
    pub brand_data: WorldpayraftBrandRequestData,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    pub local_date_time: String,
}

/// Outer wrapper for the merchant-initiated request. Capture mode picks the endpoint
/// exactly as it does on Authorize: `POST /credit/purchase` for an auto-captured MIT,
/// `POST /credit/authorization` for one that will be completed separately.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftRepeatPaymentRequest {
    CreditPurchase {
        creditpurchase: WorldpayraftRepeatCreditAuth,
    },
    CreditAuth {
        creditauth: WorldpayraftRepeatCreditAuth,
    },
}

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
    /// `(inner, is_auto_capture)` — the wrapper key identifies which endpoint answered.
    fn parts(&self) -> (&WorldpayraftResponseInner, bool) {
        match self {
            Self::CreditPurchase {
                creditpurchaseresponse,
            } => (creditpurchaseresponse, true),
            Self::CreditAuth { creditauthresponse } => (creditauthresponse, false),
        }
    }
}

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
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires the repeat payment amount in major currency units"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => Some(card),
            _ => None,
        };

        // Worldpay explicitly recommends a non-empty `CardInfo.ExpirationDate` on every
        // token-initiated transaction, and Discover requires it.
        let expiration_date = match card {
            Some(card) => Some(card.get_expiry_date_as_yymm().change_context(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "card.card_exp_year / card.card_exp_month",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Worldpay RAFT expects card expiry in YYMM format".to_string(),
                        ),
                        ..Default::default()
                    },
                },
            )?),
            None => None,
        };

        let mit_category = router_data.request.mit_category.clone();
        let subsequent_reason_code = mit_category.clone().and_then(|category| {
            WorldpayraftSubsequentTransactionReasonCode::try_from(category).ok()
        });

        // Each mandate flavour selects a different credential carrier:
        // - ConnectorMandateId  → a Worldpay token in `EncryptionTokenData.TokenizedPAN`
        // - NetworkMandateId    → raw card in `CardInfo.PAN` + the scheme NTID echo
        // - NetworkTokenWithNTI → network token in `CardInfo.PAN` + the scheme NTID echo
        let (card_info, encryption_token_data, network_transaction_id, transaction_link_id) =
            match &router_data.request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
                    let tokenized_pan = connector_mandate_ref
                        .get_connector_mandate_id()
                        .ok_or_else(|| {
                            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                                field_name: "connector_mandate_id",
                                context: errors::IntegrationErrorContext {
                                    additional_context: Some(
                                        "Worldpay RAFT RepeatPayment requires the stored \
                                         TokenizedPAN as the connector mandate id"
                                            .to_string(),
                                    ),
                                    ..Default::default()
                                },
                            })
                        })?;
                    (
                        expiration_date
                            .clone()
                            .map(|expiry| WorldpayraftPlainCardInfo {
                                pan: None,
                                expiration_date: Some(expiry),
                            }),
                        Some(WorldpayraftEncryptionTokenRequestData {
                            tokenized_pan: Secret::new(tokenized_pan),
                        }),
                        None,
                        None,
                    )
                }
                MandateReferenceId::NetworkMandateId(network_mandate) => {
                    let card = card.ok_or_else(|| {
                        error_stack::report!(errors::IntegrationError::MissingRequiredField {
                            field_name: "payment_method_data.card",
                            context: errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "A Worldpay RAFT network-transaction-id MIT is a raw-card \
                                     message: the card must accompany the NTID"
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })
                    })?;
                    (
                        Some(WorldpayraftPlainCardInfo {
                            pan: Some(Secret::new(card.card_number.peek().to_string())),
                            expiration_date: expiration_date.clone(),
                        }),
                        None,
                        Some(network_mandate.network_transaction_id.clone()),
                        network_mandate.transaction_link_id.clone(),
                    )
                }
                MandateReferenceId::NetworkTokenWithNTI(network_token) => {
                    let card = card.ok_or_else(|| {
                        error_stack::report!(errors::IntegrationError::MissingRequiredField {
                            field_name: "payment_method_data.card",
                            context: errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "A Worldpay RAFT network-token MIT carries the network token \
                                     in CardInfo.PAN; no card data was supplied"
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })
                    })?;
                    (
                        Some(WorldpayraftPlainCardInfo {
                            pan: Some(Secret::new(card.card_number.peek().to_string())),
                            expiration_date: expiration_date.clone(),
                        }),
                        None,
                        Some(network_token.network_transaction_id.clone()),
                        network_token.transaction_link_id.clone(),
                    )
                }
            };

        // Echo the network transaction id in the block belonging to the card's own brand.
        // RAFT has no generic NetworkTransactionId field.
        let brand_data = match (&network_transaction_id, card) {
            (None, _) => WorldpayraftBrandRequestData::default(),
            (Some(ntid), card) => {
                let brand = card
                    .and_then(WorldpayraftCardBrand::resolve)
                    .ok_or_else(|| {
                        error_stack::report!(errors::IntegrationError::MissingRequiredField {
                        field_name: "payment_method_data.card.card_network",
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Worldpay RAFT carries the network transaction id in a per-brand \
                                 object (VisaSpecificData / McrdSpecificData / AmexSpecificData / \
                                 DiscSpecificData); the card brand could not be determined"
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })
                    })?;
                match brand {
                    WorldpayraftCardBrand::Visa => WorldpayraftBrandRequestData {
                        visa_specific_data: Some(WorldpayraftVisaRequestData {
                            visa_transaction_id: ntid.clone(),
                            visa_subsequent_transaction_reason_code: subsequent_reason_code,
                        }),
                        ..Default::default()
                    },
                    WorldpayraftCardBrand::Mastercard => {
                        // Mastercard needs BOTH Banknet fields; the reference number on its
                        // own is not sufficient. The settlement date travels with the NTID
                        // as `<McrdBanknetREFNUM>:<McrdBanknetSettleDate>`.
                        let (banknet_ref, settle_date) =
                            ntid.split_once(MCRD_NTID_SEPARATOR).ok_or_else(|| {
                                error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                                    field_name: "mandate_reference.network_transaction_id",
                                    context: errors::IntegrationErrorContext {
                                        additional_context: Some(format!(
                                            "A Mastercard MIT needs both McrdBanknetREFNUM \
                                                 and McrdBanknetSettleDate (MMDD); Worldpay RAFT \
                                                 stores them as '<REFNUM>:<MMDD>', got {ntid:?}"
                                        )),
                                        ..Default::default()
                                    },
                                })
                            })?;
                        WorldpayraftBrandRequestData {
                            mcrd_specific_data: Some(WorldpayraftMcrdRequestData {
                                mcrd_banknet_refnum: banknet_ref.to_string(),
                                mcrd_banknet_settle_date: settle_date.to_string(),
                                mcrd_subsequent_transaction_reason_code: subsequent_reason_code,
                            }),
                            ..Default::default()
                        }
                    }
                    WorldpayraftCardBrand::Amex => WorldpayraftBrandRequestData {
                        amex_specific_data: Some(WorldpayraftAmexRequestData {
                            amex_transaction_id: ntid.clone(),
                            amex_subsequent_transaction_reason_code: subsequent_reason_code,
                        }),
                        ..Default::default()
                    },
                    WorldpayraftCardBrand::Discover => WorldpayraftBrandRequestData {
                        disc_specific_data: Some(WorldpayraftDiscRequestData {
                            disc_transaction_id: ntid.clone(),
                            disc_subsequent_transaction_reason_code: subsequent_reason_code,
                        }),
                        ..Default::default()
                    },
                }
            }
        };

        // The Mastercard/Maestro lifecycle id received on the original CIT is resent on the
        // MIT as `ReferenceTraceNumbers.EconomicallyRelatedLinkID`.
        let reference_trace_numbers =
            transaction_link_id.map(|link_id| WorldpayraftRequestTraceNumbers {
                retrieval_ref_number: None,
                authorization_number: None,
                economically_related_link_id: Some(link_id),
            });

        // Every MIT must carry a stored-credential indicator; an unscheduled card-on-file
        // MIT is the shape UCS models when the caller does not classify the schedule.
        let pos_environment = mit_category
            .clone()
            .map(WorldpayraftPosEnvironment::from)
            .unwrap_or(WorldpayraftPosEnvironment::UnscheduledCardOnFile);

        let inner = WorldpayraftRepeatCreditAuth {
            misc_amounts_balances: WorldpayraftAmounts {
                transaction_amount,
                preauthorized_amount: None,
            },
            card_info,
            encryption_token_data,
            terminal_data: WorldpayraftTerminalData {
                entry_mode: ENTRY_MODE_ECOMM.to_string(),
                pos_condition_code: POS_CONDITION_CODE_ECOMM.to_string(),
                terminal_entry_cap: TERMINAL_ENTRY_CAP_DEFAULT.to_string(),
                pos_environment: Some(pos_environment),
            },
            ecommerce_data: WorldpayraftEcommerceData {
                ecommerce_indicator: ECOMMERCE_INDICATOR_ECOMM_NO_3DS.to_string(),
            },
            proc_flags_indicators: WorldpayraftProcFlagsIndicators {
                mastercard_advice_code_indicator: Some(WorldpayraftFlag::Yes),
                cardholder_initiated_transaction: None,
                merchant_initiated_transaction: Some(WorldpayraftFlag::Yes),
                recurring_bill_pay: matches!(
                    mit_category,
                    Some(common_enums::MitCategory::Recurring)
                )
                .then_some(WorldpayraftFlag::Yes),
            },
            reference_trace_numbers,
            brand_data,
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id: truncate_api_transaction_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
            local_date_time: get_local_datetime(),
        };

        Ok(if resolve_repeat_auto_capture(&router_data.request)? {
            Self::CreditPurchase {
                creditpurchase: inner,
            }
        } else {
            Self::CreditAuth { creditauth: inner }
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<WorldpayraftRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<WorldpayraftRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (inner, is_auto_capture) = item.response.parts();

        if !inner.is_success() {
            let status = inner.payment_decline_status(AttemptStatus::Failure);
            return Ok(Self {
                response: Err(inner.to_error_response(item.http_code, FlowStatus::Payment(status))),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let reference = inner.to_transaction_reference(
            false,
            truncate_api_transaction_id(
                &item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
            get_local_datetime(),
            Some(item.router_data.request.minor_amount.get_amount_as_i64()),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(reference.encode()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(inner.to_connector_metadata(&reference)),
                network_txn_id: inner.network_transaction_id(),
                network_txn_link_id: inner.transaction_link_id(),
                connector_response_reference_id: inner.api_transaction_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: inner.payment_account_reference(),
            }),
            resource_common_data: PaymentFlowData {
                status: if is_auto_capture {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                },
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
