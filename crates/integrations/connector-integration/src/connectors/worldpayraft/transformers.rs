use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, CardNetwork, FutureUsage, MitCategory, RefundStatus};
use common_utils::{
    ext_traits::Encode,
    pii::SecretSerdeValue,
    types::{Money, StringMajorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, RefundFlowData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors,
    payment_method_data::{
        Card, CardDetailsForNetworkTransactionId, CardWithNoCvc, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils::{get_card_issuer, CardIssuer},
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::worldpayraft::WorldpayraftRouterData;

// =============================================================================
// CONSTANTS
// =============================================================================

/// Card type identifier as received in PaymentMethodData (case-insensitive comparison via eq_ignore_ascii_case).
pub(super) const CARD_TYPE_DEBIT: &str = "debit";

/// Prefix embedded in connector_transaction_id to identify debit transactions.
pub(super) const TXN_TYPE_DEBIT: &str = "D";
/// Prefix embedded in connector_transaction_id to identify credit transactions.
pub(super) const TXN_TYPE_CREDIT: &str = "C";

const RETURN_CODE_SUCCESS: &str = "0000";
const RESPONSE_CODE_SUCCESS: &str = "000";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorldpayraftEntryMode {
    #[serde(rename = "E-COMM")]
    Ecommerce,
    #[serde(rename = "KEYED")]
    Keyed,
    #[serde(rename = "CREDONFL")]
    CredentialOnFile,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorldpayraftPosConditionCode {
    #[serde(rename = "59")]
    Ecommerce,
    #[serde(rename = "08")]
    MailTelephoneOrder,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorldpayraftTerminalEntryCapability {
    #[serde(rename = "0")]
    Unspecified,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorldpayraftEcommerceIndicator {
    #[serde(rename = "07")]
    NonThreeDs,
    #[serde(rename = "02")]
    Recurring,
    #[serde(rename = "01")]
    Unscheduled,
    #[serde(rename = "03")]
    Installment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorldpayraftCvvIndicator {
    #[serde(rename = "1")]
    Provided,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorldpayraftFlag {
    #[serde(rename = "Y")]
    Yes,
    #[serde(rename = "N")]
    No,
}

#[derive(Debug, Clone, Copy, strum::Display)]
pub enum WorldpayraftSubsequentTransactionReasonCode {
    #[strum(serialize = "41")]
    Resubmission,
}

// =============================================================================
// SHARED HELPERS
// =============================================================================

fn resolve_card_network(network: Option<CardNetwork>, pan: &str) -> Option<CardNetwork> {
    network.or_else(|| match get_card_issuer(pan).ok()? {
        CardIssuer::Visa => Some(CardNetwork::Visa),
        CardIssuer::Master => Some(CardNetwork::Mastercard),
        CardIssuer::Discover => Some(CardNetwork::Discover),
        CardIssuer::AmericanExpress => Some(CardNetwork::AmericanExpress),
        _ => None,
    })
}

fn is_debit_card_type(card_type: Option<&str>) -> bool {
    card_type.is_some_and(|card_type| card_type.eq_ignore_ascii_case(CARD_TYPE_DEBIT))
}

/// Returns true when the payment method data indicates a debit card.
/// Includes raw cards, no-CVC cards, and cards carrying a network transaction reference.
pub fn is_debit_card<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> bool {
    match payment_method_data {
        PaymentMethodData::Card(c) => c
            .card_type
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case(CARD_TYPE_DEBIT))
            .unwrap_or(false),
        PaymentMethodData::CardWithNoCvc(c) => c
            .card_type
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case(CARD_TYPE_DEBIT))
            .unwrap_or(false),
        PaymentMethodData::CardDetailsForNetworkTransactionId(card) => {
            is_debit_card_type(card.card_type.as_deref())
        }
        _ => false,
    }
}

// =============================================================================
// VOID ENUMS
// =============================================================================

/// `AuthorizationType` field values. Only "RV" (Reversal) is used for voids.
/// "FP" (Force Post) is used on completion/capture calls.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WorldpayraftAuthorizationType {
    #[serde(rename = "RV")]
    Reversal,
    #[serde(rename = "FP")]
    ForcePost,
}

/// `ReversalAdviceReasonCd` values per the RAFT spec.
/// Use `NormalReversal` for merchant-initiated voids; `CustomerCancel` when
/// the cancellation_reason field is populated.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WorldpayraftReversalAdviceReasonCode {
    /// 000 — Normal Reversal (merchant void)
    #[serde(rename = "000")]
    NormalReversal,
    /// 006 — Customer Cancel
    #[serde(rename = "006")]
    CustomerCancel,
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
                            "Worldpay RAFT requires ConnectorSpecificConfig::Worldpayraft with license and merchant_id to authenticate requests".to_string(),
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

/// Worldpay RAFT error response body.
///
/// The API always returns HTTP 200. To detect errors, check:
/// - `ReturnCode` != "0000"  → operation-level failure
/// - `ResponseCode` != "000" → soft decline / issuer decline
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftErrorResponse {
    pub return_code: Option<String>,
    pub reason_code: Option<String>,
    pub response_code: Option<String>,
}

// =============================================================================
// SHARED HELPERS
// =============================================================================

/// Returns the current UTC datetime formatted as "YYYY-MM-DDTHH:MM:SS".
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

/// Truncates a string to at most 16 characters (APITransactionID max length).
fn truncate_api_transaction_id(id: &str) -> String {
    id.chars().take(16).collect()
}

/// Parse a connector_transaction_id that encodes card type + auth trace numbers.
///
/// New format: `"{prefix}|{AuthorizationNumber}|{RetrievalREFNumber}|{SystemTraceNumber}"`
/// where prefix is `TXN_TYPE_CREDIT` ("C") or `TXN_TYPE_DEBIT` ("D").
///
/// Legacy format (backwards compat, treated as credit):
/// `"{AuthorizationNumber}|{RetrievalREFNumber}|{SystemTraceNumber}"`
///
/// Returns `(is_debit, authorization_number, retrieval_ref_number, system_trace_number)`.
pub(super) fn parse_connector_transaction_id(id: &str) -> (bool, &str, &str, &str) {
    let mut iter = id.splitn(4, '|');
    match (iter.next(), iter.next(), iter.next(), iter.next()) {
        (Some(prefix), Some(auth_num), Some(retrieval_ref), Some(sys_trace)) => {
            (prefix == TXN_TYPE_DEBIT, auth_num, retrieval_ref, sys_trace)
        }
        (Some(auth_num), Some(retrieval_ref), Some(sys_trace), None) => {
            // Legacy format without prefix — treat as credit
            (false, auth_num, retrieval_ref, sys_trace)
        }
        _ => (false, id, "", ""),
    }
}

fn map_payment_status(return_code: &str, response_code: &str) -> bool {
    return_code == "0000" && response_code == "000"
}

fn payment_status(return_code: &str, response_code: &str, auto_capture: bool) -> AttemptStatus {
    match (return_code, response_code, auto_capture) {
        ("0000", "000", true) => AttemptStatus::Charged,
        ("0000", "000", false) => AttemptStatus::Authorized,
        _ => AttemptStatus::Failure,
    }
}

// =============================================================================
// SHARED STRUCTS
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftAmounts {
    pub transaction_amount: StringMajorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCompletionAmounts {
    pub transaction_amount: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preauthorized_amount: Option<StringMajorUnit>,
}

/// Non-EMD values. Subscription is explicitly signalled by MitCategory::Subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldpayraftPosEnvironment {
    #[serde(rename = "C")]
    CredentialOnFile,
    #[serde(rename = "R")]
    Recurring,
    #[serde(rename = "S")]
    Subscription,
    #[serde(rename = "I")]
    Installment,
    #[serde(rename = "F")]
    FinalAuthorization,
    #[serde(rename = "P")]
    Preauthorization,
}

/// Initiation and credential usage are independent of the recurring arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorldpayraftPaymentKind {
    OneOff,
    InitialStoredCredential,
    StoredCredentialCit,
    Mit,
}

impl WorldpayraftPaymentKind {
    fn for_authorize<T: PaymentMethodDataTypes>(
        request: &PaymentsAuthorizeData<T>,
        was_stored: bool,
    ) -> Self {
        let has_mandate = request
            .mandate_id
            .as_ref()
            .and_then(|id| id.mandate_reference_id.as_ref())
            .is_some();
        if request.off_session.unwrap_or(has_mandate) {
            Self::Mit
        } else if was_stored || has_mandate {
            Self::StoredCredentialCit
        } else if request.mit_category.is_some()
            || (request.setup_future_usage == Some(FutureUsage::OffSession)
                && request.customer_acceptance.is_some())
        {
            Self::InitialStoredCredential
        } else {
            Self::OneOff
        }
    }
}

impl WorldpayraftPosEnvironment {
    fn for_payment(
        network: Option<&CardNetwork>,
        category: Option<&MitCategory>,
        kind: WorldpayraftPaymentKind,
        auto_capture: bool,
    ) -> Option<Self> {
        match category {
            Some(MitCategory::Recurring | MitCategory::Subscription)
                if network == Some(&CardNetwork::AmericanExpress) =>
            {
                Some(Self::CredentialOnFile)
            }
            Some(MitCategory::Recurring) => Some(Self::Recurring),
            Some(MitCategory::Subscription) => Some(Self::Subscription),
            Some(MitCategory::Installment) if network == Some(&CardNetwork::Visa) => {
                Some(Self::Installment)
            }
            Some(
                MitCategory::Installment | MitCategory::Unscheduled | MitCategory::Resubmission,
            ) => Some(Self::CredentialOnFile),
            None if kind != WorldpayraftPaymentKind::OneOff => Some(Self::CredentialOnFile),
            None if network == Some(&CardNetwork::Mastercard) => Some(if auto_capture {
                Self::FinalAuthorization
            } else {
                Self::Preauthorization
            }),
            None => None,
        }
    }
}

/// Only the POS fields shared by Authorize, RepeatPayment and completion.
struct WorldpayraftPosData {
    terminal_data: WorldpayraftTerminalData,
    ecommerce_data: WorldpayraftEcommerceData,
    proc_flags: Option<WorldpayraftProcFlags>,
}

impl WorldpayraftPosData {
    fn for_credit(
        network: Option<&CardNetwork>,
        category: Option<&MitCategory>,
        kind: WorldpayraftPaymentKind,
        auto_capture: bool,
        partial_allowed: Option<bool>,
    ) -> Self {
        let is_mit = kind == WorldpayraftPaymentKind::Mit;
        let is_stored = kind != WorldpayraftPaymentKind::OneOff;
        let is_recurring = matches!(
            category,
            Some(MitCategory::Recurring | MitCategory::Subscription)
        );
        let is_installment = category == Some(&MitCategory::Installment);
        Self {
            terminal_data: WorldpayraftTerminalData {
                entry_mode: if matches!(
                    kind,
                    WorldpayraftPaymentKind::Mit | WorldpayraftPaymentKind::StoredCredentialCit
                ) {
                    WorldpayraftEntryMode::CredentialOnFile
                } else {
                    WorldpayraftEntryMode::Keyed
                },
                pos_condition_code: if is_mit && !is_installment {
                    WorldpayraftPosConditionCode::MailTelephoneOrder
                } else {
                    WorldpayraftPosConditionCode::Ecommerce
                },
                terminal_entry_cap: WorldpayraftTerminalEntryCapability::Unspecified,
                pos_environment: WorldpayraftPosEnvironment::for_payment(
                    network,
                    category,
                    kind,
                    auto_capture,
                ),
            },
            ecommerce_data: WorldpayraftEcommerceData {
                ecommerce_indicator: if !is_mit {
                    WorldpayraftEcommerceIndicator::NonThreeDs
                } else if is_recurring {
                    WorldpayraftEcommerceIndicator::Recurring
                } else if is_installment {
                    WorldpayraftEcommerceIndicator::Installment
                } else {
                    WorldpayraftEcommerceIndicator::Unscheduled
                },
            },
            proc_flags: (is_stored || partial_allowed == Some(true)).then_some(
                WorldpayraftProcFlags {
                    cardholder_initiated_transaction: (is_stored && !is_mit)
                        .then_some(WorldpayraftFlag::Yes),
                    merchant_initiated_transaction: is_mit.then_some(WorldpayraftFlag::Yes),
                    recurring_bill_pay: (is_mit && is_recurring).then_some(WorldpayraftFlag::Yes),
                    partial_allowed: (partial_allowed == Some(true))
                        .then_some(WorldpayraftFlag::Yes),
                    prior_auth: None,
                },
            ),
        }
    }

    fn for_debit(auto_capture: bool) -> Self {
        Self {
            terminal_data: WorldpayraftTerminalData {
                entry_mode: if auto_capture {
                    WorldpayraftEntryMode::Keyed
                } else {
                    WorldpayraftEntryMode::Ecommerce
                },
                pos_condition_code: WorldpayraftPosConditionCode::Ecommerce,
                terminal_entry_cap: WorldpayraftTerminalEntryCapability::Unspecified,
                pos_environment: None,
            },
            ecommerce_data: WorldpayraftEcommerceData {
                ecommerce_indicator: WorldpayraftEcommerceIndicator::NonThreeDs,
            },
            proc_flags: None,
        }
    }
}

fn validate_stored_card_network(
    network: Option<&CardNetwork>,
    category: Option<&MitCategory>,
    network_transaction_id: Option<&str>,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    // An old Worldpay token can still be charged as unscheduled COF without
    // metadata. A category or network reference needs an explicit brand.
    if network.is_none() && (category.is_some() || network_transaction_id.is_some()) {
        return Err(errors::IntegrationError::MissingRequiredField {
            field_name: "payment_method_data.card.card_network",
            context: errors::IntegrationErrorContext {
                additional_context: Some("Card network is required for the RAFT payment category or network transaction reference".to_string()),
                ..Default::default()
            },
        }.into());
    }
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftProcFlags {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_initiated_transaction: Option<WorldpayraftFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_initiated_transaction: Option<WorldpayraftFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_bill_pay: Option<WorldpayraftFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_allowed: Option<WorldpayraftFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prior_auth: Option<WorldpayraftFlag>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldpayraftVisaSpecificData {
    #[serde(rename = "VisaTransactionId", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(
        rename = "VisaSubsequentTransactionReasonCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub subsequent_transaction_reason_code: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldpayraftMastercardSpecificData {
    #[serde(rename = "McrdBanknetREFNUM", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(
        rename = "McrdSubsequentTransactionReasonCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub subsequent_transaction_reason_code: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldpayraftDiscoverSpecificData {
    #[serde(rename = "DiscTransactionId", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(
        rename = "DiscSubsequentTransactionReasonCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub subsequent_transaction_reason_code: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldpayraftAmexSpecificData {
    #[serde(rename = "AmexTransactionId", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(
        rename = "AmexSubsequentTransactionReasonCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub subsequent_transaction_reason_code: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldpayraftBrandSpecificData {
    #[serde(rename = "VisaSpecificData", skip_serializing_if = "Option::is_none")]
    pub visa: Option<WorldpayraftVisaSpecificData>,
    #[serde(rename = "McrdSpecificData", skip_serializing_if = "Option::is_none")]
    pub mastercard: Option<WorldpayraftMastercardSpecificData>,
    #[serde(rename = "DiscSpecificData", skip_serializing_if = "Option::is_none")]
    pub discover: Option<WorldpayraftDiscoverSpecificData>,
    #[serde(rename = "AmexSpecificData", skip_serializing_if = "Option::is_none")]
    pub amex: Option<WorldpayraftAmexSpecificData>,
}

impl WorldpayraftBrandSpecificData {
    fn for_mit(
        network: Option<&CardNetwork>,
        transaction_id: Option<String>,
        category: Option<&MitCategory>,
    ) -> Self {
        let reason = (category == Some(&MitCategory::Resubmission))
            .then(|| WorldpayraftSubsequentTransactionReasonCode::Resubmission.to_string());
        match (network, transaction_id, reason) {
            // Omit every brand block when there is no ID or reason to send.
            (_, None, None) => Self::default(),
            (Some(CardNetwork::Visa), transaction_id, reason) => Self {
                visa: Some(WorldpayraftVisaSpecificData {
                    transaction_id,
                    subsequent_transaction_reason_code: reason,
                }),
                ..Self::default()
            },
            (Some(CardNetwork::Mastercard), transaction_id, reason) => Self {
                mastercard: Some(WorldpayraftMastercardSpecificData {
                    transaction_id,
                    subsequent_transaction_reason_code: reason,
                }),
                ..Self::default()
            },
            (Some(CardNetwork::Discover), transaction_id, reason) => Self {
                discover: Some(WorldpayraftDiscoverSpecificData {
                    transaction_id,
                    subsequent_transaction_reason_code: reason,
                }),
                ..Self::default()
            },
            (Some(CardNetwork::AmericanExpress), transaction_id, reason) => Self {
                amex: Some(WorldpayraftAmexSpecificData {
                    transaction_id,
                    subsequent_transaction_reason_code: reason,
                }),
                ..Self::default()
            },
            _ => Self::default(),
        }
    }
}

impl WorldpayraftBrandSpecificData {
    fn network_transaction_id(&self) -> Option<String> {
        self.visa
            .as_ref()
            .and_then(|data| data.transaction_id.clone())
            .or_else(|| {
                self.mastercard
                    .as_ref()
                    .and_then(|data| data.transaction_id.clone())
            })
            .or_else(|| {
                self.discover
                    .as_ref()
                    .and_then(|data| data.transaction_id.clone())
            })
            .or_else(|| {
                self.amex
                    .as_ref()
                    .and_then(|data| data.transaction_id.clone())
            })
    }
}

/// Returned as connector_feature_data; replay it on capture and connector-token MITs.
/// PAN, CVC, and card expiry are excluded from this persisted data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldpayraftConnectorMetadata {
    pub is_stored_credential: bool,
    pub card_network: Option<CardNetwork>,
    pub original_authorized_amount: Option<Money>,
    pub terminal_data: Option<WorldpayraftTerminalData>,
    pub ecommerce_data: Option<WorldpayraftEcommerceData>,
    pub proc_flags_indicators: Option<WorldpayraftProcFlags>,
    pub api_transaction_id: Option<String>,
}

impl TryFrom<Option<&SecretSerdeValue>> for WorldpayraftConnectorMetadata {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(data: Option<&SecretSerdeValue>) -> Result<Self, Self::Error> {
        data.map(|data| {
            Self::deserialize(data.peek()).change_context(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "connector_feature_data",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Expected Worldpay RAFT connector_feature_data containing stored-credential flags and authorization details for subsequent payments and captures".to_string(),
                        ),
                        ..Default::default()
                    },
                },
            )
        })
        .transpose()
        .map(|metadata| metadata.unwrap_or_default())
    }
}

/// Stored only inside the secret-wrapped mandate_metadata, never connector_metadata.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct WorldpayraftMandateMetadata {
    #[serde(flatten)]
    connector_metadata: WorldpayraftConnectorMetadata,
    // Connector-token MITs need this alongside TokenizedPAN. NetworkMandateId carries it directly.
    network_transaction_id: Option<String>,
    expiration_date: Option<Secret<String>>,
}

impl TryFrom<Option<&SecretSerdeValue>> for WorldpayraftMandateMetadata {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(data: Option<&SecretSerdeValue>) -> Result<Self, Self::Error> {
        data.map(|data| {
            Self::deserialize(data.peek()).change_context(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "mandate_metadata",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Expected Worldpay RAFT mandate_metadata returned by Authorize or SetupMandate; it carries the card network and expiry needed to reuse a stored TokenizedPAN".to_string(),
                        ),
                        ..Default::default()
                    },
                },
            )
        })
        .transpose()
        .map(|metadata| metadata.unwrap_or_default())
    }
}

impl TryFrom<(Option<&SecretSerdeValue>, Option<&MandateReferenceId>)>
    for WorldpayraftMandateMetadata
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        (feature_data, mandate): (Option<&SecretSerdeValue>, Option<&MandateReferenceId>),
    ) -> Result<Self, Self::Error> {
        let mut connector_metadata = WorldpayraftConnectorMetadata::try_from(feature_data)?;
        let mut network_transaction_id = None;
        let mut expiration_date = None;
        match mandate {
            Some(MandateReferenceId::NetworkMandateId(data)) => {
                network_transaction_id = Some(data.network_transaction_id.clone())
            }
            Some(MandateReferenceId::NetworkTokenWithNTI(data)) => {
                network_transaction_id = Some(data.network_transaction_id.clone())
            }
            Some(MandateReferenceId::ConnectorMandateId(data)) => {
                let stored_metadata = data.get_mandate_metadata();
                let stored = Self::try_from(stored_metadata.as_ref())?;
                let stored_metadata = stored.connector_metadata;
                network_transaction_id = stored.network_transaction_id;
                connector_metadata.card_network = connector_metadata
                    .card_network
                    .or(stored_metadata.card_network);
                expiration_date = stored.expiration_date;
            }
            None => {}
        }
        Ok(Self {
            connector_metadata,
            network_transaction_id,
            expiration_date,
        })
    }
}

impl WorldpayraftMandateMetadata {
    fn to_mandate_reference(&self, token: String) -> Box<MandateReference> {
        // Persist only the reusable credential details in secret mandate metadata.
        let mandate_metadata = Secret::new(serde_json::json!({
            "card_network": self.connector_metadata.card_network,
            "network_transaction_id": self.network_transaction_id,
            "expiration_date": self.expiration_date,
        }));
        Box::new(MandateReference {
            connector_mandate_id: Some(token),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata: Some(mandate_metadata),
        })
    }

    /// Recover only metadata; never turn an approved payment into a card-validation error.
    fn from_response_request<T: PaymentMethodDataTypes>(
        payment_method: &PaymentMethodData<T>,
        feature_data: Option<&SecretSerdeValue>,
        mandate: Option<&MandateReferenceId>,
    ) -> Self {
        let mut metadata = Self::try_from((feature_data, mandate)).unwrap_or_default();
        let (network, expiry) = match payment_method {
            PaymentMethodData::Card(card) => (
                resolve_card_network(card.card_network.clone(), card.card_number.peek()),
                card.get_expiry_date_as_yymm().ok(),
            ),
            PaymentMethodData::CardWithNoCvc(card) => (
                resolve_card_network(card.card_network.clone(), card.card_number.peek()),
                card.get_expiry_date_as_yymm().ok(),
            ),
            PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
                resolve_card_network(card.card_network.clone(), card.card_number.peek()),
                card.get_expiry_date_as_yymm().ok(),
            ),
            _ => (None, None),
        };
        metadata.connector_metadata.card_network =
            network.or(metadata.connector_metadata.card_network);
        metadata.expiration_date = expiry.or(metadata.expiration_date);
        metadata
    }

    fn set_authorization(
        &mut self,
        amount: &Money,
        payment_id: &str,
        pos: WorldpayraftPosData,
        brand_data: &WorldpayraftBrandSpecificData,
    ) {
        self.network_transaction_id = brand_data
            .network_transaction_id()
            .or(self.network_transaction_id.take());
        let data = &mut self.connector_metadata;
        data.original_authorized_amount = Some(amount.clone());
        data.terminal_data = Some(pos.terminal_data);
        data.ecommerce_data = Some(pos.ecommerce_data);
        data.proc_flags_indicators = pos.proc_flags;
        data.api_transaction_id = Some(truncate_api_transaction_id(payment_id));
    }

    fn from_authorize_response<T: PaymentMethodDataTypes>(
        request: &PaymentsAuthorizeData<T>,
        payment_id: &str,
        brand_data: &WorldpayraftBrandSpecificData,
    ) -> Self {
        let mandate = request
            .mandate_id
            .as_ref()
            .and_then(|id| id.mandate_reference_id.as_ref());
        let mut metadata = Self::from_response_request(
            &request.payment_method_data,
            request.connector_feature_data.as_ref(),
            mandate,
        );
        let kind = WorldpayraftPaymentKind::for_authorize(
            request,
            metadata.connector_metadata.is_stored_credential,
        );
        let pos = if is_debit_card(&request.payment_method_data) {
            WorldpayraftPosData::for_debit(request.is_auto_capture())
        } else {
            WorldpayraftPosData::for_credit(
                metadata.connector_metadata.card_network.as_ref(),
                request.mit_category.as_ref(),
                kind,
                request.is_auto_capture(),
                request.enable_partial_authorization,
            )
        };
        metadata.connector_metadata.is_stored_credential = kind != WorldpayraftPaymentKind::OneOff;
        metadata.set_authorization(&request.amount, payment_id, pos, brand_data);
        metadata
    }

    fn from_repeat_payment_response<T: PaymentMethodDataTypes>(
        request: &RepeatPaymentData<T>,
        payment_id: &str,
        brand_data: &WorldpayraftBrandSpecificData,
    ) -> Self {
        let mut metadata = Self::from_response_request(
            &request.payment_method_data,
            request.connector_feature_data.as_ref(),
            Some(&request.mandate_reference),
        );
        let pos = WorldpayraftPosData::for_credit(
            metadata.connector_metadata.card_network.as_ref(),
            request.mit_category.as_ref(),
            WorldpayraftPaymentKind::Mit,
            request.is_auto_capture(),
            request.enable_partial_authorization,
        );
        metadata.connector_metadata.is_stored_credential = true;
        metadata.set_authorization(&request.amount, payment_id, pos, brand_data);
        metadata
    }
}

/// PANs retain their holder type; no-CVC cards always contain validated PCI data.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftAuthorizePan<T: PaymentMethodDataTypes> {
    Card(RawCardNumber<T>),
    CardWithNoCvc(cards::CardNumber),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCardInfo<T: PaymentMethodDataTypes> {
    #[serde(rename = "PAN", skip_serializing_if = "Option::is_none")]
    pub pan: Option<WorldpayraftAuthorizePan<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftStoredTokenData {
    #[serde(rename = "TokenizedPAN")]
    pub tokenized_pan: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftCardVerificationData {
    #[serde(rename = "Cvv2Cvc2CIDIndicator")]
    pub cvv_indicator: WorldpayraftCvvIndicator,
    #[serde(rename = "Cvv2Cvc2CIDValue")]
    pub cvv2_cvc2: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftAddressVerificationData {
    #[serde(skip_serializing_if = "Option::is_none", rename = "AVSZIPCode")]
    pub avs_zip_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "AVSAddress")]
    pub avs_address: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftTerminalData {
    pub entry_mode: WorldpayraftEntryMode,
    #[serde(rename = "POSConditionCode")]
    pub pos_condition_code: WorldpayraftPosConditionCode,
    pub terminal_entry_cap: WorldpayraftTerminalEntryCapability,
    #[serde(rename = "POSEnvironment", skip_serializing_if = "Option::is_none")]
    pub pos_environment: Option<WorldpayraftPosEnvironment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldpayraftEcommerceData {
    #[serde(rename = "E-commerceIndicator")]
    pub ecommerce_indicator: WorldpayraftEcommerceIndicator,
}

// =============================================================================
// AUTHORIZE REQUEST
// =============================================================================

/// Fields shared by credit auth/purchase and debit preauth/purchase requests.
/// Credit-only fields are omitted when constructing debit requests.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCardAuthInner<T: PaymentMethodDataTypes> {
    pub misc_amounts_balances: WorldpayraftAmounts,
    pub card_info: WorldpayraftCardInfo<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption_token_data: Option<WorldpayraftStoredTokenData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_verification_data: Option<WorldpayraftCardVerificationData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_verification_data: Option<WorldpayraftAddressVerificationData>,
    pub terminal_data: WorldpayraftTerminalData,
    #[serde(rename = "E-commerceData")]
    pub ecommerce_data: WorldpayraftEcommerceData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proc_flags_indicators: Option<WorldpayraftProcFlags>,
    #[serde(flatten)]
    pub brand_specific_data: WorldpayraftBrandSpecificData,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    pub local_date_time: String,
}

/// Outer wrapper for authorize requests.
///
/// Credit cards (manual capture): `{ "creditauth": { ... } }` → POST /credit/authorization
/// Debit cards (manual capture):  `{ "debitpreauth": { ... } }` → POST /debit/preauth
/// Credit cards (auto-capture):   `{ "creditpurchase": { ... } }` → POST /credit/purchase
/// Debit cards (auto-capture):    `{ "debitpurchase": { ... } }` → POST /debit/purchase
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftAuthorizeRequest<T: PaymentMethodDataTypes> {
    Credit {
        creditauth: WorldpayraftCardAuthInner<T>,
    },
    Debit {
        debitpreauth: WorldpayraftCardAuthInner<T>,
    },
    CreditPurchase {
        creditpurchase: WorldpayraftCardAuthInner<T>,
    },
    DebitPurchase {
        debitpurchase: WorldpayraftCardAuthInner<T>,
    },
}

// =============================================================================
// AUTHORIZE RESPONSE
// =============================================================================

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftResponseTraceNumbers {
    pub authorization_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
    pub system_trace_number: Option<String>,
    pub network_ref_number: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldpayraftEncryptionTokenData {
    #[serde(rename = "TokenizedPAN")]
    pub tokenized_pan: Option<String>,
    #[serde(rename = "PAN-Last4")]
    pub pan_last4: Option<String>,
}

/// Inner fields shared by creditauthresponse and debitpreauthresponse.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCardAuthResponseInner {
    pub return_code: String,
    pub reason_code: Option<String>,
    pub response_code: String,
    pub reference_trace_numbers: Option<WorldpayraftResponseTraceNumbers>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenData>,
    #[serde(flatten)]
    pub brand_specific_data: WorldpayraftBrandSpecificData,
}

/// Outer wrapper for authorize responses.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftAuthorizeResponse {
    Credit {
        creditauthresponse: WorldpayraftCardAuthResponseInner,
    },
    Debit {
        debitpreauthresponse: WorldpayraftCardAuthResponseInner,
    },
    CreditPurchase {
        creditpurchaseresponse: WorldpayraftCardAuthResponseInner,
    },
    DebitPurchase {
        debitpurchaseresponse: WorldpayraftCardAuthResponseInner,
    },
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftAuthorizeRequest<T>
// =============================================================================

/// Card credentials shared by Authorize and RepeatPayment; CVV is handled only for CITs.
struct CardFields<T: PaymentMethodDataTypes> {
    pan: WorldpayraftAuthorizePan<T>,
    expiration_date: Secret<String>,
    is_debit: bool,
    network: Option<CardNetwork>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&PaymentMethodData<T>> for CardFields<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(payment_method: &PaymentMethodData<T>) -> Result<Self, Self::Error> {
        match payment_method {
            PaymentMethodData::Card(card) => Self::try_from(card),
            PaymentMethodData::CardWithNoCvc(card) => Self::try_from(card),
            PaymentMethodData::CardDetailsForNetworkTransactionId(card) => Self::try_from(card),
            _ => Err(error_stack::report!(
                errors::IntegrationError::NotImplemented(
                    "Only card payment methods are supported for Worldpay RAFT".to_string(),
                    errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Worldpay RAFT authorization requires a card PAN and expiry; use Card, CardWithNoCvc, or CardDetailsForNetworkTransactionId".to_string(),
                        ),
                        ..Default::default()
                    },
                )
            )),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&Card<T>> for CardFields<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(card: &Card<T>) -> Result<Self, Self::Error> {
        let pan = WorldpayraftAuthorizePan::Card(card.card_number.clone());
        let expiration_date = card.get_expiry_date_as_yymm()?;
        let is_debit = is_debit_card_type(card.card_type.as_deref());
        Ok(Self {
            pan,
            expiration_date,
            is_debit,
            network: resolve_card_network(card.card_network.clone(), card.card_number.peek()),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<&CardWithNoCvc> for CardFields<T> {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(card: &CardWithNoCvc) -> Result<Self, Self::Error> {
        let pan = WorldpayraftAuthorizePan::CardWithNoCvc(card.card_number.clone());
        let expiration_date = card.get_expiry_date_as_yymm()?;
        let is_debit = is_debit_card_type(card.card_type.as_deref());
        Ok(Self {
            pan,
            expiration_date,
            is_debit,
            network: resolve_card_network(card.card_network.clone(), card.card_number.peek()),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<&CardDetailsForNetworkTransactionId<T>> for CardFields<T> {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(card: &CardDetailsForNetworkTransactionId<T>) -> Result<Self, Self::Error> {
        let expiration_date = card.get_expiry_date_as_yymm()?;
        Ok(Self {
            pan: WorldpayraftAuthorizePan::Card(card.card_number.clone()),
            expiration_date,
            is_debit: is_debit_card_type(card.card_type.as_deref()),
            network: resolve_card_network(card.card_network.clone(), card.card_number.peek()),
        })
    }
}

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
                router_data.request.amount.amount,
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

        let request = &router_data.request;
        let card = CardFields::try_from(&request.payment_method_data)?;
        let is_debit = card.is_debit;
        let is_auto_capture = request.is_auto_capture();
        let mandate = request
            .mandate_id
            .as_ref()
            .and_then(|id| id.mandate_reference_id.as_ref());
        let mandate_metadata = WorldpayraftMandateMetadata::try_from((
            request.connector_feature_data.as_ref(),
            mandate,
        ))?;
        let metadata = mandate_metadata.connector_metadata;
        let network_transaction_id = mandate_metadata.network_transaction_id;
        let kind = WorldpayraftPaymentKind::for_authorize(request, metadata.is_stored_credential);
        let card_verification_data = match &request.payment_method_data {
            PaymentMethodData::Card(card)
                if kind != WorldpayraftPaymentKind::Mit && (!is_debit || is_auto_capture) =>
            {
                Some(WorldpayraftCardVerificationData {
                    cvv_indicator: WorldpayraftCvvIndicator::Provided,
                    cvv2_cvc2: card.card_cvc.clone(),
                })
            }
            _ => None,
        };
        let network = card.network.or(metadata.card_network);
        let pos = if is_debit {
            WorldpayraftPosData::for_debit(is_auto_capture)
        } else {
            validate_stored_card_network(
                network.as_ref(),
                request.mit_category.as_ref(),
                network_transaction_id.as_deref(),
            )?;
            WorldpayraftPosData::for_credit(
                network.as_ref(),
                request.mit_category.as_ref(),
                kind,
                is_auto_capture,
                request.enable_partial_authorization,
            )
        };
        let brand_specific_data = if !is_debit && kind == WorldpayraftPaymentKind::Mit {
            WorldpayraftBrandSpecificData::for_mit(
                network.as_ref(),
                network_transaction_id,
                request.mit_category.as_ref(),
            )
        } else {
            WorldpayraftBrandSpecificData::default()
        };

        let address_verification_data = if !is_debit
            && router_data.request.enable_avs_check == Some(true)
        {
            let billing = router_data.resource_common_data.get_billing_address()?;
            let zip = billing
                .zip
                .clone()
                .filter(|zip| !zip.peek().trim().is_empty());
            let address_line = billing
                .line1
                .clone()
                .filter(|line| !line.peek().trim().is_empty());
            if zip.is_none() || address_line.is_none() {
                return Err(errors::IntegrationError::MissingRequiredField {
                        field_name: "billing.address.zip and billing.address.line1",
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Worldpay RAFT requires both a non-empty billing postal code and a non-empty address line 1 for AVS verification when enable_avs_check is true".to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                    .into());
            }
            Some(WorldpayraftAddressVerificationData {
                avs_zip_code: zip,
                avs_address: address_line,
            })
        } else {
            None
        };

        let payment_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .as_str();
        let inner = WorldpayraftCardAuthInner {
            misc_amounts_balances: WorldpayraftAmounts { transaction_amount },
            card_info: WorldpayraftCardInfo {
                pan: Some(card.pan),
                expiration_date: Some(card.expiration_date),
            },
            encryption_token_data: None,
            card_verification_data,
            address_verification_data,
            terminal_data: pos.terminal_data,
            ecommerce_data: pos.ecommerce_data,
            proc_flags_indicators: pos.proc_flags,
            brand_specific_data,
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id: truncate_api_transaction_id(payment_id),
            local_date_time: get_local_datetime(),
        };

        match (is_debit, is_auto_capture) {
            (true, true) => Ok(Self::DebitPurchase {
                debitpurchase: inner,
            }),
            (true, false) => Ok(Self::Debit {
                debitpreauth: inner,
            }),
            (false, true) => Ok(Self::CreditPurchase {
                creditpurchase: inner,
            }),
            (false, false) => Ok(Self::Credit { creditauth: inner }),
        }
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
        let (is_debit, inner) = match &item.response {
            WorldpayraftAuthorizeResponse::Credit { creditauthresponse } => {
                (false, creditauthresponse)
            }
            WorldpayraftAuthorizeResponse::Debit {
                debitpreauthresponse,
            } => (true, debitpreauthresponse),
            WorldpayraftAuthorizeResponse::CreditPurchase {
                creditpurchaseresponse,
            } => (false, creditpurchaseresponse),
            WorldpayraftAuthorizeResponse::DebitPurchase {
                debitpurchaseresponse,
            } => (true, debitpurchaseresponse),
        };

        let status = payment_status(
            inner.return_code.as_str(),
            inner.response_code.as_str(),
            item.router_data.request.is_auto_capture(),
        );

        // Encode card type prefix so downstream flows can route correctly
        // Format: "{C|D}|{AuthorizationNumber}|{RetrievalREFNumber}|{SystemTraceNumber}"
        let prefix = if is_debit {
            TXN_TYPE_DEBIT
        } else {
            TXN_TYPE_CREDIT
        };
        let connector_transaction_id = inner
            .reference_trace_numbers
            .as_ref()
            .map(|trace| {
                let auth_num = trace.authorization_number.as_deref().unwrap_or("");
                let retrieval_ref = trace.retrieval_ref_number.as_deref().unwrap_or("");
                let sys_trace = trace.system_trace_number.as_deref().unwrap_or("");
                format!("{prefix}|{auth_num}|{retrieval_ref}|{sys_trace}")
            })
            .unwrap_or_default();

        let metadata = WorldpayraftMandateMetadata::from_authorize_response(
            &item.router_data.request,
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .as_str(),
            &inner.brand_specific_data,
        );
        let connector_metadata = serde_json::to_value(&metadata.connector_metadata)
            .change_context(errors::ConnectorError::response_handling_failed(
                item.http_code,
            ))?;
        let mandate_reference = inner
            .encryption_token_data
            .as_ref()
            .and_then(|data| data.tokenized_pan.clone())
            .map(|token| metadata.to_mandate_reference(token));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference,
                connector_metadata: Some(connector_metadata),
                network_txn_id: inner.brand_specific_data.network_transaction_id(),
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
// CAPTURE REQUEST
// =============================================================================

/// Trace numbers carried in the Capture request body.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCaptureTraceNumbers {
    pub authorization_number: String,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: String,
    pub system_trace_number: String,
}

/// Inner fields shared by creditcompletion and debitcompletion requests.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCompletionInner {
    pub misc_amounts_balances: WorldpayraftCompletionAmounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_data: Option<WorldpayraftTerminalData>,
    #[serde(rename = "E-commerceData", skip_serializing_if = "Option::is_none")]
    pub ecommerce_data: Option<WorldpayraftEcommerceData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proc_flags_indicators: Option<WorldpayraftProcFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_type: Option<WorldpayraftAuthorizationType>,
    pub reference_trace_numbers: WorldpayraftCaptureTraceNumbers,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    pub local_date_time: String,
}

/// Outer wrapper for capture requests.
///
/// Credit: `{ "creditcompletion": { ... } }` → POST /credit/completion
/// Debit:  `{ "debitcompletion": { ... } }` → POST /debit/completion
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

/// Inner fields shared by creditcompletionresponse and debitcompletionresponse.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftCompletionResponseInner {
    pub return_code: String,
    pub reason_code: Option<String>,
    pub response_code: String,
    pub reference_trace_numbers: Option<WorldpayraftCaptureTraceNumbers>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
}

/// Outer wrapper for capture responses.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftCaptureResponse {
    Credit {
        creditcompletionresponse: WorldpayraftCompletionResponseInner,
    },
    Debit {
        debitcompletionresponse: WorldpayraftCompletionResponseInner,
    },
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

        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.amount_to_capture.amount,
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

        let connector_txn_id = router_data.request.get_connector_transaction_id()?;
        let (is_debit, auth_num, retrieval_ref, sys_trace) =
            parse_connector_transaction_id(&connector_txn_id);

        if auth_num.is_empty() && retrieval_ref.is_empty() {
            return Err(error_stack::report!(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "connector_transaction_id",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(format!(
                            "Expected format: [C|D]|AuthorizationNumber|RetrievalREFNumber|SystemTraceNumber, got: {connector_txn_id}"
                        )),
                        ..Default::default()
                    },
                }
            ));
        }

        let metadata = WorldpayraftConnectorMetadata::try_from(
            router_data.request.connector_feature_data.as_ref(),
        )?;
        let preauthorized_amount = if is_debit {
            None
        } else {
            metadata
                .original_authorized_amount
                .as_ref()
                .or(router_data.resource_common_data.amount_authorized.as_ref())
                .or(router_data.resource_common_data.amount.as_ref())
                .map(|original_amount| {
                    item.connector
                        .amount_converter
                        .convert(original_amount.amount, original_amount.currency)
                        .change_context(errors::IntegrationError::AmountConversionFailed {
                            context: errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "Worldpay RAFT credit completion requires the original authorized amount in major currency units for MiscAmountsBalances.PreauthorizedAmount".to_string(),
                                ),
                                ..Default::default()
                            },
                        })
                })
                .transpose()?
        };
        // Older authorizations may have no metadata or original authorized amount.
        let has_original_amount = preauthorized_amount.is_some();
        let api_transaction_id = metadata.api_transaction_id.unwrap_or_else(|| {
            truncate_api_transaction_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
        });
        let local_date_time = get_local_datetime();

        let proc_flags_indicators = if is_debit {
            None
        } else {
            metadata
                .proc_flags_indicators
                .or_else(|| has_original_amount.then(WorldpayraftProcFlags::default))
                .map(|mut flags| {
                    flags.prior_auth = has_original_amount.then_some(WorldpayraftFlag::Yes);
                    flags
                })
        };
        let inner = WorldpayraftCompletionInner {
            misc_amounts_balances: WorldpayraftCompletionAmounts {
                transaction_amount,
                preauthorized_amount,
            },
            terminal_data: metadata.terminal_data,
            ecommerce_data: metadata.ecommerce_data,
            proc_flags_indicators,
            authorization_type: has_original_amount
                .then_some(WorldpayraftAuthorizationType::ForcePost),
            reference_trace_numbers: WorldpayraftCaptureTraceNumbers {
                authorization_number: auth_num.to_string(),
                retrieval_ref_number: retrieval_ref.to_string(),
                system_trace_number: sys_trace.to_string(),
            },
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id,
            local_date_time,
        };

        if is_debit {
            Ok(Self::Debit {
                debitcompletion: inner,
            })
        } else {
            Ok(Self::Credit {
                creditcompletion: inner,
            })
        }
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
        let inner = match &item.response {
            WorldpayraftCaptureResponse::Credit {
                creditcompletionresponse,
            } => creditcompletionresponse,
            WorldpayraftCaptureResponse::Debit {
                debitcompletionresponse,
            } => debitcompletionresponse,
        };

        let status = payment_status(
            inner.return_code.as_str(),
            inner.response_code.as_str(),
            true,
        );

        // Preserve the original composite connector_transaction_id from the Authorize flow
        // so downstream flows (refund) can still use the stored trace numbers.
        let connector_transaction_id = match &item.router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(txn_id) => txn_id.clone(),
            _ => String::new(),
        };
        let metadata = WorldpayraftConnectorMetadata::try_from(
            item.router_data.request.connector_feature_data.as_ref(),
        )
        .change_context(errors::ConnectorError::response_handling_failed(
            item.http_code,
        ))?;
        // Serialize the non-sensitive schema instead of echoing legacy metadata.
        let connector_metadata = item
            .router_data
            .request
            .connector_feature_data
            .as_ref()
            .map(|_| serde_json::to_value(&metadata))
            .transpose()
            .change_context(errors::ConnectorError::response_handling_failed(
                item.http_code,
            ))?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
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
// REFUND REQUEST
// =============================================================================

/// Trace numbers carried in the Refund request body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftRefundTraceNumbers {
    pub authorization_number: String,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: String,
    pub system_trace_number: String,
}

/// Inner fields shared by creditrefund and debitrefund requests.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftRefundInner {
    pub misc_amounts_balances: WorldpayraftAmounts,
    pub reference_trace_numbers: WorldpayraftRefundTraceNumbers,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    pub local_date_time: String,
}

/// Outer wrapper for refund requests.
///
/// Credit: `{ "creditrefund": { ... } }` → POST /credit/refund
/// Debit:  `{ "debitrefund": { ... } }` → POST /debit/refund
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftRefundResponseTraceNumbers {
    pub authorization_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
}

/// Inner fields shared by creditrefundresponse and debitrefundresponse.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftRefundResponseInner {
    pub return_code: String,
    pub reason_code: Option<String>,
    pub response_code: String,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
    pub reference_trace_numbers: Option<WorldpayraftRefundResponseTraceNumbers>,
}

/// Outer wrapper for refund responses.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftRefundResponse {
    Credit {
        creditrefundresponse: WorldpayraftRefundResponseInner,
    },
    Debit {
        debitrefundresponse: WorldpayraftRefundResponseInner,
    },
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

        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.refund_amount.amount,
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

        let connector_txn_id = &router_data.request.connector_transaction_id;
        let (is_debit, auth_num, retrieval_ref, sys_trace) =
            parse_connector_transaction_id(connector_txn_id);

        if auth_num.is_empty() && retrieval_ref.is_empty() {
            return Err(error_stack::report!(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "connector_transaction_id",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(format!(
                            "Expected format: [C|D]|AuthorizationNumber|RetrievalREFNumber|SystemTraceNumber, got: {connector_txn_id}"
                        )),
                        ..Default::default()
                    },
                }
            ));
        }

        let api_transaction_id = truncate_api_transaction_id(&router_data.request.refund_id);
        let local_date_time = get_local_datetime();

        let inner = WorldpayraftRefundInner {
            misc_amounts_balances: WorldpayraftAmounts { transaction_amount },
            reference_trace_numbers: WorldpayraftRefundTraceNumbers {
                authorization_number: auth_num.to_string(),
                retrieval_ref_number: retrieval_ref.to_string(),
                system_trace_number: sys_trace.to_string(),
            },
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id,
            local_date_time,
        };

        if is_debit {
            Ok(Self::Debit { debitrefund: inner })
        } else {
            Ok(Self::Credit {
                creditrefund: inner,
            })
        }
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
        let inner = match &item.response {
            WorldpayraftRefundResponse::Credit {
                creditrefundresponse,
            } => creditrefundresponse,
            WorldpayraftRefundResponse::Debit {
                debitrefundresponse,
            } => debitrefundresponse,
        };

        let refund_status = if inner.return_code == RETURN_CODE_SUCCESS
            && inner.response_code == RESPONSE_CODE_SUCCESS
        {
            RefundStatus::Success
        } else {
            RefundStatus::Failure
        };

        // connector_refund_id: prefer AuthorizationNumber from response trace numbers,
        // fall back to APITransactionID
        let connector_refund_id = inner
            .reference_trace_numbers
            .as_ref()
            .and_then(|t| t.authorization_number.clone())
            .or_else(|| inner.api_transaction_id.clone())
            .unwrap_or_default();

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
// Worldpay RAFT card tokenization endpoint: POST /tokenization/token
// Stores a card PAN as a TokenizedPAN which is returned as the mandate reference.

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftSetupMandateCardInfo<T: PaymentMethodDataTypes> {
    #[serde(rename = "PAN")]
    pub pan: RawCardNumber<T>,
    pub expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftSetupMandateInner<T: PaymentMethodDataTypes> {
    pub card_info: WorldpayraftSetupMandateCardInfo<T>,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    pub local_date_time: String,
}

/// Wrapper for the tokenize request body.
#[derive(Debug, Serialize)]
pub struct WorldpayraftSetupMandateRequest<T: PaymentMethodDataTypes> {
    pub tokenize: WorldpayraftSetupMandateInner<T>,
}

// =============================================================================
// SETUPMANDATE RESPONSE
// =============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct WorldpayraftSetupMandateTokenData {
    #[serde(rename = "TokenizedPAN")]
    pub tokenized_pan: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftSetupMandateTraceNumbers {
    pub authorization_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
    pub system_trace_number: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftSetupMandateResponseInner {
    pub return_code: String,
    pub reason_code: Option<String>,
    pub response_code: String,
    pub encryption_token_data: Option<WorldpayraftSetupMandateTokenData>,
    pub reference_trace_numbers: Option<WorldpayraftSetupMandateTraceNumbers>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
    #[serde(flatten)]
    pub brand_specific_data: WorldpayraftBrandSpecificData,
}

/// Wrapper for the tokenizeresponse body.
#[derive(Debug, Deserialize, Serialize)]
pub struct WorldpayraftSetupMandateResponse {
    pub tokenizeresponse: WorldpayraftSetupMandateResponseInner,
}

// =============================================================================
// TryFrom: RouterDataV2 → WorldpayraftSetupMandateRequest<T>
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

        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let card: &Card<T> = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "Only Card payment method is supported for Worldpay RAFT SetupMandate"
                            .to_string(),
                        errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Provide PaymentMethodData::Card with a PAN and expiry so Worldpay RAFT can create a TokenizedPAN for subsequent payments".to_string(),
                            ),
                            ..Default::default()
                        },
                    )
                ))
            }
        };

        let expiration_date = card.get_expiry_date_as_yymm()?;

        let pan = card.card_number.clone();

        let payment_id = &router_data
            .resource_common_data
            .connector_request_reference_id;
        let api_transaction_id = truncate_api_transaction_id(payment_id);
        let local_date_time = get_local_datetime();

        Ok(Self {
            tokenize: WorldpayraftSetupMandateInner {
                card_info: WorldpayraftSetupMandateCardInfo {
                    pan,
                    expiration_date,
                },
                world_pay_merchant_id: auth.merchant_id,
                api_transaction_id,
                local_date_time,
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
        let response = &item.response.tokenizeresponse;

        let status = payment_status(
            response.return_code.as_str(),
            response.response_code.as_str(),
            true,
        );

        let tokenized_pan = response
            .encryption_token_data
            .as_ref()
            .and_then(|etd| etd.tokenized_pan.clone());

        let mut metadata = WorldpayraftMandateMetadata::from_response_request(
            &item.router_data.request.payment_method_data,
            None,
            None,
        );
        metadata.connector_metadata.is_stored_credential = true;
        metadata.network_transaction_id = response.brand_specific_data.network_transaction_id();
        let connector_metadata = metadata
            .connector_metadata
            .encode_to_value()
            .change_context(errors::ConnectorError::response_handling_failed(
                item.http_code,
            ))?;

        let mandate_reference = tokenized_pan.map(|pan| metadata.to_mandate_reference(pan));

        // connector_transaction_id = "C|{AuthorizationNumber}|{RetrievalREFNumber}|{SystemTraceNumber}"
        // SetupMandate is always treated as a credit-path operation
        let connector_transaction_id = response
            .reference_trace_numbers
            .as_ref()
            .map(|trace| {
                let auth_num = trace.authorization_number.as_deref().unwrap_or("");
                let retrieval_ref = trace.retrieval_ref_number.as_deref().unwrap_or("");
                let sys_trace = trace.system_trace_number.as_deref().unwrap_or("");
                format!("{TXN_TYPE_CREDIT}|{auth_num}|{retrieval_ref}|{sys_trace}")
            })
            .unwrap_or_default();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference,
                connector_metadata: Some(connector_metadata),
                network_txn_id: response.brand_specific_data.network_transaction_id(),
                network_txn_link_id: None,
                connector_response_reference_id: response.api_transaction_id.clone(),
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
// REPEAT PAYMENT REQUEST
// =============================================================================
// Stored credentials use authorization for manual capture and purchase for auto capture.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftRepeatPaymentRequest<T: PaymentMethodDataTypes> {
    Credit {
        creditauth: WorldpayraftCardAuthInner<T>,
    },
    CreditPurchase {
        creditpurchase: WorldpayraftCardAuthInner<T>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorldpayraftRepeatPaymentResponse {
    Credit {
        creditauthresponse: WorldpayraftCardAuthResponseInner,
    },
    CreditPurchase {
        creditpurchaseresponse: WorldpayraftCardAuthResponseInner,
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
    > for WorldpayraftRepeatPaymentRequest<T>
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
                router_data.request.amount.amount,
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

        let request = &router_data.request;
        let metadata = WorldpayraftMandateMetadata::try_from((
            request.connector_feature_data.as_ref(),
            Some(&request.mandate_reference),
        ))?;
        // MIT needs only PAN/expiry or a Worldpay token. CardFields never reads CVV,
        // including when a caller supplies the full Card variant.
        let card = match &request.payment_method_data {
            PaymentMethodData::Card(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Some(CardFields::try_from(&request.payment_method_data)?)
            }
            _ => None,
        };
        let network = card
            .as_ref()
            .and_then(|card| card.network.clone())
            .or(metadata.connector_metadata.card_network);
        let (card_info, encryption_token_data) = match &request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(reference) => {
                let token = reference.get_connector_mandate_id().ok_or_else(|| {
                    errors::IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Worldpay token charges require a stored TokenizedPAN".to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                })?;
                (
                    WorldpayraftCardInfo {
                        pan: None,
                        expiration_date: card
                            .map(|card| card.expiration_date)
                            .or(metadata.expiration_date),
                    },
                    Some(WorldpayraftStoredTokenData {
                        tokenized_pan: Secret::new(token),
                    }),
                )
            }
            MandateReferenceId::NetworkMandateId(_) => {
                let card = card.ok_or_else(|| errors::IntegrationError::MissingRequiredField {
                    field_name: "payment_method_data.card",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Network-reference MITs require the card PAN and expiry".to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
                (
                    WorldpayraftCardInfo {
                        pan: Some(card.pan),
                        expiration_date: Some(card.expiration_date),
                    },
                    None,
                )
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(errors::IntegrationError::NotImplemented(
                    "Network tokens are not supported for Worldpay RAFT RepeatPayment".to_string(),
                    errors::IntegrationErrorContext::default(),
                )
                .into())
            }
        };
        let network_transaction_id = metadata.network_transaction_id;
        validate_stored_card_network(
            network.as_ref(),
            request.mit_category.as_ref(),
            network_transaction_id.as_deref(),
        )?;
        let pos = WorldpayraftPosData::for_credit(
            network.as_ref(),
            request.mit_category.as_ref(),
            WorldpayraftPaymentKind::Mit,
            request.is_auto_capture(),
            request.enable_partial_authorization,
        );
        let address_verification_data = router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing| billing.address.as_ref())
            .and_then(|address| {
                let zip = address
                    .zip
                    .clone()
                    .filter(|value| !value.peek().trim().is_empty());
                let line = address
                    .line1
                    .clone()
                    .filter(|value| !value.peek().trim().is_empty());
                (zip.is_some() && line.is_some()).then_some(WorldpayraftAddressVerificationData {
                    avs_zip_code: zip,
                    avs_address: line,
                })
            });
        let inner = WorldpayraftCardAuthInner {
            misc_amounts_balances: WorldpayraftAmounts { transaction_amount },
            card_info,
            encryption_token_data,
            card_verification_data: None,
            address_verification_data,
            terminal_data: pos.terminal_data,
            ecommerce_data: pos.ecommerce_data,
            proc_flags_indicators: pos.proc_flags,
            brand_specific_data: WorldpayraftBrandSpecificData::for_mit(
                network.as_ref(),
                network_transaction_id,
                request.mit_category.as_ref(),
            ),
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id: truncate_api_transaction_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
            local_date_time: get_local_datetime(),
        };
        if router_data.request.is_auto_capture() {
            Ok(Self::CreditPurchase {
                creditpurchase: inner,
            })
        } else {
            Ok(Self::Credit { creditauth: inner })
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
        let response = match &item.response {
            WorldpayraftRepeatPaymentResponse::Credit { creditauthresponse } => creditauthresponse,
            WorldpayraftRepeatPaymentResponse::CreditPurchase {
                creditpurchaseresponse,
            } => creditpurchaseresponse,
        };

        let status = payment_status(
            response.return_code.as_str(),
            response.response_code.as_str(),
            item.router_data.request.is_auto_capture(),
        );

        // RepeatPayment is always credit; encode with TXN_TYPE_CREDIT prefix
        let connector_transaction_id = response
            .reference_trace_numbers
            .as_ref()
            .map(|trace| {
                let auth_num = trace.authorization_number.as_deref().unwrap_or("");
                let retrieval_ref = trace.retrieval_ref_number.as_deref().unwrap_or("");
                let sys_trace = trace.system_trace_number.as_deref().unwrap_or("");
                format!("{TXN_TYPE_CREDIT}|{auth_num}|{retrieval_ref}|{sys_trace}")
            })
            .unwrap_or_default();

        let metadata = WorldpayraftMandateMetadata::from_repeat_payment_response(
            &item.router_data.request,
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .as_str(),
            &response.brand_specific_data,
        );
        let connector_metadata = metadata
            .connector_metadata
            .encode_to_value()
            .change_context(errors::ConnectorError::response_handling_failed(
                item.http_code,
            ))?;
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(connector_metadata),
                network_txn_id: response.brand_specific_data.network_transaction_id(),
                network_txn_link_id: None,
                connector_response_reference_id: response.api_transaction_id.clone(),
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
// A void is a reversal sent to the SAME endpoint as the original authorization:
//   - credit card auth   → POST /credit/authorization  (body key: "creditauth")
//   - debit card preauth → POST /debit/preauth          (body key: "debitpreauth")
// The reversal is distinguished from a normal auth by AuthorizationType: "RV".
// The original authorization is linked via ReferenceTraceNumbers.AuthorizationNumber.

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayraftVoidInner {
    pub misc_amounts_balances: WorldpayraftAmounts,
    pub authorization_type: WorldpayraftAuthorizationType,
    pub reversal_advice_reason_cd: WorldpayraftReversalAdviceReasonCode,
    pub reference_trace_numbers: WorldpayraftCaptureTraceNumbers,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    pub local_date_time: String,
}

/// Outer wrapper for void/reversal requests.
///
/// `{ "creditauth": { "AuthorizationType": "RV", ... } }` → POST /credit/authorization
#[derive(Debug, Serialize)]
pub struct WorldpayraftVoidRequest {
    pub creditauth: WorldpayraftVoidInner,
}

// =============================================================================
// VOID RESPONSE
// =============================================================================

/// Outer wrapper for void/reversal response.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldpayraftVoidResponse {
    pub creditauthresponse: WorldpayraftCardAuthResponseInner,
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
        let auth = WorldpayraftAuthType::try_from(&router_data.connector_config)?;

        let amount = router_data.request.amount.as_ref().ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT void requires the original authorized amount".to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?;
        let currency = router_data.request.currency.ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT void requires currency to convert the amount".to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?;

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(amount.amount, currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Worldpay RAFT requires the void amount in major currency units"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let connector_txn_id = &router_data.request.connector_transaction_id;
        let (_, auth_num, retrieval_ref, sys_trace) =
            parse_connector_transaction_id(connector_txn_id);

        if auth_num.is_empty() {
            return Err(error_stack::report!(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "connector_transaction_id",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(format!(
                            "Expected format: [C|D]|AuthorizationNumber|RetrievalREFNumber|SystemTraceNumber, got: {connector_txn_id}"
                        )),
                        ..Default::default()
                    },
                }
            ));
        }

        let reversal_advice_reason_cd = if router_data.request.cancellation_reason.is_some() {
            WorldpayraftReversalAdviceReasonCode::CustomerCancel
        } else {
            WorldpayraftReversalAdviceReasonCode::NormalReversal
        };

        let api_transaction_id = truncate_api_transaction_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );
        let local_date_time = get_local_datetime();

        Ok(Self {
            creditauth: WorldpayraftVoidInner {
                misc_amounts_balances: WorldpayraftAmounts { transaction_amount },
                authorization_type: WorldpayraftAuthorizationType::Reversal,
                reversal_advice_reason_cd,
                reference_trace_numbers: WorldpayraftCaptureTraceNumbers {
                    authorization_number: auth_num.to_string(),
                    retrieval_ref_number: retrieval_ref.to_string(),
                    system_trace_number: sys_trace.to_string(),
                },
                world_pay_merchant_id: auth.merchant_id,
                api_transaction_id,
                local_date_time,
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
        let inner = &item.response.creditauthresponse;

        let status = if map_payment_status(&inner.return_code, &inner.response_code) {
            AttemptStatus::Voided
        } else {
            AttemptStatus::VoidFailed
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
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
