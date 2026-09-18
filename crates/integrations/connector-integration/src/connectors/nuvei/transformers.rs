use common_utils::{consts, pii, request::Method, types::StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync,
        PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference, MandateReferenceId,
        NuveiClientAuthenticationResponse as NuveiClientAuthenticationResponseDomain,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        BankDebitData, BankRedirectData, BankTransferData, NetworkTokenData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::{ConnectorSpecificConfig, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use super::NuveiRouterData;
use crate::types::ResponseRouterData;
use domain_types::errors::{ConnectorError, IntegrationError};

// Nuvei's APM (Alternative Payment Method) identifier for ACH. Required literal
// per Nuvei's API; reused by both BankTransfer::AchBankTransfer and
// BankDebit::AchBankDebit. See https://docs.nuvei.com/documentation/us-and-canada-guides/ach/
const NUVEI_ACH_PAYMENT_METHOD: &str = "apmgw_ACH";

// Auth Type
#[derive(Debug, Clone)]
pub struct NuveiAuthType {
    pub(super) merchant_id: Secret<String>,
    pub(super) merchant_site_id: Secret<String>,
    pub(super) merchant_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NuveiAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nuvei {
                merchant_id,
                merchant_site_id,
                merchant_secret,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.clone(),
                merchant_site_id: merchant_site_id.clone(),
                merchant_secret: merchant_secret.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl NuveiAuthType {
    /// SHA-256 over the UTF-8 concatenation of `fields` (in the given order) with the
    /// merchant secret appended last, rendered as lower-case hex.
    ///
    /// `fields` carries the *name* of each member alongside its value so that the
    /// concatenation order of every call site is self-documenting and diagnosable:
    /// Nuvei publishes a different order per request type (see the techspec's
    /// "Checksum Generation — exact concatenation order per request type"), and a wrong
    /// order fails every call of that flow with `errCode 1001 Invalid checksum`.
    /// Only the field *names* are traced — never their values, and never the secret.
    pub fn generate_checksum(
        &self,
        request_type: &'static str,
        fields: &[(&'static str, &str)],
    ) -> Secret<String> {
        use sha2::{Digest, Sha256};

        tracing::debug!(
            connector = "nuvei",
            request_type,
            checksum_field_order = ?fields.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            "nuvei: building checksum (field order only, no values)"
        );

        let mut concatenated = fields.iter().map(|(_, value)| *value).collect::<String>();
        concatenated.push_str(self.merchant_secret.peek());

        let mut hasher = Sha256::new();
        hasher.update(concatenated.as_bytes());
        Secret::new(format!("{:x}", hasher.finalize()))
    }

    pub fn get_timestamp(
    ) -> common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss> {
        // Generate timestamp in YYYYMMDDHHmmss format using common_utils date_time
        common_utils::date_time::DateTime::from(common_utils::date_time::now())
    }
}

// ============================================================================
// Shared three-stage error model (spec: "## HTTP Codes and Errors")
// ============================================================================

/// Stage-2 (gateway) and stage-3 (APM) error members. Flattened into every Nuvei
/// response struct so that a decline is diagnosable from any flow, not just the two
/// that historically parsed `gwErrorCode` / `gwErrorReason`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NuveiGatewayError {
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i32>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    #[serde(rename = "gwExtendedErrorCode")]
    pub gw_extended_error_code: Option<i32>,
    #[serde(rename = "merchantAdviceCode")]
    pub merchant_advice_code: Option<String>,
    #[serde(rename = "issuerDeclineCode")]
    pub issuer_decline_code: Option<String>,
    #[serde(rename = "issuerDeclineReason")]
    pub issuer_decline_reason: Option<String>,
    #[serde(rename = "paymentMethodErrorCode")]
    pub payment_method_error_code: Option<i32>,
    #[serde(rename = "paymentMethodErrorReason")]
    pub payment_method_error_reason: Option<String>,
}

/// Resolved error identity for one Nuvei response, ready to be copied onto an
/// `ErrorResponse`.
#[derive(Debug, Clone)]
pub struct NuveiErrorFields {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
    pub network_advice_code: Option<String>,
    pub network_decline_code: Option<String>,
    pub network_error_message: Option<String>,
}

/// `gwErrorReason` value that Nuvei returns on an accepted envelope with no
/// `transactionStatus` at all; per the spec it must still be treated as an error.
const NUVEI_MISSING_ARGUMENT_REASON: &str = "Missing argument";

/// Shared three-stage error resolver.
///
/// Precedence (spec "### Error-code → connector-error mapping rules"):
/// 1. `status == ERROR` → `errCode` / `reason`;
/// 2. `status == SUCCESS` and `transactionStatus` in (DECLINED, ERROR) → `gwErrorCode` /
///    `gwErrorReason`;
/// 3. `status == SUCCESS` with no `transactionStatus` but `gwErrorReason == "Missing
///    argument"` → `gwErrorCode` / `gwErrorReason`.
///
/// `issuerDeclineCode` / `issuerDeclineReason` are preferred as the *issuer* decline
/// code/message when the acquirer forwards them; `merchantAdviceCode` is the network
/// advice code. Empty strings and absent members fall back to
/// `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`, never to `""` or a literal.
pub fn nuvei_error_fields(
    status: &NuveiPaymentStatus,
    transaction_status: Option<&NuveiTransactionStatus>,
    err_code: Option<i32>,
    reason: Option<&str>,
    gateway_error: &NuveiGatewayError,
) -> NuveiErrorFields {
    let non_empty = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };

    let use_envelope = matches!(status, NuveiPaymentStatus::Error);
    let use_gateway = !use_envelope
        && (matches!(
            transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        ) || (transaction_status.is_none()
            && gateway_error.gw_error_reason.as_deref() == Some(NUVEI_MISSING_ARGUMENT_REASON)));

    let (code, message) = if use_envelope {
        (
            non_empty(err_code.map(|code| code.to_string()).as_deref()),
            non_empty(reason),
        )
    } else if use_gateway {
        (
            non_empty(
                gateway_error
                    .gw_error_code
                    .map(|code| code.to_string())
                    .as_deref(),
            ),
            non_empty(gateway_error.gw_error_reason.as_deref()),
        )
    } else {
        // Not a documented error shape; still surface whatever the body carried rather
        // than inventing a value.
        (
            non_empty(err_code.map(|code| code.to_string()).as_deref()),
            non_empty(reason),
        )
    };

    if code.is_none() || message.is_none() {
        tracing::warn!(
            connector = "nuvei",
            has_code = code.is_some(),
            has_message = message.is_some(),
            "nuvei: error-mapping fallback reached, using NO_ERROR_CODE / NO_ERROR_MESSAGE"
        );
    }

    let network_decline_code =
        non_empty(gateway_error.issuer_decline_code.as_deref()).or_else(|| {
            non_empty(
                gateway_error
                    .gw_error_code
                    .map(|code| code.to_string())
                    .as_deref(),
            )
        });
    let network_error_message = non_empty(gateway_error.issuer_decline_reason.as_deref())
        .or_else(|| non_empty(gateway_error.gw_error_reason.as_deref()));

    NuveiErrorFields {
        code: code.unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
        message: message
            .clone()
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
        reason: message,
        network_advice_code: non_empty(gateway_error.merchant_advice_code.as_deref()),
        network_decline_code,
        network_error_message,
    }
}

/// Composite attempt-status map, keyed on `transactionStatus` × `transactionType`
/// plus the zero-amount override and the envelope status
/// (spec "### Composite mapping to a payment attempt status").
///
/// There is deliberately no `_ =>` arm on `transactionStatus`: an undocumented value
/// deserialises to [`NuveiTransactionStatus::Unknown`] and is traced.
pub fn nuvei_attempt_status(
    transaction_status: Option<&NuveiTransactionStatus>,
    transaction_type: Option<&NuveiTransactionType>,
    is_zero_amount: bool,
    envelope_status: &NuveiPaymentStatus,
) -> common_enums::AttemptStatus {
    use common_enums::AttemptStatus;

    let zero_amount_auth =
        is_zero_amount && matches!(transaction_type, Some(NuveiTransactionType::Auth));

    match transaction_status {
        Some(NuveiTransactionStatus::Approved) => {
            if zero_amount_auth {
                // An approved zero-dollar auth is a verification success, not an
                // authorisation to capture.
                return AttemptStatus::Charged;
            }
            match transaction_type {
                Some(NuveiTransactionType::Auth)
                | Some(NuveiTransactionType::PreAuth)
                | Some(NuveiTransactionType::InitAuth3D) => AttemptStatus::Authorized,
                Some(NuveiTransactionType::Sale) | Some(NuveiTransactionType::Settle) => {
                    AttemptStatus::Charged
                }
                Some(NuveiTransactionType::Void) | Some(NuveiTransactionType::VoidCredit) => {
                    AttemptStatus::Voided
                }
                Some(NuveiTransactionType::Auth3D) => AttemptStatus::AuthenticationPending,
                _ => AttemptStatus::Pending,
            }
        }
        Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
            if zero_amount_auth {
                return AttemptStatus::AuthorizationFailed;
            }
            match transaction_type {
                Some(NuveiTransactionType::Auth) | Some(NuveiTransactionType::PreAuth) => {
                    AttemptStatus::AuthorizationFailed
                }
                Some(NuveiTransactionType::Void) | Some(NuveiTransactionType::VoidCredit) => {
                    AttemptStatus::VoidFailed
                }
                Some(NuveiTransactionType::Auth3D) | Some(NuveiTransactionType::InitAuth3D) => {
                    AttemptStatus::AuthenticationFailed
                }
                Some(NuveiTransactionType::Settle) => AttemptStatus::CaptureFailed,
                _ => AttemptStatus::Failure,
            }
        }
        Some(NuveiTransactionStatus::Redirect) => {
            tracing::info!(
                connector = "nuvei",
                "nuvei: non-terminal transactionStatus REDIRECT, awaiting the redirection result"
            );
            AttemptStatus::AuthenticationPending
        }
        Some(NuveiTransactionStatus::Pending) | Some(NuveiTransactionStatus::Processing) => {
            AttemptStatus::Pending
        }
        Some(NuveiTransactionStatus::Unknown) => {
            tracing::warn!(
                connector = "nuvei",
                "nuvei: undocumented transactionStatus, treating the attempt as non-terminal"
            );
            AttemptStatus::Pending
        }
        None => {
            tracing::warn!(
                connector = "nuvei",
                envelope_status = ?envelope_status,
                "nuvei: response carried no transactionStatus, deciding from the envelope status"
            );
            match envelope_status {
                NuveiPaymentStatus::Failed | NuveiPaymentStatus::Error => AttemptStatus::Failure,
                NuveiPaymentStatus::Success | NuveiPaymentStatus::Processing => {
                    AttemptStatus::Pending
                }
            }
        }
    }
}

/// AVS verdict descriptions (spec "## AVS — Address Verification Service").
fn nuvei_avs_description(code: &str) -> &'static str {
    match code {
        "X" => "Exact match of both the 9-digit ZIP code and the street address",
        "Y" => "Postal code and the street address match",
        "A" => "The street address matches, the ZIP code does not",
        "W" | "Z" => "Postal code matches, the street address does not",
        "N" => "Both the street address and postal code do not match",
        "U" => "Issuer is unavailable",
        "S" => "AVS not supported by issuer",
        "R" => "Retry",
        "B" => "Not authorized (declined)",
        _ => "Unrecognised AVS code",
    }
}

/// CVV2 verdict descriptions (spec "## CVV / CVV2 verification").
fn nuvei_cvv_description(code: &str) -> &'static str {
    match code {
        "M" => "CVV2 Match",
        "N" => "CVV2 No Match",
        "P" => "Not Processed",
        "U" => "Issuer is not certified and/or has not supplied the encryption keys",
        "S" => "CVV2 processor is unavailable",
        _ => "Unrecognised CVV2 code",
    }
}

/// Builds the structured AVS / CVV / auth-code carrier for `PaymentFlowData.connector_response`.
///
/// Returns `None` when the response carried none of the four values, so an empty
/// `payment_checks` object is never attached.
pub fn nuvei_connector_response_data(
    payment_option: Option<&NuveiResponsePaymentOption>,
    auth_code: Option<&str>,
) -> Option<domain_types::router_data::ConnectorResponseData> {
    let card = payment_option.and_then(|option| option.card.as_ref());
    let avs_code = card
        .and_then(|card| card.avs_code.as_deref())
        .filter(|code| !code.is_empty());
    let cvv2_reply = card
        .and_then(|card| card.cvv2_reply.as_deref())
        .filter(|code| !code.is_empty());
    let card_network = card.and_then(|card| card.card_brand.clone());
    let auth_code = auth_code
        .filter(|code| !code.is_empty())
        .map(ToOwned::to_owned);

    if avs_code.is_none() && cvv2_reply.is_none() && card_network.is_none() && auth_code.is_none() {
        return None;
    }

    let payment_checks = serde_json::json!({
        "avs_result": avs_code,
        "avs_description": avs_code.map(nuvei_avs_description),
        "card_validation_result": cvv2_reply,
        "card_validation_description": cvv2_reply.map(nuvei_cvv_description),
    });

    Some(
        domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: Some(payment_checks),
                card_network,
                domestic_network: None,
                auth_code,
            },
        ),
    )
}

/// Network transaction id, read opportunistically: an empty string is dropped rather
/// than emitted as `Some("")` (spec Gaps G-06 — the response-side member is unverified).
pub fn nuvei_network_txn_id(external_scheme_transaction_id: Option<&str>) -> Option<String> {
    match external_scheme_transaction_id {
        Some(id) if !id.trim().is_empty() => Some(id.to_string()),
        Some(_) => {
            tracing::debug!(
                connector = "nuvei",
                "nuvei: dropping empty externalSchemeTransactionId instead of emitting network_txn_id"
            );
            None
        }
        None => None,
    }
}

/// `clientUniqueId` (the merchant's reference for the payment, `String(45)`) and
/// `clientRequestId` (unique to *this* HTTP call) are not interchangeable: reusing the
/// merchant reference for `clientRequestId` makes the second call on the same order fail
/// with `errCode 1089`.
///
/// Returns `(client_request_id, client_unique_id)`. Over-long references are **refused**,
/// never truncated.
pub fn nuvei_client_ids(
    connector_request_reference_id: &str,
    time_stamp: &common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
) -> Result<(String, String), Report<IntegrationError>> {
    if connector_request_reference_id.len() > NUVEI_MAX_CLIENT_UNIQUE_ID_LENGTH {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "client_unique_id",
            context: nuvei_error_context(
                "Shorten connector_request_reference_id to at most 45 characters; Nuvei's clientUniqueId is String(45) and must not be truncated silently.",
            ),
        }
        .into());
    }
    let client_request_id = format!("{connector_request_reference_id}-{time_stamp}");
    Ok((
        client_request_id,
        connector_request_reference_id.to_string(),
    ))
}

/// `clientUniqueId` is `String(45)` (spec "### Field length limits").
const NUVEI_MAX_CLIENT_UNIQUE_ID_LENGTH: usize = 45;

/// Nuvei's public API reference, used as the `doc_url` on every integration error.
const NUVEI_DOC_URL: &str = "https://docs.nuvei.com/api/main/indexMain_v1_0.html";

/// Never a bare `IntegrationErrorContext::default()`: every refusal carries a
/// remediation and a documentation link.
fn nuvei_error_context(suggested_action: &str) -> domain_types::errors::IntegrationErrorContext {
    domain_types::errors::IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_string()),
        doc_url: Some(NUVEI_DOC_URL.to_string()),
        additional_context: None,
    }
}

// Session Token Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

// Session Token Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenResponse {
    pub session_token: Option<Secret<String>>,
    pub internal_request_id: Option<i64>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub version: Option<String>,
    pub client_request_id: Option<String>,
}

// URL Details for redirect URLs
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiUrlDetails {
    pub success_url: String,
    pub failure_url: String,
    pub pending_url: String,
}

// Payment Request
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Option<Secret<String>>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub user_token_id: Option<Secret<String>>,
    pub client_unique_id: Option<String>,
    pub payment_option: NuveiPaymentOption<T>,
    pub transaction_type: NuveiTransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    /// Root-level `shippingAddress` (spec "## Address objects"); omitted entirely when
    /// the caller sent no shipping data.
    pub shipping_address: Option<NuveiShippingAddress>,
    /// Root-level `dynamicDescriptor` (spec "## `dynamicDescriptor`"); Nuvei rejects an
    /// empty object on some accounts, so it is emitted only when populated.
    pub dynamic_descriptor: Option<NuveiDynamicDescriptor>,
    /// Root-level risk/display basket — *not* `addendums.l23processingData`, which
    /// `/payment.do` rejects (spec "### The *other* basket").
    pub items: Option<Vec<NuveiBasketItem>>,
    pub amount_details: Option<NuveiAmountDetails>,
    /// `"0"` marks the initial customer-initiated transaction that establishes a
    /// credential on file.
    pub is_rebilling: Option<String>,
    pub is_partial_approval: Option<String>,
    pub merchant_details: Option<NuveiMerchantDetails>,
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

/// Root-level basket line item accepted by `/payment.do` for risk scoring and APM
/// display (spec "### The *other* basket: root-level `items` / `amountDetails`").
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiBasketItem {
    pub name: String,
    pub price: StringMajorUnit,
    pub quantity: String,
    pub group_id: Option<String>,
    pub discount: Option<StringMajorUnit>,
    pub tax: Option<StringMajorUnit>,
}

/// Root-level `amountDetails` on `/payment.do`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiAmountDetails {
    pub total_tax: Option<StringMajorUnit>,
    pub total_shipping: Option<StringMajorUnit>,
    pub total_handling: Option<StringMajorUnit>,
    pub total_discount: Option<StringMajorUnit>,
}

/// Root-level `merchantDetails` passthrough.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiMerchantDetails {
    pub custom_field1: Option<String>,
}

/// `dynamicDescriptor` — exactly the two documented members.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiDynamicDescriptor {
    /// Statement descriptor, max 25 characters.
    pub merchant_name: Option<String>,
    /// Customer-service contact shown on the statement, max 13 characters.
    pub merchant_phone: Option<Secret<String>>,
}

/// `paymentOption.card.threeD.externalMpi` — merchant-supplied 3DS.
///
/// Exactly the five documented members; `threeDSVersion` and `xid` are deliberately
/// **not** part of this object (spec "#### 5d. External MPI").
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiExternalMpi {
    pub eci: String,
    pub cavv: Secret<String>,
    /// JSON key is `dsTransID` — capital `ID`.
    #[serde(rename = "dsTransID")]
    pub ds_trans_id: String,
    /// Mandatory whenever external MPI values are sent.
    pub challenge_preference: NuveiChallengePreference,
    /// Mandatory when `challengePreference == ExemptionRequest`.
    pub exemption_request_reason: Option<NuveiExemptionRequestReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum NuveiChallengePreference {
    NoPreference,
    ExemptionRequest,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum NuveiExemptionRequestReason {
    AddCard,
    AccountVerification,
    LowValuePayment,
    TransactionRiskAnalysis,
}

/// `paymentOption.card.threeD` on the request side.
///
/// Three different call sites fill three disjoint subsets of this object and every
/// member is therefore optional:
///
/// * Authorize — `externalMpi` only (merchant-supplied 3DS, spec "#### 5d");
/// * PreAuthenticate (`/initPayment.do`) — `methodNotificationUrl` only
///   (spec "### 4. Initialise a 3DS payment");
/// * Authenticate (the first `/payment.do`) — the challenge block
///   `methodCompletionInd` / `version` / `notificationURL` / `merchantURL` /
///   `platformType` / `browserDetails` / `v2AdditionalParams` (spec "#### 5b").
///
/// The PostAuthenticate leg is deliberately unable to reach this type at all: its
/// request carries [`NuveiThreeDSFinalCard`], which has no `threeD` member, because a
/// `threeD` object on the final `/payment.do` is filter code 1155
/// ("3D Related transaction is missing or incorrect").
#[serde_with::skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeD {
    /// `/initPayment.do` only — where the issuer posts the fingerprinting result.
    pub method_notification_url: Option<String>,
    /// `Y` fingerprinting completed, `N` failed, `U` unavailable.
    pub method_completion_ind: Option<String>,
    /// Must echo the `version` returned by `/initPayment.do`.
    pub version: Option<String>,
    /// JSON key is `notificationURL` — capital `URL`, so it is renamed explicitly
    /// rather than left to the container's camelCase rule.
    #[serde(rename = "notificationURL")]
    pub notification_url: Option<String>,
    /// JSON key is `merchantURL` — capital `URL`, same reason.
    #[serde(rename = "merchantURL")]
    pub merchant_url: Option<String>,
    /// `01` mobile app, `02` browser.
    pub platform_type: Option<String>,
    pub v2_additional_params: Option<NuveiV2AdditionalParams>,
    /// Mandatory for `platformType == "02"`.
    pub browser_details: Option<NuveiBrowserDetails>,
    pub external_mpi: Option<NuveiExternalMpi>,
}

/// `paymentOption.card.threeD.v2AdditionalParams` (spec "#### 5b").
///
/// `challengePreference` is deliberately **not** modelled: the spec documents it both
/// as a numeric `01`–`03` request value and as a string enum on the response side with
/// no published mapping between the two (spec Gaps), so inventing one would be a guess.
#[serde_with::skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiV2AdditionalParams {
    /// `01`–`05`; `05` is full screen.
    pub challenge_window_size: Option<String>,
    /// `YYYYMMDD`; required by Nuvei only when `isRebilling == "0"`, which the
    /// authentication legs never send.
    pub rebill_expiry: Option<String>,
    /// Recurring frequency in days; paired with `rebillExpiry`.
    pub rebill_frequency: Option<String>,
}

/// `paymentOption.card.threeD.browserDetails` (spec "#### 5b").
///
/// Nuvei takes every member as a string, including the booleans (`TRUE` / `FALSE`) and
/// the numeric screen geometry, so the conversion happens here and not on the wire type.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiBrowserDetails {
    pub accept_header: Option<String>,
    /// The cardholder IP; the same value as root-level `deviceDetails.ipAddress`.
    pub ip: Secret<String, pii::IpAddress>,
    /// `TRUE` / `FALSE`, uppercase.
    pub java_enabled: Option<String>,
    /// `TRUE` / `FALSE`, uppercase.
    pub java_script_enabled: Option<String>,
    pub language: Option<String>,
    pub color_depth: Option<String>,
    pub screen_height: Option<String>,
    pub screen_width: Option<String>,
    pub time_zone: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<NuveiCardPaymentOption<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternative_payment_method: Option<NuveiAlternativePaymentMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_payment_option_id: Option<Secret<String>>,
}

// Serialize-only: untagged is wire-invisible, so raw-card requests keep their
// exact previous shape while network-token requests emit the externalToken form
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiCardPaymentOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Raw(NuveiCard<T>),
    NetworkToken(NuveiNetworkTokenCard),
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    pub card_holder_name: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(rename = "CVV")]
    pub cvv: Secret<String>,
    /// The `threeD` block, boxed: it carries the whole 3DS challenge object and would
    /// otherwise make `NuveiCardPaymentOption`'s raw-card variant an order of magnitude
    /// larger than its network-token one. Boxing is invisible on the wire.
    pub three_d: Option<Box<NuveiThreeD>>,
}

/// card object used when paying with a network token: no PAN/CVV/holder name,
/// only expiry + externalToken (mirrors hyperswitch get_network_token_info)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiNetworkTokenCard {
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub external_token: NuveiNetworkTokenExternalToken,
}

/// Nuvei externalToken payload for network-token payments.
/// tokenAssuranceLevel / tokenRequestorId mirror hyperswitch PR #13093, which
/// always sends None for them today; skip_serializing_none keeps them off the wire.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiNetworkTokenExternalToken {
    pub network_token_number: cards::NetworkToken,
    pub network_token_cryptogram: Option<Secret<String>>,
    pub token_assurance_level: Option<String>,
    pub token_requestor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiCardType {
    Visa,
    MasterCard,
    Amex,
    Discover,
    Diners,
}

impl TryFrom<common_enums::CardNetwork> for NuveiCardType {
    type Error = Report<IntegrationError>;

    fn try_from(network: common_enums::CardNetwork) -> Result<Self, Self::Error> {
        match network {
            common_enums::CardNetwork::Visa => Ok(Self::Visa),
            common_enums::CardNetwork::Mastercard => Ok(Self::MasterCard),
            common_enums::CardNetwork::AmericanExpress => Ok(Self::Amex),
            common_enums::CardNetwork::Discover => Ok(Self::Discover),
            common_enums::CardNetwork::DinersClub => Ok(Self::Diners),
            _ => Err(IntegrationError::NotSupported {
                message: format!("Card network {network:?}"),
                connector: "nuvei",
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl TryFrom<&domain_types::utils::CardIssuer> for NuveiCardType {
    type Error = Report<IntegrationError>;

    fn try_from(issuer: &domain_types::utils::CardIssuer) -> Result<Self, Self::Error> {
        match issuer {
            domain_types::utils::CardIssuer::Visa => Ok(Self::Visa),
            domain_types::utils::CardIssuer::Master => Ok(Self::MasterCard),
            domain_types::utils::CardIssuer::AmericanExpress => Ok(Self::Amex),
            domain_types::utils::CardIssuer::Discover => Ok(Self::Discover),
            domain_types::utils::CardIssuer::DinersClub => Ok(Self::Diners),
            _ => Err(IntegrationError::NotSupported {
                message: format!("Card issuer {issuer:?}"),
                connector: "nuvei",
                context: Default::default(),
            }
            .into()),
        }
    }
}

/// externalSchemeDetails: carries the original network transaction id (NTID)
/// and card brand for MIT network-token payments
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiExternalSchemeDetails {
    pub transaction_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<NuveiCardType>,
}

/// Shared card mapping for network-token CIT and MIT requests
fn build_nuvei_network_token_card(token_data: &NetworkTokenData) -> NuveiNetworkTokenCard {
    NuveiNetworkTokenCard {
        expiration_month: token_data.get_network_token_expiry_month(),
        expiration_year: token_data.get_network_token_expiry_year(),
        external_token: NuveiNetworkTokenExternalToken {
            network_token_number: token_data.get_network_token(),
            network_token_cryptogram: token_data.get_cryptogram(),
            token_assurance_level: None,
            token_requestor_id: None,
        },
    }
}

/// Brand for externalSchemeDetails: prefer the explicit card_network, fall back
/// to BIN-derived issuer
fn get_nuvei_card_brand(
    token_data: &NetworkTokenData,
) -> Result<NuveiCardType, Report<IntegrationError>> {
    match token_data.card_network.clone() {
        Some(network) => NuveiCardType::try_from(network),
        None => NuveiCardType::try_from(&token_data.get_card_issuer()?),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlternativePaymentMethodType {
    #[serde(rename = "apmgw_Giropay")]
    Giropay,
    #[serde(rename = "apmgw_Sofort")]
    Sofort,
    #[serde(rename = "apmgw_iDeal")]
    Ideal,
    #[serde(rename = "apmgw_EPS")]
    Eps,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NuveiBIC {
    #[serde(rename = "ABNANL2A")]
    Abnamro,
    #[serde(rename = "ASNBNL21")]
    AsnBank,
    #[serde(rename = "BUNQNL2A")]
    Bunq,
    #[serde(rename = "INGBNL2A")]
    Ing,
    #[serde(rename = "KNABNL2H")]
    Knab,
    #[serde(rename = "RABONL2U")]
    Rabobank,
    #[serde(rename = "RBRBNL21")]
    Regiobank,
    #[serde(rename = "SNSBNL2A")]
    SnsBank,
    #[serde(rename = "TRIONL2U")]
    TriodosBank,
    #[serde(rename = "FVLBNL22")]
    VanLanschotBankiers,
    #[serde(rename = "MOYONL21")]
    Moneyou,
}

impl TryFrom<common_enums::BankNames> for NuveiBIC {
    type Error = Report<IntegrationError>;

    fn try_from(bank: common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank {
            common_enums::BankNames::AbnAmro => Ok(Self::Abnamro),
            common_enums::BankNames::AsnBank => Ok(Self::AsnBank),
            common_enums::BankNames::Bunq => Ok(Self::Bunq),
            common_enums::BankNames::Ing => Ok(Self::Ing),
            common_enums::BankNames::Knab => Ok(Self::Knab),
            common_enums::BankNames::Rabobank => Ok(Self::Rabobank),
            common_enums::BankNames::Regiobank => Ok(Self::Regiobank),
            common_enums::BankNames::SnsBank => Ok(Self::SnsBank),
            common_enums::BankNames::TriodosBank => Ok(Self::TriodosBank),
            common_enums::BankNames::VanLanschot => Ok(Self::VanLanschotBankiers),
            common_enums::BankNames::Moneyou => Ok(Self::Moneyou),
            _ => Err(IntegrationError::NotSupported {
                message: format!("Bank not supported by Nuvei iDEAL: {}", bank),
                connector: "nuvei",
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiAlternativePaymentMethod {
    Ach {
        #[serde(rename = "paymentMethod")]
        payment_method: String,
        #[serde(rename = "AccountNumber")]
        account_number: Secret<String>,
        #[serde(rename = "RoutingNumber")]
        routing_number: Secret<String>,
        #[serde(rename = "SECCode", skip_serializing_if = "Option::is_none")]
        sec_code: Option<String>,
    },
    Redirect {
        #[serde(rename = "paymentMethod")]
        payment_method: AlternativePaymentMethodType,
        #[serde(rename = "BIC")]
        bank_id: Option<NuveiBIC>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiDeviceDetails {
    pub ip_address: Secret<String, pii::IpAddress>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiBillingAddress {
    // The only two members Nuvei enforces on /payment.do.
    pub email: pii::Email,
    /// ISO-3166-1 alpha-2, uppercase; an invalid value is `errCode 1014`.
    pub country: common_enums::CountryAlpha2,
    // Optional fields. `address` and `zip` are what drive the AVS check.
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub phone: Option<Secret<String>>,
    /// Mobile number.
    pub cell: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub address: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    pub address_line3: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub county: Option<Secret<String>>,
}

/// Root-level `shippingAddress` (spec "### `shippingAddress`"). Note that the county
/// member is keyed `shippingCounty` here, not `county` as on `billingAddress`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiShippingAddress {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub address: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    pub address_line3: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country: Option<common_enums::CountryAlpha2>,
    pub email: Option<pii::Email>,
    pub phone: Option<Secret<String>>,
    pub cell: Option<Secret<String>>,
    pub shipping_county: Option<Secret<String>>,
}

/// Build a Nuvei `billingAddress` block from `PaymentFlowData`. Returns
/// `None` if either of the two fields Nuvei requires (email + country)
/// is missing, so MIT flows can treat a missing block as "skip" while
/// CIT flows `.ok_or(...)` a specific error.
fn get_billing_address(
    resource_data: &PaymentFlowData,
    fallback_email: Option<pii::Email>,
) -> Option<NuveiBillingAddress> {
    let email = resource_data
        .get_optional_billing_email()
        .or(fallback_email)?;
    let country = resource_data.get_optional_billing_country()?;
    Some(build_billing_address(resource_data, email, country))
}

/// Shared `billingAddress` builder: the two required members are supplied by the
/// caller (each flow refuses in its own way when they are absent), every other member
/// is optional and simply omitted when the caller sent nothing.
fn build_billing_address(
    resource_data: &PaymentFlowData,
    email: pii::Email,
    country: common_enums::CountryAlpha2,
) -> NuveiBillingAddress {
    let billing_line3 = resource_data
        .get_optional_billing()
        .and_then(|billing| billing.address.as_ref())
        .and_then(|addr| addr.line3.clone());
    NuveiBillingAddress {
        email,
        country,
        first_name: resource_data.get_optional_billing_first_name(),
        last_name: resource_data.get_optional_billing_last_name(),
        phone: resource_data.get_optional_billing_phone_number(),
        cell: resource_data.get_optional_billing_phone_number(),
        city: resource_data.get_optional_billing_city(),
        address: resource_data.get_optional_billing_line1(),
        address_line2: resource_data.get_optional_billing_line2(),
        address_line3: billing_line3,
        zip: resource_data.get_optional_billing_zip(),
        state: resource_data.get_optional_billing_state(),
        county: None,
    }
}

/// Build the root-level `shippingAddress` from `PaymentFlowData`. Returns `None` when
/// the caller sent no shipping data at all, so the member stays off the wire.
fn get_shipping_address(resource_data: &PaymentFlowData) -> Option<NuveiShippingAddress> {
    resource_data.get_optional_shipping()?;
    let shipping = NuveiShippingAddress {
        first_name: resource_data.get_optional_shipping_first_name(),
        last_name: resource_data.get_optional_shipping_last_name(),
        address: resource_data.get_optional_shipping_line1(),
        address_line2: resource_data.get_optional_shipping_line2(),
        address_line3: resource_data.get_optional_shipping_line3(),
        city: resource_data.get_optional_shipping_city(),
        state: resource_data.get_optional_shipping_state(),
        zip: resource_data.get_optional_shipping_zip(),
        country: resource_data.get_optional_shipping_country(),
        email: resource_data.get_optional_shipping_email(),
        phone: resource_data.get_optional_shipping_phone_number(),
        cell: resource_data.get_optional_shipping_phone_number(),
        shipping_county: None,
    };
    let is_empty = shipping.first_name.is_none()
        && shipping.last_name.is_none()
        && shipping.address.is_none()
        && shipping.address_line2.is_none()
        && shipping.address_line3.is_none()
        && shipping.city.is_none()
        && shipping.state.is_none()
        && shipping.zip.is_none()
        && shipping.country.is_none()
        && shipping.email.is_none()
        && shipping.phone.is_none();
    if is_empty {
        None
    } else {
        Some(shipping)
    }
}

/// Build the root-level `dynamicDescriptor`. Emits the object only when at least one
/// member is populated — Nuvei rejects an empty object on some accounts.
fn get_dynamic_descriptor(
    statement_descriptor: Option<&str>,
    descriptor_phone: Option<Secret<String>>,
) -> Option<NuveiDynamicDescriptor> {
    let merchant_name = statement_descriptor
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            name.chars()
                .take(NUVEI_MAX_DESCRIPTOR_NAME_LENGTH)
                .collect()
        });
    let merchant_phone = descriptor_phone;
    if merchant_name.is_none() && merchant_phone.is_none() {
        return None;
    }
    Some(NuveiDynamicDescriptor {
        merchant_name,
        merchant_phone,
    })
}

/// `dynamicDescriptor.merchantName` is String(25) (spec "## `dynamicDescriptor`").
const NUVEI_MAX_DESCRIPTOR_NAME_LENGTH: usize = 25;

// Payment Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
    pub auth_code: Option<String>,
    pub session_token: Option<Secret<String>>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    /// See spec Gaps G-06: read opportunistically, never fabricated.
    pub external_scheme_transaction_id: Option<String>,
    pub transaction_link_id: Option<String>,
    #[serde(rename = "paymentOption")]
    pub payment_option: Option<NuveiResponsePaymentOption>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiPaymentStatus {
    Success,
    Failed,
    Error,
    #[default]
    Processing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiTransactionStatus {
    #[serde(alias = "Approved", alias = "APPROVED")]
    Approved,
    #[serde(alias = "Declined", alias = "DECLINED")]
    Declined,
    #[serde(alias = "Filter Error", alias = "ERROR", alias = "Error")]
    Error,
    #[serde(alias = "Redirect", alias = "REDIRECT")]
    Redirect,
    #[serde(alias = "Pending", alias = "PENDING")]
    Pending,
    #[serde(alias = "Processing", alias = "PROCESSING")]
    Processing,
    /// An undocumented connector value. It must never silently become `Processing`:
    /// [`nuvei_attempt_status`] traces it and keeps the attempt non-terminal.
    #[serde(other)]
    Unknown,
}

/// `transactionType` (spec "### `transactionType`"). Only `Sale` and `Auth` are ever
/// sent; the remaining variants exist so that the response side is exhaustively typed
/// rather than matched against string literals.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NuveiTransactionType {
    Sale,
    Auth,
    PreAuth,
    Settle,
    Credit,
    Void,
    VoidCredit,
    InitAuth3D,
    Auth3D,
    VerifyAuth3D,
    Chargeback,
    Modification,
    #[serde(other)]
    Unknown,
}

impl NuveiTransactionType {
    /// `Auth` for a manual capture or a zero-amount verification, `Sale` otherwise.
    ///
    /// The decision is made on the typed `MinorUnit`, never on a parsed `f64`: money is
    /// not compared as a float and a failed parse must not silently pick a branch.
    fn get_from_capture_method(
        capture_method: Option<common_enums::CaptureMethod>,
        minor_amount: common_utils::types::MinorUnit,
    ) -> Self {
        if capture_method == Some(common_enums::CaptureMethod::Manual)
            || minor_amount == common_utils::types::MinorUnit::zero()
        {
            Self::Auth
        } else {
            Self::Sale
        }
    }
}

/// The response-side `paymentOption` view, shared by every flow that returns one.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponsePaymentOption {
    pub redirect_url: Option<String>,
    /// Nuvei's stored-card token — the mandate / connector token.
    pub user_payment_option_id: Option<String>,
    pub card: Option<NuveiResponseCard>,
}

/// `paymentOption.card` on the response: the AVS / CVV verdicts and the 3DS block.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponseCard {
    /// Issuer AVS verdict, one character (spec "## AVS").
    pub avs_code: Option<String>,
    /// Issuer CVV2 verdict, one character (spec "## CVV / CVV2 verification").
    pub cvv2_reply: Option<String>,
    pub card_brand: Option<String>,
    pub card_type: Option<String>,
    pub bin: Option<String>,
    pub last4_digits: Option<String>,
    pub issuer_bank_name: Option<String>,
    pub issuer_country: Option<String>,
    pub three_d: Option<NuveiResponseThreeD>,
}

/// `paymentOption.card.threeD` on the response (spec "#### 5b — 3DS response fields"
/// and "### 4. Initialise a 3DS payment").
///
/// One view covers all three legs: `/initPayment.do` fills the capability and
/// fingerprinting members, the first `/payment.do` fills the challenge pair, and the
/// second fills the authentication result. Every member is optional because no leg
/// returns all of them.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponseThreeD {
    pub acs_url: Option<String>,
    #[serde(rename = "cReq")]
    pub c_req: Option<String>,
    /// `/initPayment.do`: the string `"true"` or `"false"`, **not** a JSON boolean.
    /// It is the documented branch field — `"false"` means the card is not enrolled
    /// for 3DS 2 and the payment must fall back to a plain `/payment.do`.
    pub v2supported: Option<String>,
    /// Negotiated 3DS message version, e.g. `2.2.0`.
    pub version: Option<String>,
    /// `/initPayment.do`: the 3DS method (device fingerprinting) endpoint.
    pub method_url: Option<String>,
    /// `/initPayment.do`: the base64 payload to POST to `methodUrl` as `threeDSMethodData`.
    pub method_payload: Option<String>,
    /// 3DS server transaction id, carried forward as `threeds_server_transaction_id`.
    pub server_trans_id: Option<String>,
    pub acs_trans_id: Option<String>,
    /// JSON key is `dsTransID` — capital `ID`.
    #[serde(rename = "dsTransID")]
    pub ds_trans_id: Option<String>,
    pub cavv: Option<Secret<String>>,
    /// Visa `5` / Mastercard `2` on a successful authentication, `7` for 3RI.
    pub eci: Option<String>,
    /// `Y` authenticated, `N` not authenticated, `C` challenge required,
    /// `U` unavailable, `A` attempted, `R` rejected.
    pub result: Option<String>,
    /// `challenge`, `frictionless`, `exemption`, `none`, or `softDecline` on a
    /// recoverable decline (spec "#### 5e. Soft-decline recovery").
    pub flow: Option<String>,
    /// `"1"` when the liability has shifted to the issuer.
    pub is_liability_on_issuer: Option<String>,
    /// Failure reason id on a failed authentication.
    pub three_d_reason_id: Option<String>,
    pub three_d_reason: Option<String>,
}

// Sync Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

// Sync Response (getTransactionDetails has different structure than payment response)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncResponse {
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
    pub internal_request_id: Option<i64>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub version: Option<String>,
    /// Nuvei's own order identifier. This is the value `/payment.do` returns as
    /// `orderId` and the one the Authorize response publishes as
    /// `connector_response_reference_id`; a sync re-emits the *same* identifier so a
    /// payment never carries two different reference ids (P-PSync-03).
    pub order_id: Option<String>,
    /// The merchant reference echoed back. It is deliberately **not** used as the
    /// response reference id: it is the caller's own id, not Nuvei's.
    pub client_unique_id: Option<String>,
    pub transaction_link_id: Option<String>,
    /// Scheme network transaction id. Unverified on this endpoint (spec Gaps G-06), so
    /// it is read opportunistically and dropped when empty.
    pub external_scheme_transaction_id: Option<String>,
    pub payment_option: Option<NuveiResponsePaymentOption>,
    pub transaction_details: Option<NuveiTransactionDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiTransactionDetails {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub auth_code: Option<String>,
    pub client_unique_id: Option<String>,
    pub date: Option<String>,
    pub original_transaction_date: Option<String>,
    pub credited: Option<String>,
    pub acquiring_bank_name: Option<String>,
    pub transaction_type: Option<NuveiTransactionType>,
    /// The amount Nuvei actually processed, as a decimal **major**-unit string
    /// (spec "### 9. Transaction sync" response). Typed, never a bare `String`, so the
    /// partial-capture comparison goes through the amount framework (INV-19).
    pub processed_amount: Option<StringMajorUnit>,
    pub processed_currency: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
}

// Capture Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCaptureRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    /// Level 2 / Level 3 interchange data. `/settleTransaction.do` is the **only**
    /// Nuvei endpoint that accepts it — `/payment.do` rejects the block outright.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addendums: Option<NuveiAddendums>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

/// Addendum container on `/settleTransaction.do` (spec "## Level 2 / Level 3
/// processing data (addendums)").
#[derive(Debug, Serialize)]
pub struct NuveiAddendums {
    #[serde(rename = "l23processingData")]
    pub l23_processing_data: NuveiL23ProcessingData,
}

/// `addendums.l23processingData` root fields.
///
/// Only the members reachable from `PaymentFlowData.l2_l3_data` are emitted (UD-22):
/// `customerCode`, `destinationZip`, `shipFromZip` and `destinationCountryCode` need a
/// customer and an address on the capture request, which the capture contract does not
/// carry, so they are deliberately **not** sent. Nuvei's Level 2&3 page asks for every
/// relevant field to be populated for interchange qualification, so a settle built from
/// an incomplete `L2L3Data` may still qualify only at Level 2.
#[derive(Debug, Serialize)]
pub struct NuveiL23ProcessingData {
    /// `0` tax not included, `1` state/provincial tax included, `2` not subject to tax.
    #[serde(rename = "taxIndicator", skip_serializing_if = "Option::is_none")]
    pub tax_indicator: Option<String>,
    #[serde(rename = "merchantVATRegNum", skip_serializing_if = "Option::is_none")]
    pub merchant_vat_reg_num: Option<Secret<String>>,
    #[serde(rename = "customerVATRegNum", skip_serializing_if = "Option::is_none")]
    pub customer_vat_reg_num: Option<Secret<String>>,
    /// `YYMMDD`.
    #[serde(rename = "orderDate", skip_serializing_if = "Option::is_none")]
    pub order_date: Option<String>,
    #[serde(rename = "lineItemCount", skip_serializing_if = "Option::is_none")]
    pub line_item_count: Option<String>,
    #[serde(rename = "items", skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<NuveiL23Item>>,
    #[serde(rename = "amountDetails", skip_serializing_if = "Option::is_none")]
    pub amount_details: Option<NuveiL23AmountDetails>,
}

/// One `addendums.l23processingData.items[]` line item.
#[derive(Debug, Serialize)]
pub struct NuveiL23Item {
    #[serde(rename = "commodityCode", skip_serializing_if = "Option::is_none")]
    pub commodity_code: Option<String>,
    #[serde(rename = "description")]
    pub description: String,
    #[serde(rename = "productCode", skip_serializing_if = "Option::is_none")]
    pub product_code: Option<String>,
    #[serde(rename = "quantity")]
    pub quantity: String,
    #[serde(rename = "unitMeasure", skip_serializing_if = "Option::is_none")]
    pub unit_measure: Option<String>,
    #[serde(rename = "price")]
    pub price: StringMajorUnit,
    #[serde(rename = "vatOrTaxAmount", skip_serializing_if = "Option::is_none")]
    pub vat_or_tax_amount: Option<StringMajorUnit>,
    #[serde(rename = "vatOrTaxRate", skip_serializing_if = "Option::is_none")]
    pub vat_or_tax_rate: Option<String>,
    #[serde(rename = "totalAmount", skip_serializing_if = "Option::is_none")]
    pub total_amount: Option<StringMajorUnit>,
    #[serde(rename = "discountRate", skip_serializing_if = "Option::is_none")]
    pub discount_rate: Option<String>,
    #[serde(rename = "discount", skip_serializing_if = "Option::is_none")]
    pub discount: Option<StringMajorUnit>,
    #[serde(rename = "taxType", skip_serializing_if = "Option::is_none")]
    pub tax_type: Option<String>,
}

/// `addendums.l23processingData.amountDetails{}`.
#[derive(Debug, Serialize)]
pub struct NuveiL23AmountDetails {
    #[serde(rename = "totalDiscount", skip_serializing_if = "Option::is_none")]
    pub total_discount: Option<StringMajorUnit>,
    #[serde(rename = "totalShipping", skip_serializing_if = "Option::is_none")]
    pub total_shipping: Option<StringMajorUnit>,
    #[serde(rename = "dutyAmount", skip_serializing_if = "Option::is_none")]
    pub duty_amount: Option<StringMajorUnit>,
    /// VAT / tax charged on freight or shipping.
    #[serde(rename = "vatOrTaxAmount", skip_serializing_if = "Option::is_none")]
    pub vat_or_tax_amount: Option<StringMajorUnit>,
    /// State / provincial tax on the order.
    #[serde(rename = "taxAmount", skip_serializing_if = "Option::is_none")]
    pub tax_amount: Option<StringMajorUnit>,
}

// Capture Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCaptureResponse {
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub internal_request_id: Option<i64>,
    pub transaction_id: Option<String>,
    pub status: NuveiPaymentStatus,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub auth_code: Option<String>,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
}

// Refund Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

// Refund Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundResponse {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
}

// Refund Sync Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundSyncRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

// Refund Sync Response (separate type to avoid macro conflicts)
//
// P-RSync-01: `/getTransactionDetails.do` answers a refund lookup with exactly the same
// envelope it answers a payment lookup with — the transaction's own members live in a
// nested `transactionDetails` object and **never** at the root (spec "### 9. Transaction
// sync"; "### Flow: RSync"). The previous shape read `transactionId` / `transactionStatus`
// off the root, so serde produced `None` for both on every reply and the id lookup below
// failed: that is the whole of the baseline `RESPONSE_HANDLING_FAILED` on all three
// RefundService/Get scenarios. The nested block is the one `NuveiSyncResponse` already
// declares, so the two syncs share a single decoding of the same endpoint.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundSyncResponse {
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
    /// The merchant reference echoed back. Never used as an identifier the refund is
    /// published under: it is the caller's own id, not Nuvei's.
    pub client_unique_id: Option<String>,
    /// Carries `transactionId`, `transactionStatus`, `transactionType` (`Credit` for a
    /// refund) and the `gw*` error members.
    pub transaction_details: Option<NuveiTransactionDetails>,
}

// Void Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiVoidRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

// Void Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiVoidResponse {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
}

/// Nuvei does not enumerate which states are voidable — the documentation says only that
/// a void is possible for "a short window, possibly up to 24 hours". Voidability is
/// therefore discovered at runtime from the refusal code, and this table gives each
/// documented refusal its published meaning so a not-voidable answer is reported as such
/// instead of as an anonymous failure.
fn nuvei_void_refusal(code: i32) -> Option<&'static str> {
    match code {
        1082 => Some("relatedTransactionId is invalid"),
        1092 => Some("the transaction does not support the requested operation"),
        1176 => Some("an authorization transaction cannot be voided"),
        1177 => Some("a credit cannot be voided"),
        1178 => Some("a void cannot be voided"),
        1179 => Some("this void cannot be performed"),
        1288 => Some("void is not supported for the related transaction type"),
        1559 => Some("the authorization has already been settled and can no longer be voided"),
        3506 => Some("the transaction id is wrong or the transaction does not exist"),
        _ => None,
    }
}

/// Attempt status for a void response.
///
/// Keyed on the shared composite map with the void's own `transactionType` — Nuvei echoes
/// `Void` / `VoidCredit`, so `Void` is assumed when the member is absent — and with the
/// generic payment `Failure` narrowed to `VoidFailed`, including on the
/// absent-`transactionStatus` branch: a refused cancellation leaves the underlying
/// authorisation intact and must never be reported as a failed payment.
fn nuvei_void_attempt_status(response: &NuveiVoidResponse) -> common_enums::AttemptStatus {
    let status = nuvei_attempt_status(
        response.transaction_status.as_ref(),
        response
            .transaction_type
            .as_ref()
            .or(Some(&NuveiTransactionType::Void)),
        false,
        &response.status,
    );
    match status {
        common_enums::AttemptStatus::Failure => common_enums::AttemptStatus::VoidFailed,
        status => status,
    }
}

// Error Response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiErrorResponse {
    pub reason: Option<String>,
    pub err_code: Option<String>,
    pub status: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    /// Stage-2 / stage-3 members, so the shared `ConnectorCommon::build_error_response`
    /// can surface the network advice and issuer decline codes the GSM smart-retry
    /// error-code update reads.
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
}

impl NuveiErrorResponse {
    /// Resolves the error identity of a transport-level error body through the shared
    /// three-stage resolver.
    pub fn resolve_error_fields(&self) -> NuveiErrorFields {
        let envelope_status = match self.status.as_deref() {
            Some("ERROR") | Some("Error") => NuveiPaymentStatus::Error,
            Some("FAILED") | Some("Failed") => NuveiPaymentStatus::Failed,
            Some("SUCCESS") | Some("Success") => NuveiPaymentStatus::Success,
            _ => NuveiPaymentStatus::Error,
        };
        let mut fields = nuvei_error_fields(
            &envelope_status,
            self.transaction_status.as_ref(),
            None,
            self.reason.as_deref(),
            &self.gateway_error,
        );
        // `errCode` is a string on this body, so it is resolved here rather than by the
        // shared helper's `Option<i32>` parameter.
        if let Some(code) = self
            .err_code
            .as_deref()
            .map(str::trim)
            .filter(|code| !code.is_empty())
        {
            fields.code = code.to_string();
        }
        fields
    }
}

// Session Token Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerSessionAuthenticationTokenRequestData,
                domain_types::connector_types::ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for NuveiSessionTokenRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerSessionAuthenticationTokenRequestData,
                domain_types::connector_types::ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Generate checksum for getSessionToken: merchantId + merchantSiteId + clientRequestId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(
            "getSessionToken.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            time_stamp,
            checksum,
        })
    }
}

// Session Token Response Transformation
impl TryFrom<ResponseRouterData<NuveiSessionTokenResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::ServerSessionAuthenticationToken,
        MerchantAuthenticationFlowData,
        domain_types::connector_types::ServerSessionAuthenticationTokenRequestData,
        domain_types::connector_types::ServerSessionAuthenticationTokenResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiSessionTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_fields = nuvei_error_fields(
                &response.status,
                None,
                response.err_code,
                response.reason.as_deref(),
                &NuveiGatewayError::default(),
            );
            let raw_connector_response = nuvei_raw_response(response);

            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract session token
        let session_token = response.session_token.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("session_token missing in Nuvei response".to_string()),
            ))
        })?;

        let session_response_data =
            domain_types::connector_types::ServerSessionAuthenticationTokenResponseData {
                // The domain type carries a plain String; the value is only
                // de-masked at this boundary.
                session_token: session_token.peek().to_string(),
            };

        Ok(Self {
            response: Ok(session_response_data),
            ..router_data.clone()
        })
    }
}

// Sync Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for NuveiSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();

        // Both keys are sent, but only `transactionId` is load-bearing: REST 1.0 support
        // for a `clientUniqueId`-only lookup is unconfirmed (spec Gaps G-09).
        // `clientUniqueId` is validated to String(45) here, never truncated.
        let (_client_request_id, client_unique_id) = nuvei_client_ids(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
            &time_stamp,
        )?;
        // G-PSync-01: without Nuvei's transactionId there is no reliable lookup key. The
        // gRPC surface types connector_transaction_id as a non-optional proto3 string, so
        // an absent id arrives as ResponseId::ConnectorTransactionId("") and never as
        // NoResponseId: an empty id must be refused here too.
        let transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) | ResponseId::EncodedData(id)
                if !id.trim().is_empty() =>
            {
                id.clone()
            }
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: nuvei_error_context(
                        "Nuvei's /getTransactionDetails.do is keyed by the transactionId the authorisation returned; sync the payment only after an Authorize response carried one.",
                    ),
                }
                .into());
            }
        };

        // Generate checksum for getTransactionDetails: merchantId + merchantSiteId + transactionId + clientUniqueId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(
            "getTransactionDetails.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("transactionId", &transaction_id),
                ("clientUniqueId", &client_unique_id),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_unique_id,
            transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiPaymentRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
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
        let resource_data = &router_data.resource_common_data;
        let request = &router_data.request;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // G-Authorize-06: Nuvei's transactionType only distinguishes Sale (automatic)
        // from Auth/PreAuth (manual) on /payment.do — there is no multi-capture or
        // scheduled transaction type. Refuse before the call, never silently downgrade.
        if matches!(
            request.capture_method,
            Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        ) {
            return Err(IntegrationError::CaptureMethodNotSupported {
                context: nuvei_error_context(
                    "Nuvei /payment.do supports only automatic (Sale) and manual (Auth) capture; send capture_method AUTOMATIC or MANUAL.",
                ),
            }
            .into());
        }

        // G-Authorize-02: billingAddress.email is one of the two members Nuvei enforces.
        let email = resource_data
            .get_optional_billing_email()
            .or_else(|| request.email.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing_address.email",
                context: nuvei_error_context(
                    "Send billing_address.email (or the request-level email); Nuvei enforces billingAddress.email on /payment.do.",
                ),
            })?;

        // G-Authorize-03: billingAddress.country is the other enforced member; an
        // invalid value is errCode 1014.
        let country = resource_data.get_optional_billing_country().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "billing_address.country",
                context: nuvei_error_context(
                    "Send billing_address.country as an ISO-3166-1 alpha-2 code; Nuvei rejects a missing or invalid country with errCode 1014.",
                ),
            },
        )?;

        // G-Authorize-04: deviceDetails.ipAddress is a required root member.
        let ip_address = request
            .browser_info
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "browser_info.ip_address",
                context: nuvei_error_context(
                    "Send browser_info.ip_address; Nuvei requires deviceDetails.ipAddress on /payment.do.",
                ),
            })?
            .ip_address
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "browser_info.ip_address",
                context: nuvei_error_context(
                    "Send browser_info.ip_address; Nuvei requires deviceDetails.ipAddress on /payment.do.",
                ),
            })?;

        // G-Authorize-01: /payment.do requires a sessionToken — the
        // ServerSessionAuthenticationToken pre-step is mandatory on every payments call.
        let session_token = resource_data
            .session_token
            .clone()
            .filter(|token| !token.trim().is_empty())
            .map(Secret::new)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: nuvei_error_context(
                    "Run MerchantAuthenticationService/CreateServerSessionAuthenticationToken first; /payment.do rejects a call without a sessionToken.",
                ),
            })?;

        // P-Authorize-07: userTokenId identifies the customer, not the payer email. It
        // is required for any transaction that should create or reuse a stored payment
        // option, and it is sent on every arm — never derived by de-masking the email.
        let user_token_id = resource_data
            .connector_customer
            .clone()
            .or_else(|| {
                resource_data
                    .customer_id
                    .as_ref()
                    .map(|customer_id| customer_id.get_string_repr().to_string())
            })
            .or_else(|| {
                request
                    .customer_id
                    .as_ref()
                    .map(|customer_id| customer_id.get_string_repr().to_string())
            })
            .map(Secret::new);

        // P-Authorize-02 / G-Authorize-07 / G-Authorize-08: external MPI.
        let three_d =
            build_nuvei_external_mpi(request.authentication_data.as_ref())?.map(|external_mpi| {
                Box::new(NuveiThreeD {
                    external_mpi: Some(external_mpi),
                    ..Default::default()
                })
            });

        // Extract payment method data
        let payment_option = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_holder_name = resource_data
                    .get_optional_billing_full_name()
                    .or(request.customer_name.clone().map(Secret::new))
                    .or_else(|| card_data.card_holder_name.clone())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_address.first_name and billing_address.last_name, customer_name or payment_method.card.card_holder_name",
                        context: nuvei_error_context(
                            "Send billing_address.first_name + billing_address.last_name, customer_name, or payment_method.card.card_holder_name; Nuvei requires paymentOption.card.cardHolderName.",
                        ),
                    })?;

                NuveiPaymentOption {
                    card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                        card_number: card_data.card_number.clone(),
                        card_holder_name,
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        cvv: card_data.card_cvc.clone(),
                        three_d,
                    })),
                    alternative_payment_method: None,
                    user_payment_option_id: None,
                }
            }
            // Network-token CIT: expiry + externalToken only, no PAN/CVV/holder
            // name (a token payment must not fail on missing billing name)
            PaymentMethodData::NetworkToken(token_data) => NuveiPaymentOption {
                card: Some(NuveiCardPaymentOption::NetworkToken(
                    build_nuvei_network_token_card(token_data),
                )),
                alternative_payment_method: None,
                user_payment_option_id: None,
            },
            PaymentMethodData::BankDebit(bank_debit_data) => {
                match bank_debit_data {
                    BankDebitData::AchBankDebit {
                        account_number,
                        routing_number,
                        bank_account_holder_name: _,
                        bank_holder_type,
                        ..
                    } => {
                        // SEC (Standard Entry Class) code: CCD for Business,
                        // WEB for Personal/consumer-initiated entries.
                        let sec_code = Some(
                            match bank_holder_type {
                                Some(common_enums::BankHolderType::Business) => "CCD",
                                Some(common_enums::BankHolderType::Personal) | None => "WEB",
                            }
                            .to_string(),
                        );

                        NuveiPaymentOption {
                            card: None,
                            alternative_payment_method: Some(NuveiAlternativePaymentMethod::Ach {
                                payment_method: NUVEI_ACH_PAYMENT_METHOD.to_string(),
                                account_number: Secret::new(account_number.peek().to_string()),
                                routing_number: Secret::new(routing_number.peek().to_string()),
                                sec_code,
                            }),
                            user_payment_option_id: None,
                        }
                    }
                    other => {
                        return Err(IntegrationError::NotSupported {
                            message: format!("{:?} is not supported for Nuvei", other),
                            connector: "nuvei",
                            context: nuvei_error_context(
                                "Nuvei's ACH APM covers only AchBankDebit on this transformer.",
                            ),
                        }
                        .into())
                    }
                }
            }
            PaymentMethodData::BankTransfer(bank_transfer_data) => {
                match bank_transfer_data.as_ref() {
                    BankTransferData::AchBankTransfer {} => {
                        // For ACH Bank Transfer, Nuvei requires account_number and routing_number
                        // These should be provided in the request metadata as ACH details
                        let metadata = request.metadata.as_ref().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "metadata for ACH details",
                                context: nuvei_error_context(
                                    "Send metadata.ach.{account_number,routing_number}; Nuvei's ACH APM needs the bank account on the request.",
                                ),
                            },
                        )?;

                        let ach_data = metadata.peek().get("ach").ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "ach in metadata",
                                context: nuvei_error_context(
                                    "Send metadata.ach.{account_number,routing_number}; Nuvei's ACH APM needs the bank account on the request.",
                                ),
                            },
                        )?;

                        let account_number = ach_data
                            .get("account_number")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "account_number",
                                context: nuvei_error_context(
                                    "Send metadata.ach.account_number for a Nuvei ACH payment.",
                                ),
                            })?;

                        let routing_number = ach_data
                            .get("routing_number")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "routing_number",
                                context: nuvei_error_context(
                                    "Send metadata.ach.routing_number for a Nuvei ACH payment.",
                                ),
                            })?;

                        let sec_code = ach_data
                            .get("sec_code")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .map(String::from);

                        NuveiPaymentOption {
                            card: None,
                            alternative_payment_method: Some(NuveiAlternativePaymentMethod::Ach {
                                payment_method: NUVEI_ACH_PAYMENT_METHOD.to_string(),
                                account_number: Secret::new(account_number.to_string()),
                                routing_number: Secret::new(routing_number.to_string()),
                                sec_code,
                            }),
                            user_payment_option_id: None,
                        }
                    }
                    other => {
                        return Err(IntegrationError::NotSupported {
                            message: format!("{:?} is not supported for Nuvei", other),
                            connector: "nuvei",
                            context: nuvei_error_context(
                                "Nuvei's ACH APM covers only AchBankTransfer on this transformer.",
                            ),
                        }
                        .into())
                    }
                }
            }
            PaymentMethodData::BankRedirect(ref redirect_data) => {
                let payment_method = match redirect_data {
                    BankRedirectData::Eps { .. } => AlternativePaymentMethodType::Eps,
                    BankRedirectData::Giropay { .. } => AlternativePaymentMethodType::Giropay,
                    BankRedirectData::Ideal { bank_name } => {
                        if let Some(ref bank) = bank_name {
                            let _ = NuveiBIC::try_from(*bank)?;
                        }
                        AlternativePaymentMethodType::Ideal
                    }
                    BankRedirectData::Sofort { .. } => AlternativePaymentMethodType::Sofort,
                    other => {
                        return Err(IntegrationError::NotSupported {
                            message: format!(
                                "Bank redirect method {:?} not supported by Nuvei",
                                other
                            ),
                            connector: "nuvei",
                            context: nuvei_error_context(
                                "Nuvei supports EPS, Giropay, iDEAL and Sofort bank redirects.",
                            ),
                        }
                        .into())
                    }
                };

                let bank_id = match redirect_data {
                    BankRedirectData::Ideal { bank_name } => bank_name
                        .as_ref()
                        .map(|bank| NuveiBIC::try_from(*bank))
                        .transpose()?,
                    _ => None,
                };

                NuveiPaymentOption {
                    card: None,
                    alternative_payment_method: Some(NuveiAlternativePaymentMethod::Redirect {
                        payment_method,
                        bank_id,
                    }),
                    user_payment_option_id: None,
                }
            }
            PaymentMethodData::PaymentMethodToken(token_data) => NuveiPaymentOption {
                card: None,
                alternative_payment_method: None,
                user_payment_option_id: Some(token_data.token.clone()),
            },
            PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                return Err(IntegrationError::NotSupported {
                    message: "NTID MIT is served by the RepeatPayment flow, not Authorize"
                        .to_string(),
                    connector: "nuvei",
                    context: nuvei_error_context(
                        "Use RecurringPaymentService/Charge for a merchant-initiated transaction carrying a network transaction id.",
                    ),
                }
                .into())
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Payment method not supported by Nuvei in this transformer".to_string(),
                    nuvei_error_context(
                        "Nuvei supports card, network token, stored payment option, ACH and the EPS/Giropay/iDEAL/Sofort bank redirects on Authorize.",
                    ),
                )
                .into())
            }
        };

        // billingAddress: email + country are the two enforced members; `address` and
        // `zip` are what produce a meaningful AVS verdict.
        let billing_address = build_billing_address(resource_data, email, country);
        let shipping_address = get_shipping_address(resource_data);

        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address.to_string()),
        };

        let time_stamp = NuveiAuthType::get_timestamp();

        // P-foundation-13: clientUniqueId is the merchant's reference for the payment
        // (String(45), guarded not truncated); clientRequestId is unique to this call so
        // a retry or a capture-after-auth on the same reference cannot collide with 1089.
        let (client_request_id, client_unique_id) =
            nuvei_client_ids(&resource_data.connector_request_reference_id, &time_stamp)?;

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Nuvei /payment.do takes the amount in decimal major units as a string.",
                ),
            })?;

        let currency = request.currency;

        // Determine transaction type from the typed minor amount and capture method.
        let transaction_type = NuveiTransactionType::get_from_capture_method(
            request.capture_method,
            request.minor_amount,
        );

        // Build urlDetails from router_return_url if available
        let url_details = request
            .router_return_url
            .as_ref()
            .map(|url| NuveiUrlDetails {
                success_url: url.clone(),
                failure_url: url.clone(),
                pending_url: url.clone(),
            });

        // dynamicDescriptor: statement descriptor text and service phone.
        let dynamic_descriptor = get_dynamic_descriptor(
            request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.as_deref()),
            request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.phone.clone()),
        );

        // "0" marks the initial customer-initiated transaction that establishes the
        // credential on file.
        let is_rebilling = (request.customer_acceptance.is_some()
            || request.setup_mandate_details.is_some())
        .then(|| "0".to_string());

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + amount +
        // currency + timeStamp + merchantSecretKey (techspec checksum table, row 4).
        let checksum = auth.generate_checksum(
            "payment.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            session_token: Some(session_token),
            merchant_id: auth.merchant_id.clone(),
            merchant_site_id: auth.merchant_site_id.clone(),
            client_request_id,
            amount,
            currency,
            user_token_id,
            client_unique_id: Some(client_unique_id),
            payment_option,
            transaction_type,
            device_details,
            billing_address,
            shipping_address,
            dynamic_descriptor,
            // Level 2/3 data rides on /settleTransaction.do only: addendums are
            // rejected on /payment.do, so the Capture flow owns them.
            items: None,
            amount_details: None,
            is_rebilling,
            is_partial_approval: None,
            merchant_details: None,
            url_details,
            time_stamp,
            checksum,
        })
    }
}

/// Builds `paymentOption.card.threeD.externalMpi` from the caller's merchant-supplied
/// 3DS values (spec "#### 5d. External MPI").
///
/// `eci`, `cavv` and `dsTransID` are required together — a partial block is a hard
/// reject (G-Authorize-07). `challengePreference` is mandatory whenever external MPI
/// values are sent, and `exemptionRequestReason` is mandatory when it is
/// `ExemptionRequest` (G-Authorize-08).
fn build_nuvei_external_mpi(
    authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
) -> Result<Option<NuveiExternalMpi>, Report<IntegrationError>> {
    let Some(authentication_data) = authentication_data else {
        return Ok(None);
    };

    let eci = authentication_data.eci.clone().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "authentication_data.eci",
            context: nuvei_error_context(
                "Nuvei's externalMpi requires eci, cavv and ds_transaction_id together; sending a partial block is a hard reject.",
            ),
        },
    )?;
    let cavv = authentication_data.cavv.clone().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "authentication_data.cavv",
            context: nuvei_error_context(
                "Nuvei's externalMpi requires eci, cavv and ds_transaction_id together; sending a partial block is a hard reject.",
            ),
        },
    )?;
    let ds_trans_id = authentication_data.ds_trans_id.clone().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "authentication_data.ds_transaction_id",
            context: nuvei_error_context(
                "Nuvei's externalMpi requires eci, cavv and ds_transaction_id together; sending a partial block is a hard reject.",
            ),
        },
    )?;

    let (challenge_preference, exemption_request_reason) = match authentication_data
        .exemption_indicator
        .clone()
    {
        Some(exemption_indicator) => {
            let reason = nuvei_exemption_request_reason(exemption_indicator).ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "authentication_data.exemption_indicator",
                        context: nuvei_error_context(
                            "exemptionRequestReason is mandatory when challengePreference is ExemptionRequest; Nuvei accepts AddCard, AccountVerification, LowValuePayment or TransactionRiskAnalysis.",
                        ),
                    },
                )?;
            (NuveiChallengePreference::ExemptionRequest, Some(reason))
        }
        None => (NuveiChallengePreference::NoPreference, None),
    };

    Ok(Some(NuveiExternalMpi {
        eci,
        cavv,
        ds_trans_id,
        challenge_preference,
        exemption_request_reason,
    }))
}

/// Maps the domain exemption indicator onto the four values Nuvei documents for
/// `exemptionRequestReason`. An indicator Nuvei has no equivalent for returns `None`,
/// which G-Authorize-08 turns into a refusal rather than an unnamed exemption.
fn nuvei_exemption_request_reason(
    exemption_indicator: common_enums::ExemptionIndicator,
) -> Option<NuveiExemptionRequestReason> {
    match exemption_indicator {
        common_enums::ExemptionIndicator::LowValue => {
            Some(NuveiExemptionRequestReason::LowValuePayment)
        }
        common_enums::ExemptionIndicator::TransactionRiskAssessment => {
            Some(NuveiExemptionRequestReason::TransactionRiskAnalysis)
        }
        common_enums::ExemptionIndicator::TrustedListing
        | common_enums::ExemptionIndicator::ScaDelegation => {
            Some(NuveiExemptionRequestReason::AddCard)
        }
        common_enums::ExemptionIndicator::SecureCorporatePayment
        | common_enums::ExemptionIndicator::ThreeDsOutage
        | common_enums::ExemptionIndicator::OutOfScaScope => {
            Some(NuveiExemptionRequestReason::AccountVerification)
        }
        _ => None,
    }
}

// Response Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let is_zero_amount =
            router_data.request.minor_amount == common_utils::types::MinorUnit::zero();

        // The whole body, verbatim as it was parsed, so the GSM error-code update and
        // any later diagnosis can read what Nuvei actually sent.
        let raw_connector_response = nuvei_raw_response(response);

        // A 2xx is never success on its own: the envelope can say SUCCESS while the
        // gateway declined. Both shapes return Err(ErrorResponse).
        let is_envelope_error = matches!(response.status, NuveiPaymentStatus::Error);
        let is_gateway_failure = matches!(
            response.transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        );

        if is_envelope_error || is_gateway_failure {
            let error_fields = nuvei_error_fields(
                &response.status,
                response.transaction_status.as_ref(),
                response.err_code,
                response.reason.as_deref(),
                &response.gateway_error,
            );
            let status = nuvei_attempt_status(
                response.transaction_status.as_ref(),
                response.transaction_type.as_ref(),
                is_zero_amount,
                &response.status,
            );

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    connector_response: nuvei_connector_response_data(
                        response.payment_option.as_ref(),
                        response.auth_code.as_deref(),
                    ),
                    raw_connector_response: raw_connector_response.clone(),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let status = nuvei_attempt_status(
            response.transaction_status.as_ref(),
            response.transaction_type.as_ref(),
            is_zero_amount,
            &response.status,
        );

        // One reference id per payment: `transactionId` is the only value that is a
        // valid `relatedTransactionId` for settle / void / refund. `orderId` is *not* a
        // substitute — falling back to it produces two different ids for one payment.
        let connector_transaction_id = response
            .transaction_id
            .clone()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                tracing::error!(
                    connector = "nuvei",
                    "nuvei: /payment.do response carried no transactionId; refusing to fall back to orderId"
                );
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some(
                        "Nuvei's /payment.do response carried no transactionId; the payment cannot be settled, voided or refunded without it"
                            .to_string(),
                    ),
                ))
            })?;

        let redirection_data = nuvei_redirect_form(response.payment_option.as_ref())?;

        if response.order_id.as_deref().is_some_and(str::is_empty) {
            tracing::debug!(
                connector = "nuvei",
                "nuvei: response carried an empty orderId; connector_response_reference_id is left unset"
            );
        }

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data,
            mandate_reference: response
                .payment_option
                .as_ref()
                .and_then(|option| option.user_payment_option_id.clone())
                .filter(|id| !id.is_empty())
                .map(|connector_mandate_id| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(connector_mandate_id),
                        payment_method_id: None,
                        mandate_metadata: None,
                        connector_mandate_request_reference_id: None,
                    })
                }),
            connector_metadata: None,
            network_txn_id: nuvei_network_txn_id(
                response.external_scheme_transaction_id.as_deref(),
            ),
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|id| !id.is_empty()),
            connector_response_reference_id: response.order_id.clone().filter(|id| !id.is_empty()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: nuvei_connector_response_data(
                    response.payment_option.as_ref(),
                    response.auth_code.as_deref(),
                ),
                raw_connector_response,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

/// Serialises a parsed Nuvei response back to JSON for
/// `PaymentFlowData.raw_connector_response`. The transformers only receive the parsed
/// struct, so this is the verbatim content of the body as the connector understood it.
fn nuvei_raw_response<R: Serialize>(response: &R) -> Option<Secret<String>> {
    match serde_json::to_string(response) {
        Ok(body) => Some(Secret::new(body)),
        Err(error) => {
            tracing::warn!(
                connector = "nuvei",
                serialization_error = ?error,
                "nuvei: could not render the response body for raw_connector_response"
            );
            None
        }
    }
}

/// Builds the redirection form for the two shapes Nuvei documents
/// (spec "### Redirection"):
///
/// - card 3DS challenge: **POST** to `paymentOption.card.threeD.acsUrl` with the single
///   form field `creq` = `cReq`;
/// - APM / bank redirect: **GET** to `paymentOption.redirectUrl`.
///
/// A URL that does not parse is an error, never silently dropped.
fn nuvei_redirect_form(
    payment_option: Option<&NuveiResponsePaymentOption>,
) -> Result<Option<Box<RedirectForm>>, Report<ConnectorError>> {
    let Some(payment_option) = payment_option else {
        return Ok(None);
    };

    let three_d = payment_option
        .card
        .as_ref()
        .and_then(|card| card.three_d.as_ref());
    if let Some(three_d) = three_d {
        if let (Some(acs_url), Some(c_req)) = (three_d.acs_url.as_ref(), three_d.c_req.as_ref()) {
            let mut form_fields = std::collections::HashMap::new();
            form_fields.insert("creq".to_string(), c_req.clone());
            return Ok(Some(Box::new(RedirectForm::Form {
                endpoint: acs_url.clone(),
                method: Method::Post,
                form_fields,
            })));
        }
    }

    match payment_option.redirect_url.as_ref() {
        Some(redirect_url) if !redirect_url.is_empty() => {
            let url = Url::parse(redirect_url).map_err(|error| {
                tracing::error!(
                    connector = "nuvei",
                    parse_error = ?error,
                    "nuvei: paymentOption.redirectUrl is not a valid URL"
                );
                Report::new(ConnectorError::response_handling_failed_with_context(
                    0,
                    Some("Nuvei returned a redirect URL that is not a valid URL".to_string()),
                ))
            })?;
            Ok(Some(Box::new(RedirectForm::from((url, Method::Get)))))
        }
        _ => Ok(None),
    }
}

/// `addendums.l23processingData.taxIndicator`: `1` when state/provincial tax is
/// included in the order, `2` when the order is not subject to tax. `0` (tax not
/// included) has no domain counterpart and is never emitted.
fn nuvei_l23_tax_indicator(tax_status: common_enums::TaxStatus) -> &'static str {
    match tax_status {
        common_enums::TaxStatus::Taxable => "1",
        common_enums::TaxStatus::Exempt => "2",
    }
}

/// `orderDate` is `String(6)` in `YYMMDD` (spec "### `addendums.l23processingData` —
/// root fields").
fn nuvei_l23_order_date(order_date: &time::PrimitiveDateTime) -> String {
    let date = order_date.date();
    format!(
        "{:02}{:02}{:02}",
        date.year().rem_euclid(100),
        u8::from(date.month()),
        date.day()
    )
}

/// Builds `addendums.l23processingData` from `PaymentFlowData.l2_l3_data`.
///
/// Returns `None` when the merchant sent no Level 2/3 data, or when the data carries
/// none of the members Nuvei accepts here — an empty `addendums` object is never sent.
///
/// Deliberately omitted (UD-22, contract C-04 rejected): `customerCode`,
/// `destinationZip`, `shipFromZip` and `destinationCountryCode` are sourced from a
/// customer and an address that the capture request does not carry. `vatOrTaxRate`,
/// `summaryCommodityCode`, `uniqueVATReference`, `nationalTaxAmount`,
/// `lineItemCountTax` and `taxItems[]` have no counterpart on `L2L3Data` either, and a
/// value is never invented for them.
fn build_nuvei_l23_processing_data(
    l2_l3_data: Option<&domain_types::connector_types::L2L3Data>,
    currency: common_enums::Currency,
    amount_converter: &(dyn common_utils::types::AmountConvertor<Output = StringMajorUnit> + Sync),
) -> Result<Option<NuveiAddendums>, Report<IntegrationError>> {
    let Some(l2_l3_data) = l2_l3_data else {
        return Ok(None);
    };

    let to_major = |amount: common_utils::types::MinorUnit| {
        amount_converter.convert(amount, currency).change_context(
            IntegrationError::AmountConversionFailed {
                context: nuvei_error_context(
                    "Nuvei's Level 2/3 addendum amounts are decimal major units; check the currency sent on the capture matches the authorisation.",
                ),
            },
        )
    };

    let order_info = l2_l3_data.order_info.as_ref();
    let tax_info = l2_l3_data.tax_info.as_ref();

    let tax_indicator = tax_info
        .and_then(|tax_info| tax_info.tax_status)
        .map(|tax_status| nuvei_l23_tax_indicator(tax_status).to_string());
    let merchant_vat_reg_num =
        tax_info.and_then(|tax_info| tax_info.merchant_tax_registration_id.clone());
    let customer_vat_reg_num =
        tax_info.and_then(|tax_info| tax_info.customer_tax_registration_id.clone());
    let order_date = order_info
        .and_then(|order_info| order_info.order_date.as_ref())
        .map(nuvei_l23_order_date);

    let order_details = order_info
        .and_then(|order_info| order_info.order_details.as_ref())
        .filter(|order_details| !order_details.is_empty());

    let items = order_details
        .map(|order_details| {
            order_details
                .iter()
                .map(|item| {
                    Ok::<_, Report<IntegrationError>>(NuveiL23Item {
                        commodity_code: item.commodity_code.clone(),
                        // Nuvei's item `description` is mandatory; the product name is
                        // the only always-present descriptive value.
                        description: item
                            .description
                            .clone()
                            .unwrap_or_else(|| item.product_name.clone()),
                        product_code: item.product_id.clone().or_else(|| item.sku.clone()),
                        quantity: item.quantity.to_string(),
                        unit_measure: item.unit_of_measure.clone(),
                        price: to_major(item.amount)?,
                        vat_or_tax_amount: item.total_tax_amount.map(to_major).transpose()?,
                        vat_or_tax_rate: item.tax_rate.map(|rate| rate.to_string()),
                        total_amount: item.total_amount.map(to_major).transpose()?,
                        discount_rate: item.discount_percentage.map(|rate| rate.to_string()),
                        discount: item.unit_discount_amount.map(to_major).transpose()?,
                        tax_type: item.product_tax_code.clone(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    let line_item_count = items.as_ref().map(|items| items.len().to_string());

    let total_discount = order_info
        .and_then(|order_info| order_info.discount_amount)
        .map(to_major)
        .transpose()?;
    let total_shipping = order_info
        .and_then(|order_info| order_info.shipping_cost)
        .map(to_major)
        .transpose()?;
    let duty_amount = order_info
        .and_then(|order_info| order_info.duty_amount)
        .map(to_major)
        .transpose()?;
    let shipping_vat_or_tax_amount = tax_info
        .and_then(|tax_info| tax_info.shipping_amount_tax)
        .map(to_major)
        .transpose()?;
    let tax_amount = tax_info
        .and_then(|tax_info| tax_info.order_tax_amount)
        .map(to_major)
        .transpose()?;

    let has_amount_details = total_discount.is_some()
        || total_shipping.is_some()
        || duty_amount.is_some()
        || shipping_vat_or_tax_amount.is_some()
        || tax_amount.is_some();
    let amount_details = if has_amount_details {
        Some(NuveiL23AmountDetails {
            total_discount,
            total_shipping,
            duty_amount,
            vat_or_tax_amount: shipping_vat_or_tax_amount,
            tax_amount,
        })
    } else {
        None
    };

    if tax_indicator.is_none()
        && merchant_vat_reg_num.is_none()
        && customer_vat_reg_num.is_none()
        && order_date.is_none()
        && items.is_none()
        && amount_details.is_none()
    {
        tracing::debug!(
            connector = "nuvei",
            "nuvei: l2_l3_data carried no member Nuvei accepts on /settleTransaction.do, dropping the addendums block"
        );
        return Ok(None);
    }

    Ok(Some(NuveiAddendums {
        l23_processing_data: NuveiL23ProcessingData {
            tax_indicator,
            merchant_vat_reg_num,
            customer_vat_reg_num,
            order_date,
            line_item_count,
            items,
            amount_details,
        },
    }))
}

// Capture Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NuveiCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        // G-Capture-02: clientUniqueId is String(45) and it participates in the settle
        // checksum, so an over-long merchant reference is refused, never truncated.
        let (client_request_id, client_unique_id) = nuvei_client_ids(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
            &time_stamp,
        )?;

        // G-Capture-04: multi-settle needs a totalSettleCount repeated identically on
        // every settle of the same authorisation, a US merchantId on a TSYS acquirer and
        // a non-Amex card. None of those preconditions is visible from the request, so
        // the flow refuses rather than sending a settle that cannot be reconciled.
        if request.multiple_capture_data.is_some() {
            return Err(IntegrationError::NotSupported {
                message: "Nuvei multi-settle needs totalSettleCount, a US TSYS merchant and a non-Amex card; it is not certified in this run".to_string(),
                connector: "nuvei",
                context: nuvei_error_context(
                    "Send a single settle for the full or partial amount; repeated partial settles require Nuvei's Multi-Settle feature to be enabled on the merchant account first.",
                ),
            }
            .into());
        }

        // G-Capture-01: relatedTransactionId is the Auth transactionId and settle has no
        // alternative key. The gRPC surface types connector_transaction_id as a
        // non-optional proto3 string, so an absent id arrives as
        // ResponseId::ConnectorTransactionId("") and never as NoResponseId: an empty id
        // must be refused here too, or settleTransaction.do goes out with
        // relatedTransactionId:"" and Nuvei answers errCode 1082.
        let related_transaction_id = match &request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) | ResponseId::EncodedData(id)
                if !id.trim().is_empty() =>
            {
                id.clone()
            }
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: nuvei_error_context(
                        "Nuvei's /settleTransaction.do settles by relatedTransactionId, the transactionId returned by the original Auth; capture the payment only after the authorisation returned one.",
                    ),
                }
                .into());
            }
        };

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(request.minor_amount_to_capture, request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Nuvei's settle `amount` is a decimal major-unit string; check the capture amount and currency.",
                ),
            })?;

        let currency = request.currency;

        // G-Capture-03: Level 2/3 addendums qualify only on the Auth -> Settle path. An
        // auto-capture Sale was already settled by /payment.do, which rejects the block.
        let l2_l3_data = router_data.resource_common_data.l2_l3_data.as_deref();
        if l2_l3_data.is_some()
            && matches!(
                request.capture_method,
                Some(common_enums::CaptureMethod::Automatic)
            )
        {
            return Err(IntegrationError::NotSupported {
                message: "Nuvei accepts Level 2/3 addendums on the Auth-Settle path only; an auto-capture Sale cannot carry them".to_string(),
                connector: "nuvei",
                context: nuvei_error_context(
                    "Authorize with capture_method MANUAL and send l2_l3_data on the capture call; Nuvei rejects addendums.l23processingData on /payment.do.",
                ),
            }
            .into());
        }
        let addendums = build_nuvei_l23_processing_data(
            l2_l3_data,
            currency,
            item.connector.amount_converter_webhooks,
        )?;

        // Checksum (spec "## Checksum Generation" row 5): merchantId + merchantSiteId +
        // clientRequestId + clientUniqueId + amount + currency + relatedTransactionId +
        // authCode + comment + timeStamp + merchantSecretKey. `authCode` and `comment`
        // are not sent, so they contribute the empty string and the concatenation keeps
        // the eight-element form below (UD-03).
        //
        // WARNING: if `authCode` is ever added to this request it must **also** be
        // inserted into the concatenation between `relatedTransactionId` and `comment`.
        // Sending the field without adding it here makes every settle fail with
        // `errCode 1001 Invalid checksum`.
        let checksum = auth.generate_checksum(
            "settleTransaction.do / refundTransaction.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("clientUniqueId", &client_unique_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("relatedTransactionId", &related_transaction_id),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            addendums,
            time_stamp,
            checksum,
        })
    }
}

/// `errCode 9146` — "No transaction details returned for the provided ID". A sync that
/// runs before Nuvei has registered the transaction must leave the payment's status
/// untouched instead of failing it terminally (spec "## HTTP Codes and Errors";
/// hyperswitch bypasses the same code at
/// `crates/hyperswitch_connectors/src/connectors/nuvei/transformers.rs:3886-3892`).
const NUVEI_NO_TRANSACTION_FOUND_ERR_CODE: i32 = 9146;

// PSync Response Transformation
impl TryFrom<ResponseRouterData<NuveiSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let transaction_details = response.transaction_details.as_ref();
        let transaction_status =
            transaction_details.and_then(|details| details.transaction_status.as_ref());
        let transaction_type =
            transaction_details.and_then(|details| details.transaction_type.as_ref());

        // An early sync is not a failed payment: Nuvei has simply not registered the
        // transaction yet. Echo the id that was asked about and leave the attempt's
        // status exactly as the caller had it.
        if response.err_code == Some(NUVEI_NO_TRANSACTION_FOUND_ERR_CODE) {
            tracing::info!(
                connector = "nuvei",
                "nuvei: /getTransactionDetails.do reported no transaction for the id yet (errCode 9146); leaving the payment status unchanged"
            );
            let requested_id = match &router_data.request.connector_transaction_id {
                ResponseId::ConnectorTransactionId(id) | ResponseId::EncodedData(id) => {
                    ResponseId::ConnectorTransactionId(id.clone())
                }
                ResponseId::NoResponseId => ResponseId::NoResponseId,
            };
            return Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: requested_id,
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
                ..router_data.clone()
            });
        }

        // INV-11: a 2xx that carries `status == ERROR`, or a transaction whose own
        // `transactionStatus` is DECLINED/ERROR, is a failure and never an `Ok`.
        let is_in_band_failure = matches!(
            transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        );
        if matches!(response.status, NuveiPaymentStatus::Error) || is_in_band_failure {
            let error_fields = nuvei_error_fields(
                &response.status,
                transaction_status,
                response.err_code,
                response.reason.as_deref(),
                // The gateway members live inside `transactionDetails` on this endpoint;
                // fall back to the envelope's copy only when the nested block carried
                // none of them.
                transaction_details
                    .map(|details| &details.gateway_error)
                    .filter(|gateway_error| {
                        gateway_error.gw_error_code.is_some()
                            || gateway_error.gw_error_reason.is_some()
                    })
                    .unwrap_or(&response.gateway_error),
            );
            let raw_connector_response = nuvei_raw_response(response);
            // The failure belongs to whichever leg the synced transaction is: a declined
            // Auth is an AuthorizationFailed, a declined Settle a CaptureFailed, a
            // declined Void a VoidFailed. `nuvei_attempt_status` owns that table; there
            // is no `_ =>` arm and no generic `Failure` default here.
            let status = nuvei_attempt_status(
                transaction_status,
                transaction_type,
                router_data.request.amount == common_utils::types::MinorUnit::zero(),
                &response.status,
            );

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: transaction_details
                        .and_then(|details| details.transaction_id.clone()),
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // G-PSync-02: `/getTransactionDetails.do` nests every status field inside
        // `transactionDetails`. Without that block there is nothing to sync from, and
        // reading the flat `/payment.do` shape would silently report `None` for all of
        // them.
        let transaction_details = transaction_details.ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_details missing in Nuvei PSync response".to_string()),
            ))
        })?;

        // Composite status map, keyed on the transaction's own `transactionType`. An
        // undocumented or absent type reaches `NuveiTransactionStatus::Unknown` /
        // `None` and maps to Pending: an unsettled transaction is never reported as
        // Charged (reviewer pattern "no catch-all to a terminal success status").
        let status = nuvei_attempt_status(
            transaction_details.transaction_status.as_ref(),
            transaction_details.transaction_type.as_ref(),
            router_data.request.amount == common_utils::types::MinorUnit::zero(),
            &response.status,
        );

        // A settled transaction whose `processedAmount` is below the amount being synced
        // is a partial capture, not a full one. `processedAmount` is a decimal
        // major-unit string: it goes back through the amount framework, and a value
        // Nuvei sent that will not parse is an error, never a silent zero.
        let processed_amount =
            transaction_details
                .processed_amount
                .clone()
                .map(|processed_amount| {
                    common_utils::types::AmountConvertor::convert_back(
                        &common_utils::types::StringMajorUnitForConnector,
                        processed_amount,
                        router_data.request.currency,
                    )
                    .change_context(ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some(
                            "transactionDetails.processedAmount is not a decimal major-unit amount"
                                .to_string(),
                        ),
                    ))
                })
                .transpose()?;
        let status = match (status, processed_amount) {
            (common_enums::AttemptStatus::Charged, Some(processed))
                if processed < router_data.request.amount =>
            {
                tracing::info!(
                    connector = "nuvei",
                    "nuvei: processedAmount is below the amount synced; reporting the attempt as partially charged"
                );
                common_enums::AttemptStatus::PartialCharged
            }
            (status, _) => status,
        };

        // P-PSync-03: one identifier per payment. The resource id is Nuvei's
        // `transactionId` and never falls back to an order or merchant reference; the
        // response reference id is the same `orderId` the Authorize response published.
        let connector_transaction_id =
            transaction_details.transaction_id.clone().ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("transaction_id missing in Nuvei PSync transaction_details".to_string()),
                ))
            })?;
        let connector_response_reference_id = response.order_id.clone().filter(|id| !id.is_empty());
        if connector_response_reference_id.is_none() {
            tracing::debug!(
                connector = "nuvei",
                "nuvei: sync response carried no orderId; connector_response_reference_id is left unset rather than filled with the merchant reference"
            );
        }

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: response
                .payment_option
                .as_ref()
                .and_then(|option| option.user_payment_option_id.clone())
                .filter(|id| !id.is_empty())
                .map(|connector_mandate_id| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(connector_mandate_id),
                        payment_method_id: None,
                        mandate_metadata: None,
                        connector_mandate_request_reference_id: None,
                    })
                }),
            connector_metadata: None,
            network_txn_id: nuvei_network_txn_id(
                response.external_scheme_transaction_id.as_deref(),
            ),
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|id| !id.is_empty()),
            connector_response_reference_id,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: nuvei_connector_response_data(
                    response.payment_option.as_ref(),
                    transaction_details.auth_code.as_deref(),
                ),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
// Capture Response Transformation
impl TryFrom<ResponseRouterData<NuveiCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // A 2xx carrying `status == ERROR`, or a Settle whose transactionStatus is
        // DECLINED/ERROR, is a failed capture and never an Ok.
        let is_in_band_failure = matches!(
            response.transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        );
        if matches!(response.status, NuveiPaymentStatus::Error) || is_in_band_failure {
            let error_fields = nuvei_error_fields(
                &response.status,
                response.transaction_status.as_ref(),
                response.err_code,
                response.reason.as_deref(),
                &response.gateway_error,
            );
            let raw_connector_response = nuvei_raw_response(response);
            // A failure on the settle leg is a CaptureFailed, not the generic payment
            // Failure: the authorisation itself is still intact.
            let status = nuvei_attempt_status(
                response.transaction_status.as_ref(),
                response
                    .transaction_type
                    .as_ref()
                    .or(Some(&NuveiTransactionType::Settle)),
                false,
                &response.status,
            );

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Map transaction status to attempt status, keyed on the settle's own
        // transactionType. Nuvei echoes `Settle`; assume it when the member is absent so
        // a successful settle is never read as an authorisation.
        let status = nuvei_attempt_status(
            response.transaction_status.as_ref(),
            response
                .transaction_type
                .as_ref()
                .or(Some(&NuveiTransactionType::Settle)),
            false,
            &response.status,
        );

        // A settle for less than the amount held leaves the payment partially charged.
        // The authorised amount is only known when the caller carried it through; when
        // it is absent the settle keeps the status the map produced.
        let authorized_amount = router_data
            .resource_common_data
            .minor_amount_authorized
            .or_else(|| {
                router_data
                    .resource_common_data
                    .amount
                    .as_ref()
                    .map(|amount| amount.amount)
            });
        let status = match (status, authorized_amount) {
            (common_enums::AttemptStatus::Charged, Some(authorized))
                if router_data.request.minor_amount_to_capture < authorized =>
            {
                common_enums::AttemptStatus::PartialCharged
            }
            (status, _) => status,
        };

        // Get connector transaction ID
        let connector_transaction_id = response.transaction_id.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in Nuvei capture response".to_string()),
            ))
        })?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Refund Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for NuveiRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        // G-Refund-02 / P-foundation-13: `clientUniqueId` is `String(45)` and participates
        // in the refund checksum, so an over-long merchant reference is refused rather
        // than truncated; `clientRequestId` is made unique to this HTTP call so a second
        // partial refund on the same reference cannot collide with errCode 1089. Nuvei
        // allows several partial refunds against one settled transaction, so this split
        // is what makes partial refunds retryable at all.
        let (client_request_id, client_unique_id) = nuvei_client_ids(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
            &time_stamp,
        )?;

        // G-Refund-01: `relatedTransactionId` is the only way to name the settled
        // transaction being refunded, so an absent connector_transaction_id is refused
        // before the call rather than sent as an empty string.
        let related_transaction_id = router_data.request.connector_transaction_id.clone();
        if related_transaction_id.trim().is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID {
                context: nuvei_error_context(
                    "Supply the connector_transaction_id of the settled transaction to refund; Nuvei identifies it through relatedTransactionId on /refundTransaction.do. An unsettled transaction must be cancelled through /voidTransaction.do instead.",
                ),
            }
            .into());
        }

        let currency = router_data.request.currency;

        // G-Refund-03: Nuvei requires the refund currency to equal the currency of the
        // original transaction. `RefundsData` carries a single `currency` for both the
        // original payment amount and the refund amount, so the only independently
        // supplied refund currency in the request is the integrity object's; a
        // disagreement between the two means the caller is asking for a cross-currency
        // refund, which Nuvei rejects.
        if let Some(integrity) = router_data.request.integrity_object.as_ref() {
            if integrity.currency != currency {
                return Err(IntegrationError::NotSupported {
                    message: "Nuvei requires the refund currency to match the original transaction"
                        .to_string(),
                    connector: "nuvei",
                    context: nuvei_error_context(
                        "Issue the refund in the currency of the original transaction; Nuvei does not convert a refund into another currency.",
                    ),
                }
                .into());
            }
        }

        // Convert amount using the connector's amount converter. Partial refunds are
        // supported: Nuvei accepts several partial refunds against one settled
        // transaction, provided their sum does not exceed the original sale amount.
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                common_utils::types::MinorUnit::new(router_data.request.refund_amount),
                currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Check that the refund amount and currency form a valid major-unit amount; Nuvei's /refundTransaction.do amount is a decimal major-unit string.",
                ),
            })?;

        // Checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId +
        // amount + currency + relatedTransactionId + timeStamp + merchantSecretKey.
        //
        // Spec gaps G-03/G-05: Nuvei does not publish the concatenation order for
        // /refundTransaction.do. The order below is the one the existing working
        // integration and the hyperswitch reference both use; it is kept verbatim rather
        // than guessed at, and is marked verify_live — a wrong order fails every refund
        // with errCode 1001 "Invalid checksum".
        let checksum = auth.generate_checksum(
            "settleTransaction.do / refundTransaction.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("clientUniqueId", &client_unique_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("relatedTransactionId", &related_transaction_id),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Refund Sync Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for NuveiRefundSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();

        // P-RSync-04 / P-foundation-13: `clientUniqueId` is the merchant reference,
        // validated to String(45) and never truncated; `clientRequestId` is unique to a
        // single HTTP call. `/getTransactionDetails.do` takes no `clientRequestId`
        // (UD-02), so it is derived and deliberately dropped rather than aliased onto
        // `clientUniqueId`.
        let (_client_request_id, client_unique_id) = nuvei_client_ids(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
            &time_stamp,
        )?;

        // The id sent here is the **refund's own** `transactionId` — the one
        // `/refundTransaction.do` returned — and never the original payment's
        // (`request.connector_transaction_id`). Syncing the payment's id would return the
        // Sale/Auth/Settle leg and report the *payment's* state as the refund's
        // (spec "### Flow: RSync").
        let transaction_id = router_data.request.connector_refund_id.clone();

        // G-RSync-01: without the refund's own transactionId there is no reliable lookup
        // key. Whether REST 1.0 accepts `clientUniqueId` alone is unconfirmed (spec
        // "### Flow: RSync"; Gaps G-09), so it is never used as a substitute here.
        if transaction_id.trim().is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID {
                context: nuvei_error_context(
                    "Nuvei's /getTransactionDetails.do is keyed by the refund's own transactionId, returned by /refundTransaction.do; sync the refund only once that id is known.",
                ),
            }
            .into());
        }

        // Generate checksum for getTransactionDetails: merchantId + merchantSiteId + transactionId + clientUniqueId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(
            "getTransactionDetails.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("transactionId", &transaction_id),
                ("clientUniqueId", &client_unique_id),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_unique_id,
            transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Refund Response Transformation
impl TryFrom<ResponseRouterData<NuveiRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // INV-11: HTTP 200 is never success on its own. `status == ERROR`, and equally an
        // accepted envelope whose `transactionStatus` is DECLINED or ERROR, is a refused
        // refund and returns Err(ErrorResponse) — the same shape the Capture and Void
        // units found on their own sites, where a 2xx carrying a DECLINED/ERROR
        // transactionStatus was reported as Ok with a merely failed status.
        let is_in_band_failure = matches!(
            response.transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        );
        if matches!(response.status, NuveiPaymentStatus::Error) || is_in_band_failure {
            // P-Refund-03: resolve the error through the shared three-stage resolver so
            // the gateway stage's gwErrorCode/gwErrorReason and the issuer's decline
            // code reach the network_* fields the GSM smart retry reads.
            let error_fields = nuvei_error_fields(
                &response.status,
                response.transaction_status.as_ref(),
                response.err_code,
                response.reason.as_deref(),
                &response.gateway_error,
            );
            let raw_connector_response = nuvei_raw_response(response);

            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    // INV-08: the flow-correct failure status for a refund.
                    attempt_status: Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // P-Refund-01: map the refund status with no `_ =>` arm (INV-09). The money leaves
        // the merchant here, so nothing but an explicit APPROVED is reported as a
        // completed refund: an APM or otherwise asynchronous provider answers
        // /refundTransaction.do with status=PENDING and **no** transactionStatus at all,
        // and the terminal outcome arrives only later on a DMN carrying
        // transactionType=Credit, which can take days. An undocumented value deserialises
        // to `Unknown`. Both stay Pending, where the previous catch-all reported Success.
        let refund_status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            // Unreachable after the INV-11 branch above; kept so the match stays
            // exhaustive without a catch-all.
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::RefundStatus::Failure
            }
            Some(NuveiTransactionStatus::Redirect)
            | Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing) => common_enums::RefundStatus::Pending,
            Some(NuveiTransactionStatus::Unknown) => {
                tracing::warn!(
                    connector = "nuvei",
                    flow = "Refund",
                    "nuvei: undocumented transactionStatus on a refund; keeping the refund pending"
                );
                common_enums::RefundStatus::Pending
            }
            None => {
                tracing::info!(
                    connector = "nuvei",
                    flow = "Refund",
                    "nuvei: refund accepted with no transactionStatus (asynchronous provider); the terminal outcome arrives on the Credit DMN"
                );
                common_enums::RefundStatus::Pending
            }
        };

        // P-Refund-02: an asynchronous refund legitimately carries no transactionId yet.
        // That is a pending refund, not a terminal transport failure, so it is only a
        // hard error when the refund has already reached a terminal status. The fallback
        // is the `clientUniqueId` this refund was sent with, which is the other key
        // /getTransactionDetails.do accepts and which the RSync request already sends
        // alongside the transaction id.
        let connector_refund_id = match response.transaction_id.clone() {
            Some(transaction_id) => transaction_id,
            None if refund_status == common_enums::RefundStatus::Pending => {
                let client_unique_id = router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone();
                tracing::warn!(
                    connector = "nuvei",
                    flow = "Refund",
                    "nuvei: pending refund carried no transactionId; falling back to clientUniqueId until the Credit DMN supplies it"
                );
                client_unique_id
            }
            None => {
                return Err(Report::new(
                    ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("transaction_id missing in Nuvei refund response".to_string()),
                    ),
                ))
            }
        };

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
            acquirer_reference_number: None,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// Refund Sync Response Transformation
impl TryFrom<ResponseRouterData<NuveiRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        // P-RSync-01: every status member of this endpoint lives under
        // `transactionDetails`, so nothing is read off the root but the envelope's own
        // `status` / `errCode` / `reason`.
        let transaction_details = response.transaction_details.as_ref();
        let transaction_status =
            transaction_details.and_then(|details| details.transaction_status.as_ref());
        let transaction_type =
            transaction_details.and_then(|details| details.transaction_type.as_ref());

        // The refund this sync was asked about. A sync reports on an existing refund; it
        // never invents a new identifier for one, so this is the value every
        // non-terminal branch below publishes unchanged.
        let synced_refund_id = router_data.request.connector_refund_id.clone();

        // P-RSync-03: an RSync that runs before Nuvei has registered the refund answers
        // with errCode 9146 ("no transaction details for the provided ID"). That is an
        // early poll, not a refused refund: the refund's status is left exactly as the
        // caller had it and the money is neither reported as returned nor as lost.
        if response.err_code == Some(NUVEI_NO_TRANSACTION_FOUND_ERR_CODE) {
            tracing::info!(
                connector = "nuvei",
                flow = "RSync",
                "nuvei: /getTransactionDetails.do reported no transaction for the refund id yet (errCode 9146); leaving the refund status unchanged"
            );
            return Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: synced_refund_id,
                    refund_status: router_data.request.refund_status,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
                ..router_data.clone()
            });
        }

        // INV-11: HTTP 200 is never success on its own. `status == ERROR`, and equally an
        // accepted envelope whose `transactionStatus` is DECLINED or ERROR, returns
        // Err(ErrorResponse) rather than an Ok carrying a merely failed status — the same
        // shape the Capture, Void, RepeatPayment, Refund and PSync units corrected on
        // their own sites. It is also the only way a *reversed* refund can be seen:
        // Nuvei publishes no REVERSED/ROLLBACK status, so a reversal surfaces here as
        // DECLINED/ERROR carrying `gwErrorCode`/`gwErrorReason` (spec "### Flow: RSync").
        let is_in_band_failure = matches!(
            transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        );
        if matches!(response.status, NuveiPaymentStatus::Error) || is_in_band_failure {
            let error_fields = nuvei_error_fields(
                &response.status,
                transaction_status,
                response.err_code,
                response.reason.as_deref(),
                // The gateway members live inside `transactionDetails` on this endpoint;
                // the envelope's copy is the fallback for a stage-1 refusal that never
                // reached a transaction.
                transaction_details
                    .map(|details| &details.gateway_error)
                    .filter(|gateway_error| {
                        gateway_error.gw_error_code.is_some()
                            || gateway_error.gw_error_reason.is_some()
                    })
                    .unwrap_or(&response.gateway_error),
            );
            let raw_connector_response = nuvei_raw_response(response);

            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    // INV-08: the flow-correct failure status for a refund sync.
                    attempt_status: Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                    connector_transaction_id: transaction_details
                        .and_then(|details| details.transaction_id.clone()),
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // G-RSync-02: an accepted envelope with no `transactionDetails` block carries
        // nothing to sync from. It is a non-terminal sync result, never a terminal
        // failure: an asynchronous (APM) refund answers with status = PENDING and no
        // transaction at all, and its outcome arrives only on a later Credit DMN.
        let Some(transaction_details) = transaction_details else {
            tracing::info!(
                connector = "nuvei",
                flow = "RSync",
                "nuvei: sync response carried no transactionDetails block; keeping the refund pending until the Credit DMN resolves it"
            );
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Pending,
                    ..router_data.resource_common_data.clone()
                },
                response: Ok(RefundsResponseData {
                    connector_refund_id: synced_refund_id,
                    refund_status: common_enums::RefundStatus::Pending,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
                ..router_data.clone()
            });
        };

        // P-RSync-02: exhaustive over all seven NuveiTransactionStatus variants plus
        // `None`, with no `_ =>` arm (INV-09). The catch-all this replaces mapped an
        // absent or undocumented status to RefundStatus::Success — money reported as
        // returned that Nuvei never confirmed. Nothing but an explicit APPROVED is
        // terminal here; an undocumented value deserialises to `Unknown`, and an
        // asynchronous refund legitimately carries no status at all (UD-16). Both stay
        // Pending, and neither is a failure.
        let refund_status = match transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            // Unreachable after the INV-11 branch above; kept so the match stays
            // exhaustive without a catch-all.
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::RefundStatus::Failure
            }
            Some(NuveiTransactionStatus::Redirect)
            | Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing) => common_enums::RefundStatus::Pending,
            Some(NuveiTransactionStatus::Unknown) => {
                tracing::warn!(
                    connector = "nuvei",
                    flow = "RSync",
                    "nuvei: undocumented transactionStatus on a refund sync; keeping the refund pending"
                );
                common_enums::RefundStatus::Pending
            }
            None => {
                tracing::info!(
                    connector = "nuvei",
                    flow = "RSync",
                    "nuvei: refund sync carried no transactionStatus (asynchronous provider); the terminal outcome arrives on the Credit DMN"
                );
                common_enums::RefundStatus::Pending
            }
        };

        // A refund is the `Credit` leg of the order. If the object that came back is a
        // different leg — a `Sale`, `Auth`, `Settle` or `Void` — then the id that was
        // looked up was not the refund's, and an APPROVED payment must never be
        // published as a completed refund. The refund stays pending instead; an absent
        // `transactionType` is not second-guessed, because G-RSync-01 already fixed the
        // lookup key to the refund's own transactionId.
        let refund_status = match (refund_status, transaction_type) {
            (common_enums::RefundStatus::Success, Some(other))
                if *other != NuveiTransactionType::Credit =>
            {
                tracing::warn!(
                    connector = "nuvei",
                    flow = "RSync",
                    transaction_type = ?other,
                    "nuvei: sync returned an approved transaction that is not the refund's Credit leg; refusing to report the refund as completed"
                );
                common_enums::RefundStatus::Pending
            }
            (refund_status, _) => refund_status,
        };

        // Nuvei's own id for the Credit transaction, when it identified one. It is
        // adopted only when the object really is this refund — the Credit leg, or the
        // very id that was synced (the Refund flow falls back to the clientUniqueId when
        // an asynchronous refund had no transactionId yet, so a first successful sync
        // legitimately upgrades it). Anything else leaves the published id untouched.
        let connector_refund_id = match transaction_details.transaction_id.clone() {
            Some(returned_id)
                if !returned_id.is_empty()
                    && (returned_id == synced_refund_id
                        || transaction_type == Some(&NuveiTransactionType::Credit)) =>
            {
                returned_id
            }
            _ => synced_refund_id,
        };

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
            acquirer_reference_number: None,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// Void Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NuveiVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        // G-Void-03: `clientUniqueId` is `String(45)` and participates in the void
        // checksum, so an over-long merchant reference is refused rather than truncated;
        // `clientRequestId` is made unique to this HTTP call so a retried void cannot
        // collide with the authorisation that shares the reference (P-foundation-13).
        let (client_request_id, client_unique_id) = nuvei_client_ids(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
            &time_stamp,
        )?;

        // G-Void-02: `relatedTransactionId` is the only way to name the transaction being
        // voided, so an absent connector_transaction_id is refused before the call.
        let related_transaction_id = router_data.request.connector_transaction_id.clone();
        if related_transaction_id.trim().is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID {
                context: nuvei_error_context(
                    "Supply the connector_transaction_id of the transaction to cancel; Nuvei identifies it through relatedTransactionId on /voidTransaction.do.",
                ),
            }
            .into());
        }

        // G-Void-01: `amount` and `currency` are optional in Nuvei's reference but are
        // sent here for parity with the hyperswitch integration, and Nuvei requires them
        // to match the original transaction exactly when they are sent. UD-19: the values
        // are forwarded exactly as the caller supplied them and no bounds check is
        // performed — the connector does not hold the original amount at void time.
        let minor_amount =
            router_data
                .request
                .amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: nuvei_error_context(
                        "Send the void amount; Nuvei requires it to equal the amount of the transaction being voided, and a partial void is not supported.",
                    ),
                })?;

        let currency =
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: nuvei_error_context(
                        "Send the void currency; Nuvei requires it to equal the currency of the transaction being voided.",
                    ),
                })?;

        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // Checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId +
        // amount + currency + relatedTransactionId + authCode + comment + timeStamp +
        // merchantSecretKey.
        //
        // UD-13 (spec gap G-05): `authCode` is deliberately **not** sent. The Void page's
        // prose calls it required, but that same page's own request example omits it and
        // the hyperswitch reference omits it and is reported working. Sending it would
        // also change what is hashed here. The gap is unresolved in Nuvei's
        // documentation, so the decision is recorded rather than guessed away: the two
        // empty strings below hold the authCode and comment positions of the ten-element
        // void checksum, which is what an omitted member contributes.
        let checksum = auth.generate_checksum(
            "voidTransaction.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("clientUniqueId", &client_unique_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("relatedTransactionId", &related_transaction_id),
                ("authCode", ""),
                ("comment", ""),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Void Response Transformation
impl TryFrom<ResponseRouterData<NuveiVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // INV-11: HTTP 200 is never success on its own. `status == ERROR`, and equally an
        // accepted envelope whose `transactionStatus` is DECLINED or ERROR, is a refused
        // void and returns Err(ErrorResponse) — the shape the live baseline hit, where a
        // 2xx carrying transactionStatus ERROR was reported as Ok.
        let is_in_band_failure = matches!(
            response.transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        );
        if matches!(response.status, NuveiPaymentStatus::Error) || is_in_band_failure {
            let error_fields = nuvei_error_fields(
                &response.status,
                response.transaction_status.as_ref(),
                response.err_code,
                response.reason.as_deref(),
                &response.gateway_error,
            );
            let raw_connector_response = nuvei_raw_response(response);
            // Flow-aware failure status (INV-08): a refused cancellation is VoidFailed,
            // never the generic payment Failure.
            let status = nuvei_void_attempt_status(response);

            // Nuvei answers a not-voidable transaction with one of the documented refusal
            // codes, on either the envelope (`errCode`) or the gateway
            // (`gwErrorCode`) stage. Name it, and carry the code into
            // `network_decline_code` when the gateway stage supplied none, so the GSM
            // smart retry can tell "this transaction can never be voided" apart from a
            // transient failure.
            let refusal = response
                .err_code
                .and_then(|code| nuvei_void_refusal(code).map(|reason| (code, reason)))
                .or_else(|| {
                    response
                        .gateway_error
                        .gw_error_code
                        .and_then(|code| nuvei_void_refusal(code).map(|reason| (code, reason)))
                });
            let (reason, network_decline_code) = match refusal {
                Some((code, documented_reason)) => {
                    tracing::info!(
                        connector = "nuvei",
                        nuvei_void_refusal_code = code,
                        "nuvei: void refused, the related transaction is not voidable in its current state"
                    );
                    (
                        Some(format!(
                            "Nuvei refused the void (code {code}): {documented_reason}"
                        )),
                        error_fields
                            .network_decline_code
                            .or_else(|| Some(code.to_string())),
                    )
                }
                None => (error_fields.reason, error_fields.network_decline_code),
            };

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Non-failure statuses through the shared composite map: an approved void is
        // Voided, REDIRECT/PENDING/PROCESSING and an undocumented or absent
        // transactionStatus stay non-terminal rather than being read as a completed void.
        let status = nuvei_void_attempt_status(response);

        // Get connector transaction ID
        let connector_transaction_id = response.transaction_id.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in Nuvei void response".to_string()),
            ))
        })?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Nuvei session token for client-side SDK initialization.
/// Uses the same /getSessionToken.do endpoint as ServerSessionAuthenticationToken
/// but returns the response in the ClientAuthenticationToken format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiClientAuthRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

/// Nuvei session token response for ClientAuthenticationToken flow.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiClientAuthResponse {
    pub session_token: Option<Secret<String>>,
    pub internal_request_id: Option<i64>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub version: Option<String>,
    pub client_request_id: Option<String>,
}

// ClientAuthenticationToken Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiClientAuthRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Generate checksum for getSessionToken: merchantId + merchantSiteId + clientRequestId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(
            "getSessionToken.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            time_stamp,
            checksum,
        })
    }
}

// ClientAuthenticationToken Response Transformation
impl TryFrom<ResponseRouterData<NuveiClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Check if the overall request status is ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_fields = nuvei_error_fields(
                &response.status,
                None,
                response.err_code,
                response.reason.as_deref(),
                &NuveiGatewayError::default(),
            );
            let raw_connector_response = nuvei_raw_response(response);

            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            });
        }

        // Extract session token
        let session_token = response.session_token.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("session_token missing in Nuvei response".to_string()),
            ))
        })?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Nuvei(
                NuveiClientAuthenticationResponseDomain { session_token },
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
// OpenOrder (CreateOrder) Request/Response Types
// ============================================================================

/// OpenOrder request — creates a Nuvei order session and returns a sessionToken + orderId.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiOpenOrderRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub client_request_id: String,
    pub currency: common_enums::Currency,
    pub amount: StringMajorUnit,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<NuveiTransactionType>,
}

/// OpenOrder response — returns sessionToken and orderId for subsequent payment flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiOpenOrderResponse {
    pub session_token: Option<Secret<String>>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub order_id: Option<String>,
    pub client_unique_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub version: Option<String>,
    pub client_request_id: Option<String>,
}

/// Nuvei's `openOrder.do` returns `orderId` as a bare JSON integer despite docs
/// declaring it as String(20). Mirrors the Bambora `str_or_i32` pattern.
fn str_or_i64<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StrOrI64 {
        Str(String),
        I64(i64),
    }

    Ok(
        Option::<StrOrI64>::deserialize(deserializer)?.map(|v| match v {
            StrOrI64::Str(s) => s,
            StrOrI64::I64(n) => n.to_string(),
        }),
    )
}

// --- TryFrom: RouterDataV2 -> NuveiOpenOrderRequest (via macro wrapper) ---

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for NuveiOpenOrderRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
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

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let client_unique_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let currency = router_data.request.currency;

        // Generate checksum for openOrder: merchantId + merchantSiteId + clientRequestId + amount + currency + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(
            "payment.do / openOrder.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_unique_id,
            client_request_id,
            currency,
            amount,
            time_stamp,
            checksum,
            transaction_type: Some(NuveiTransactionType::Auth),
        })
    }
}

// --- TryFrom: NuveiOpenOrderResponse -> PaymentCreateOrderResponse ---

impl TryFrom<NuveiOpenOrderResponse> for PaymentCreateOrderResponse {
    type Error = Report<ConnectorError>;

    fn try_from(response: NuveiOpenOrderResponse) -> Result<Self, Self::Error> {
        let connector_order_id = response.order_id.unwrap_or_default();
        Ok(Self {
            connector_order_id,
            session_data: None,
        })
    }
}

// --- TryFrom: ResponseRouterData -> RouterDataV2 (CreateOrder response handler) ---

impl TryFrom<ResponseRouterData<NuveiOpenOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiOpenOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        // Check if the request status is ERROR
        if matches!(
            response.status,
            NuveiPaymentStatus::Error | NuveiPaymentStatus::Failed
        ) {
            let error_fields = nuvei_error_fields(
                &response.status,
                None,
                response.err_code,
                response.reason.as_deref(),
                &NuveiGatewayError::default(),
            );
            let raw_connector_response = nuvei_raw_response(&response);

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            });
        }

        let order_response = PaymentCreateOrderResponse::try_from(response.clone())?;

        // Extract order_id to store for Authorize flow
        let order_id = order_response.connector_order_id.clone();

        // Store session_token in session_token field for use by Authorize flow
        let session_token = response.session_token.clone();

        Ok(Self {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::Pending,
                reference_id: Some(order_id.clone()),
                connector_order_id: Some(order_id),
                // Store session_token for use by subsequent payment flows; the
                // PaymentFlowData carrier is a plain String, so the value is
                // de-masked only at this domain boundary.
                session_token: session_token.map(|token| token.peek().to_string()),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== SetupMandate (SetupRecurring) flow =====
//
// Nuvei SetupRecurring is `/payment.do` preceded by `/getSessionToken.do`; there is no
// dedicated endpoint. Nuvei documents **two** mechanisms for establishing a credential
// on file and publishes no example that combines them (spec Gaps G-07, plan UD-12):
//
// 1. *zero-amount verification* — `amount` `"0"` + `transactionType` `Auth` +
//    `authenticationOnlyType`, and **no** `isRebilling`. This is the `$0 auth`;
//    scheme rules make a full 3DS challenge mandatory for it, and it must not share a
//    session with a deposit.
// 2. *initial CIT registration* — `isRebilling` `"0"` with a **non-zero** amount plus
//    `paymentOption.card.threeD.v2AdditionalParams.rebillExpiry` / `rebillFrequency`,
//    and **no** `authenticationOnlyType`.
//
// The transformer branches on the amount and never mixes the two. Both mechanisms are
// expected to return `paymentOption.userPaymentOptionId` (the UPO token, bound to the
// merchant-supplied `userTokenId`) and `transactionId`, which the later MIT sends as
// `relatedTransactionId`. No published example shows a `userPaymentOptionId` on an
// `amount` `"0"` response, so G-SetupMandate-03 makes its absence observable instead of
// letting a mandate-less "success" through to RepeatPayment.

/// `authenticationOnlyType` — the five values Nuvei documents for a zero-amount
/// verification (`amount` `"0"` + `transactionType` `Auth`).
///
/// Only `AccountVerification` and `Recurring` are ever constructed here: the remaining
/// three describe card-management intents this flow does not express.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiAuthenticationOnlyType {
    /// Register a new card without charging it.
    AddCard,
    /// The credential on file will be used for a recurring series.
    Recurring,
    /// The credential on file will be used for an installment series.
    Installments,
    /// Refresh an already stored card.
    MaintainCard,
    /// Plain account / card verification.
    AccountVerification,
}

/// SetupMandate request — the `/payment.do` body for either mechanism above.
///
/// `isRebilling` and `authenticationOnlyType` are mutually exclusive by construction:
/// the transformer sets exactly one of them, and `skip_serializing_none` keeps the
/// other off the wire entirely.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Option<Secret<String>>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub client_unique_id: Option<String>,
    /// `userTokenId` is what Nuvei binds the returned `userPaymentOptionId` to, so the
    /// MIT can resolve the stored card later. It is mandatory here (G-SetupMandate-02).
    pub user_token_id: Option<Secret<String>>,
    pub payment_option: NuveiPaymentOption<T>,
    /// Mechanism 2 only: `"0"` marks the initial CIT of a recurring series. Never sent
    /// together with `authenticationOnlyType`.
    pub is_rebilling: Option<String>,
    /// Mechanism 1 only: the zero-amount verification intent. Never sent together with
    /// `isRebilling`.
    pub authentication_only_type: Option<NuveiAuthenticationOnlyType>,
    pub transaction_type: NuveiTransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    /// Root-level `shippingAddress`; omitted entirely when the caller sent none.
    pub shipping_address: Option<NuveiShippingAddress>,
    /// Root-level `dynamicDescriptor`; Nuvei rejects an empty object on some accounts,
    /// so it is emitted only when populated.
    pub dynamic_descriptor: Option<NuveiDynamicDescriptor>,
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

/// SetupMandate response - reuses NuveiPaymentResponse fields plus paymentOption
/// (which carries the userPaymentOptionId returned by Nuvei for future MIT calls).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSetupMandateResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
    pub auth_code: Option<String>,
    pub session_token: Option<Secret<String>>,
    pub external_scheme_transaction_id: Option<String>,
    pub transaction_link_id: Option<String>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub payment_option: Option<NuveiResponsePaymentOption>,
}

/// `rebillExpiry` is sent as `today + 5 years` in `YYYYMMDD` (mechanism 2 only).
const NUVEI_REBILL_EXPIRY_DAYS: i64 = 5 * 365;

/// `rebillFrequency` is a number of days; `"0"` means "no fixed schedule", which is what
/// an open-ended credential on file expresses.
const NUVEI_REBILL_FREQUENCY_DAYS: &str = "0";

/// `paymentOption.card.threeD.v2AdditionalParams` for mechanism 2: Nuvei requires
/// `rebillExpiry` + `rebillFrequency` whenever `isRebilling == "0"` is sent.
fn nuvei_rebill_v2_additional_params() -> NuveiV2AdditionalParams {
    let expiry = common_utils::date_time::now()
        .date()
        .saturating_add(time::Duration::days(NUVEI_REBILL_EXPIRY_DAYS));
    NuveiV2AdditionalParams {
        challenge_window_size: None,
        rebill_expiry: Some(format!(
            "{:04}{:02}{:02}",
            expiry.year(),
            u8::from(expiry.month()),
            expiry.day()
        )),
        rebill_frequency: Some(NUVEI_REBILL_FREQUENCY_DAYS.to_string()),
    }
}

// Build the SetupMandate request from the router data.
//
// Mechanism is decided by the amount alone (plan UD-12): a zero `minor_amount` sends the
// zero-amount verification, any other amount sends the initial CIT registration. The two
// are never combined — Nuvei documents them separately and publishes no combined example.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiSetupMandateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
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
        let resource_data = &router_data.resource_common_data;
        let request = &router_data.request;

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // G-SetupMandate-01: `/payment.do` is unreachable without a sessionToken, so the
        // ServerSessionAuthenticationToken pre-step is mandatory on SetupRecurring too.
        let session_token = resource_data
            .session_token
            .clone()
            .filter(|token| !token.trim().is_empty())
            .map(Secret::new)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: nuvei_error_context(
                    "Run MerchantAuthenticationService/CreateServerSessionAuthenticationToken first and thread its sessionToken into SetupRecurring; /payment.do rejects a call without one.",
                ),
            })?;

        // G-SetupMandate-04: billingAddress.email and billingAddress.country are the two
        // members Nuvei enforces, exactly as on Authorize.
        let email = resource_data
            .get_optional_billing_email()
            .or_else(|| request.email.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing_address.email",
                context: nuvei_error_context(
                    "Send billing_address.email (or the request-level email); Nuvei enforces billingAddress.email on /payment.do.",
                ),
            })?;
        let country = resource_data.get_optional_billing_country().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "billing_address.country",
                context: nuvei_error_context(
                    "Send billing_address.country as an ISO-3166-1 alpha-2 code; Nuvei rejects a missing or invalid country with errCode 1014.",
                ),
            },
        )?;

        // G-SetupMandate-02: without a userTokenId Nuvei has nothing to bind the returned
        // userPaymentOptionId to, so the whole point of the setup call is lost.
        let user_token_id = resource_data
            .connector_customer
            .clone()
            .or_else(|| {
                resource_data
                    .customer_id
                    .as_ref()
                    .map(|customer_id| customer_id.get_string_repr().to_string())
            })
            .or_else(|| {
                request
                    .customer_id
                    .as_ref()
                    .map(|customer_id| customer_id.get_string_repr().to_string())
            })
            .map(Secret::new)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "customer.connector_customer_id",
                context: nuvei_error_context(
                    "Send a connector customer id (or customer_id); Nuvei's userTokenId is what the returned userPaymentOptionId is bound to and what the later MIT must reuse verbatim.",
                ),
            })?;

        // deviceDetails.ipAddress is a required root member on /payment.do.
        let ip_address = request
            .browser_info
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "browser_info.ip_address",
                context: nuvei_error_context(
                    "Send browser_info.ip_address; Nuvei requires deviceDetails.ipAddress on /payment.do.",
                ),
            })?
            .ip_address
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "browser_info.ip_address",
                context: nuvei_error_context(
                    "Send browser_info.ip_address; Nuvei requires deviceDetails.ipAddress on /payment.do.",
                ),
            })?;

        // SetupRecurring carries an optional amount; absent means the $0 auth.
        let minor_amount = request
            .minor_amount
            .unwrap_or_else(common_utils::types::MinorUnit::zero);
        let currency = request.currency;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Nuvei /payment.do takes the amount in decimal major units as a string.",
                ),
            })?;

        // UD-12: one mechanism or the other, never both.
        let is_zero_amount = minor_amount == common_utils::types::MinorUnit::zero();
        let (is_rebilling, authentication_only_type, rebill_params) = if is_zero_amount {
            // Mechanism 1 — zero-amount verification. RECURRING when the caller said the
            // credential is for off-session use, plain ACCOUNTVERIFICATION otherwise.
            let intent = match request.setup_future_usage {
                Some(common_enums::FutureUsage::OffSession) => {
                    NuveiAuthenticationOnlyType::Recurring
                }
                Some(common_enums::FutureUsage::OnSession) | None => {
                    NuveiAuthenticationOnlyType::AccountVerification
                }
            };
            (None, Some(intent), None)
        } else {
            // Mechanism 2 — initial CIT registration on a real amount.
            (
                Some("0".to_string()),
                None,
                Some(nuvei_rebill_v2_additional_params()),
            )
        };

        // Mechanism 2's rebill parameters live inside paymentOption.card.threeD, which
        // only the raw-card shape has; the network-token and stored-payment-option shapes
        // carry no threeD object, so they send isRebilling alone.
        let three_d = rebill_params.map(|v2_additional_params| {
            Box::new(NuveiThreeD {
                v2_additional_params: Some(v2_additional_params),
                ..Default::default()
            })
        });

        let payment_option = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_holder_name = resource_data
                    .get_optional_billing_full_name()
                    .or_else(|| request.customer_name.clone().map(Secret::new))
                    .or_else(|| card_data.card_holder_name.clone())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_address.first_name and billing_address.last_name, customer_name or payment_method.card.card_holder_name",
                        context: nuvei_error_context(
                            "Send billing_address.first_name + billing_address.last_name, customer_name, or payment_method.card.card_holder_name; Nuvei requires paymentOption.card.cardHolderName.",
                        ),
                    })?;

                NuveiPaymentOption {
                    card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                        card_number: card_data.card_number.clone(),
                        card_holder_name,
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        cvv: card_data.card_cvc.clone(),
                        three_d,
                    })),
                    alternative_payment_method: None,
                    user_payment_option_id: None,
                }
            }
            PaymentMethodData::NetworkToken(token_data) => NuveiPaymentOption {
                card: Some(NuveiCardPaymentOption::NetworkToken(
                    build_nuvei_network_token_card(token_data),
                )),
                alternative_payment_method: None,
                user_payment_option_id: None,
            },
            PaymentMethodData::PaymentMethodToken(token_data) => NuveiPaymentOption {
                card: None,
                alternative_payment_method: None,
                user_payment_option_id: Some(token_data.token.clone()),
            },
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "mandates are not supported for this payment method by Nuvei"
                        .to_string(),
                    connector: "nuvei",
                    context: nuvei_error_context(
                        "Nuvei stores a credential on file only for a raw card, a network token or an existing userPaymentOptionId; charge the other payment methods through PaymentService/Authorize instead.",
                    ),
                }
                .into())
            }
        };

        let billing_address = build_billing_address(resource_data, email, country);
        let shipping_address = get_shipping_address(resource_data);
        let dynamic_descriptor = get_dynamic_descriptor(
            request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.as_deref()),
            request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.phone.clone()),
        );

        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address.to_string()),
        };

        let time_stamp = NuveiAuthType::get_timestamp();

        // P-foundation-13: clientUniqueId is the merchant's reference (String(45),
        // guarded not truncated); clientRequestId is unique to this HTTP call.
        let (client_request_id, client_unique_id) =
            nuvei_client_ids(&resource_data.connector_request_reference_id, &time_stamp)?;

        // A mandate setup never captures funds, on either mechanism.
        let transaction_type = NuveiTransactionType::Auth;

        let url_details = request
            .router_return_url
            .as_ref()
            .map(|url| NuveiUrlDetails {
                success_url: url.clone(),
                failure_url: url.clone(),
                pending_url: url.clone(),
            });

        // Checksum: merchantId + merchantSiteId + clientRequestId + amount + currency +
        // timeStamp + merchantSecretKey (techspec checksum table, row 4).
        let checksum = auth.generate_checksum(
            "payment.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            session_token: Some(session_token),
            merchant_id: auth.merchant_id.clone(),
            merchant_site_id: auth.merchant_site_id.clone(),
            client_request_id,
            amount,
            currency,
            client_unique_id: Some(client_unique_id),
            user_token_id: Some(user_token_id),
            payment_option,
            is_rebilling,
            authentication_only_type,
            transaction_type,
            device_details,
            billing_address,
            shipping_address,
            dynamic_descriptor,
            url_details,
            time_stamp,
            checksum,
        })
    }
}

// Map the Nuvei SetupMandate response onto the SetupMandate RouterDataV2.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let is_zero_amount = router_data
            .request
            .minor_amount
            .unwrap_or_else(common_utils::types::MinorUnit::zero)
            == common_utils::types::MinorUnit::zero();

        let raw_connector_response = nuvei_raw_response(response);

        // HTTP 200 alone is never success: the envelope can say SUCCESS while the gateway
        // declined. Both shapes return Err(ErrorResponse).
        let is_envelope_error = matches!(response.status, NuveiPaymentStatus::Error);
        let is_gateway_failure = matches!(
            response.transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        );

        let status = nuvei_attempt_status(
            response.transaction_status.as_ref(),
            response.transaction_type.as_ref(),
            is_zero_amount,
            &response.status,
        );

        if is_envelope_error || is_gateway_failure {
            let error_fields = nuvei_error_fields(
                &response.status,
                response.transaction_status.as_ref(),
                response.err_code,
                response.reason.as_deref(),
                &response.gateway_error,
            );

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    connector_response: nuvei_connector_response_data(
                        response.payment_option.as_ref(),
                        response.auth_code.as_deref(),
                    ),
                    raw_connector_response: raw_connector_response.clone(),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // `transactionId` is the CIT authorization id the later MIT sends as
        // `relatedTransactionId`; `orderId` is not a substitute for it.
        let connector_transaction_id = response
            .transaction_id
            .clone()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                tracing::error!(
                    connector = "nuvei",
                    "nuvei: SetupMandate response carried no transactionId; refusing to fall back to orderId"
                );
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some(
                        "Nuvei's SetupMandate response carried no transactionId; no relatedTransactionId can be stored for the follow-up MIT"
                            .to_string(),
                    ),
                ))
            })?;

        // A zero-amount verification requires a full 3DS challenge under scheme rules, so
        // the redirect form is built here exactly as on Authorize.
        let redirection_data = nuvei_redirect_form(response.payment_option.as_ref())?;

        let connector_mandate_id = response
            .payment_option
            .as_ref()
            .and_then(|option| option.user_payment_option_id.clone())
            .filter(|id| !id.trim().is_empty());

        // G-SetupMandate-03: spec gap G-07 — no published example shows a
        // userPaymentOptionId on an `amount` "0" response, yet that token is exactly what
        // RepeatPayment needs. A *terminal* success without one is refused rather than
        // reported as a stored credential that does not exist. A non-terminal status
        // (the mandatory 3DS challenge, or a pending gateway) has not reached the point
        // where Nuvei would have issued the token, so it is left alone.
        let is_terminal_success = matches!(
            status,
            common_enums::AttemptStatus::Charged | common_enums::AttemptStatus::Authorized
        );
        if connector_mandate_id.is_none() && is_terminal_success {
            tracing::error!(
                connector = "nuvei",
                ?status,
                "nuvei: SetupMandate succeeded without a userPaymentOptionId; no mandate reference can be stored"
            );
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthorizationFailed,
                    connector_response: nuvei_connector_response_data(
                        response.payment_option.as_ref(),
                        response.auth_code.as_deref(),
                    ),
                    raw_connector_response: raw_connector_response.clone(),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: consts::NO_ERROR_CODE.to_string(),
                    message: NUVEI_NO_USER_PAYMENT_OPTION_ID.to_string(),
                    reason: Some(NUVEI_NO_USER_PAYMENT_OPTION_ID.to_string()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(
                        common_enums::AttemptStatus::AuthorizationFailed,
                    )),
                    connector_transaction_id: Some(connector_transaction_id),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
            redirection_data,
            // The CIT authorization transactionId travels with the stored credential:
            // RepeatPayment reads it back as the MIT's `relatedTransactionId`, which Nuvei
            // expects to name the original CIT authorization (not its settle). The 3DS
            // REDIRECT leg still carries no userPaymentOptionId, so no mandate reference —
            // and therefore no reference id — is published there.
            mandate_reference: connector_mandate_id.map(|connector_mandate_id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(connector_mandate_id),
                    payment_method_id: None,
                    mandate_metadata: None,
                    connector_mandate_request_reference_id: Some(connector_transaction_id.clone()),
                })
            }),
            connector_metadata: None,
            // Authorize proved live that Nuvei's sandbox does return
            // externalSchemeTransactionId; an empty one is dropped, never fabricated.
            network_txn_id: nuvei_network_txn_id(
                response.external_scheme_transaction_id.as_deref(),
            ),
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|id| !id.is_empty()),
            connector_response_reference_id: response.order_id.clone().filter(|id| !id.is_empty()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: nuvei_connector_response_data(
                    response.payment_option.as_ref(),
                    response.auth_code.as_deref(),
                ),
                raw_connector_response,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

/// G-SetupMandate-03's message, kept in one place so the refusal reads identically in the
/// `message` and the `reason`.
const NUVEI_NO_USER_PAYMENT_OPTION_ID: &str =
    "Nuvei returned no userPaymentOptionId; no mandate reference can be stored";

// ===== RepeatPayment (MIT) flow =====
//
// A merchant-initiated transaction is the same `POST /payment.do` as Authorize with
// `isRebilling: "1"` (spec "#### 5f. RepeatPayment (MIT)"). Three credential shapes
// reach it, and they are chosen by the *mandate reference*, never by the payment method:
//
// | `MandateReferenceId`      | credential on the wire                                   | chain link                          |
// |---------------------------|----------------------------------------------------------|-------------------------------------|
// | `ConnectorMandateId`      | `paymentOption.userPaymentOptionId` (the stored UPO)      | `relatedTransactionId` (the CIT auth) |
// | `NetworkMandateId`        | `paymentOption.card` — raw PAN, **no CVV** on a rebill    | `externalSchemeDetails.{transactionId, brand}` |
// | `NetworkTokenWithNTI`     | `paymentOption.card.externalToken` (network token)        | `externalSchemeDetails.{transactionId, brand}` |
//
// `externalSchemeDetails` is the **inbound** NTID: the scheme transaction id of an
// initial MIT that ran at *another* PSP, which Nuvei therefore does not hold. It is a
// pointer to a credential, so the credential itself (the card or the network token) must
// travel on the same call — that is guard `G-RepeatPayment-03`.
//
// The **outbound** NTID is `externalSchemeTransactionId` on the response. Nuvei does not
// document it, but the Authorize unit proved live against the sandbox that it is returned
// (spec gap G-06 / UD-11), so it is read opportunistically through [`nuvei_network_txn_id`]
// and published as `network_txn_id` for the next MIT in the chain.
//
// `externalToken.*` (networkTokenNumber, cryptogram, tokenRequestorId) is network
// *tokenisation* and is a different feature from the NTID — the two are never conflated.
//
// Backend REST only: no Web SDK, and no CVV2 is required on a subsequent rebill.

/// `rebillingType` (spec "#### 5f. RepeatPayment (MIT)") — the four values Nuvei's MIT
/// guide documents. Typed rather than a bare `String` so the mapping from
/// [`common_enums::MitCategory`] is exhaustive and the wire vocabulary stays greppable.
#[derive(Debug, Clone, Serialize)]
pub enum NuveiRebillingType {
    /// An unscheduled merchant-initiated charge against a stored credential.
    #[serde(rename = "MIT")]
    Mit,
    /// A scheduled, repeating charge.
    #[serde(rename = "Recurring")]
    Recurring,
    /// A no-show charge (hospitality).
    #[serde(rename = "NoShow")]
    NoShow,
    /// A delayed charge raised after the original service (hospitality / rental).
    #[serde(rename = "DelayedCharges")]
    DelayedCharges,
}

impl From<common_enums::MitCategory> for NuveiRebillingType {
    fn from(category: common_enums::MitCategory) -> Self {
        match category {
            // Both are fixed-schedule repeats; Nuvei has one value for them.
            common_enums::MitCategory::Recurring | common_enums::MitCategory::Installment => {
                Self::Recurring
            }
            common_enums::MitCategory::Unscheduled | common_enums::MitCategory::Resubmission => {
                Self::Mit
            }
        }
    }
}

/// `paymentOption.card.storedCredentials` (spec "## Card-on-file, mandates and MIT").
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiStoredCredentials {
    pub stored_credentials_mode: NuveiStoredCredentialsMode,
}

/// `storedCredentialsMode`: `"0"` stores the credential for the first time (a CIT),
/// `"1"` uses one that is already stored — which every MIT does.
#[derive(Debug, Clone, Serialize)]
pub enum NuveiStoredCredentialsMode {
    #[serde(rename = "0")]
    First,
    #[serde(rename = "1")]
    Used,
}

/// Documented per-scheme shapes of `externalSchemeDetails.transactionId`:
/// Mastercard is 3 letters + 6 alphanumerics + `MMDD` (13 characters); Visa and Amex are
/// up to 15 digits. Discover and Diners publish no shape.
///
/// A mismatch is **traced, never refused**: the NTID is minted by the scheme and forwarded
/// by whichever PSP ran the initial MIT, so rejecting an unexpected shape here would break
/// a chain Nuvei itself would have accepted. The value is never logged.
fn nuvei_ntid_matches_scheme(brand: &NuveiCardType, ntid: &str) -> bool {
    if !ntid.is_ascii() {
        return false;
    }
    match brand {
        NuveiCardType::MasterCard => {
            ntid.len() == 13
                && ntid[..3].chars().all(|c| c.is_ascii_alphabetic())
                && ntid[3..9].chars().all(|c| c.is_ascii_alphanumeric())
                && ntid[9..].chars().all(|c| c.is_ascii_digit())
        }
        NuveiCardType::Visa | NuveiCardType::Amex => {
            !ntid.is_empty() && ntid.len() <= 15 && ntid.chars().all(|c| c.is_ascii_digit())
        }
        // No published shape — nothing to check against.
        NuveiCardType::Discover | NuveiCardType::Diners => true,
    }
}

/// Builds `externalSchemeDetails` from an inbound NTID, tracing when the value does not
/// match the brand's documented shape.
fn nuvei_external_scheme_details(
    network_transaction_id: &str,
    brand: NuveiCardType,
) -> NuveiExternalSchemeDetails {
    if !nuvei_ntid_matches_scheme(&brand, network_transaction_id) {
        tracing::warn!(
            connector = "nuvei",
            brand = ?brand,
            ntid_len = network_transaction_id.len(),
            "nuvei: externalSchemeDetails.transactionId does not match the documented shape for this brand; sending it unchanged"
        );
    }
    NuveiExternalSchemeDetails {
        transaction_id: Secret::new(network_transaction_id.to_string()),
        brand: Some(brand),
    }
}

/// `G-RepeatPayment-03`'s message, kept in one place so the refusal reads identically
/// wherever the credential on the request does not match the mandate reference.
const NUVEI_MIT_CREDENTIAL_MISMATCH: &str =
    "an NTID MIT needs the matching card or network-token data on the Charge request";

/// The payment-method refusal for the RepeatPayment flow (plan §4 `pm_arms`).
const NUVEI_MIT_UNSUPPORTED_PAYMENT_METHOD: &str =
    "Nuvei RepeatPayment accepts a stored userPaymentOptionId, a raw card with an NTID, or a network token with an NTID";

/// `deviceDetails.ipAddress` for a merchant-initiated charge.
///
/// A MIT has no browser by definition, so `browser_info` is usually absent. Hyperswitch's
/// own `MandatePayment` arm replays the CIT's IP out of `mandate_metadata` and hard-fails
/// without it; UCS keeps that source but tries the live `browser_info` first and accepts
/// either carrier, so a caller that does send browser info is not refused.
fn nuvei_mit_ip_address<T: PaymentMethodDataTypes>(
    request: &RepeatPaymentData<T>,
) -> Result<Secret<String, pii::IpAddress>, Report<IntegrationError>> {
    if let Some(ip_address) = request
        .browser_info
        .as_ref()
        .and_then(|browser_info| browser_info.ip_address)
    {
        return Ok(Secret::new(ip_address.to_string()));
    }

    // Replayed CIT IP: `recurring_mandate_payment_data.mandate_metadata` first (the shape
    // hyperswitch writes), then the same member on the mandate reference itself.
    let replayed = request
        .recurring_mandate_payment_data
        .as_ref()
        .and_then(|data| data.mandate_metadata.clone())
        .or_else(|| match &request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(c) => c.get_mandate_metadata(),
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => None,
        })
        .and_then(|metadata| nuvei_ip_from_mandate_metadata(&metadata));

    replayed.ok_or_else(|| {
        Report::new(IntegrationError::MissingRequiredField {
            field_name: "browser_info.ip_address",
            context: nuvei_error_context(
                "Nuvei requires deviceDetails.ipAddress on /payment.do. A merchant-initiated charge has no browser, so send the IP captured at the initial CIT either as browser_info.ip_address or as the mandate_metadata of the mandate reference.",
            ),
        })
    })
}

/// Reads the replayed CIT IP out of `mandate_metadata`, which hyperswitch writes as a bare
/// JSON string and other callers as `{"ip_address": "..."}`.
fn nuvei_ip_from_mandate_metadata(
    metadata: &pii::SecretSerdeValue,
) -> Option<Secret<String, pii::IpAddress>> {
    let value = metadata.peek();
    let ip = value
        .as_str()
        .or_else(|| value.get("ip_address").and_then(serde_json::Value::as_str))?;
    if ip.trim().is_empty() {
        tracing::debug!(
            connector = "nuvei",
            "nuvei: mandate_metadata carried an empty ip_address; ignoring it"
        );
        return None;
    }
    Some(Secret::new(ip.to_string()))
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Option<Secret<String>>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub client_unique_id: Option<String>,
    /// `userTokenId` must match the value the initial CIT used so Nuvei resolves the
    /// stored payment option; it is required on every MIT.
    pub user_token_id: Option<Secret<String>>,
    pub payment_option: NuveiRepeatPaymentOptionTypes<T>,
    /// `"1"` marks a merchant-initiated rebilling transaction. Every arm of this flow is
    /// an MIT, so it is always sent.
    pub is_rebilling: Option<String>,
    /// `MIT` | `Recurring` | `NoShow` | `DelayedCharges`, derived from `mit_category`.
    pub rebilling_type: Option<NuveiRebillingType>,
    /// The original CIT **authorization** `transactionId` — never the settle's. Carried on
    /// the mandate reference as `connector_mandate_request_reference_id`.
    pub related_transaction_id: Option<String>,
    /// Inbound NTID (+ brand) when the initial MIT was processed by an external PSP.
    pub external_scheme_details: Option<NuveiExternalSchemeDetails>,
    /// Mastercard Transaction Link Identifier from the CIT, when the scheme issued one.
    pub transaction_link_id: Option<String>,
    pub transaction_type: NuveiTransactionType,
    pub device_details: NuveiDeviceDetails,
    /// Optional on MIT: the stored userPaymentOptionId already carries the billing info
    /// captured at CIT. Forwarded when the caller supplies it so Nuvei can re-run AVS.
    pub billing_address: Option<NuveiBillingAddress>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

// Serialize-only untagged enum: each credential shape emits its own `paymentOption`
// body and the variant itself is invisible on the wire.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiRepeatPaymentOptionTypes<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    StoredCredential(NuveiRepeatPaymentOption),
    RawCard(NuveiRepeatPaymentRawCardOption<T>),
    NetworkToken(NuveiRepeatPaymentCardOption),
}

/// `paymentOption` for a stored-credential MIT — only `userPaymentOptionId` is required;
/// Nuvei reuses the card bound to that id.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentOption {
    pub user_payment_option_id: Secret<String>,
}

/// `paymentOption` for a network-token MIT.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentCardOption {
    pub card: NuveiNetworkTokenCard,
}

/// `paymentOption` for a raw-card MIT carrying an inbound NTID.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentRawCardOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card: NuveiRepeatPaymentRawCard<T>,
}

/// The card object on a rebill: no `CVV`, because Nuvei does not require CVV2 on a
/// subsequent merchant-initiated charge and the merchant is not allowed to store it.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentRawCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    pub card_holder_name: Option<Secret<String>>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub stored_credentials: Option<NuveiStoredCredentials>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
    /// The outbound NTID (spec gap G-06 / UD-11) — the scheme transaction id this MIT
    /// produced, which the next MIT in the chain sends back as `externalSchemeDetails`.
    pub external_scheme_transaction_id: Option<String>,
    pub transaction_link_id: Option<String>,
    pub auth_code: Option<String>,
    pub session_token: Option<Secret<String>>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub payment_option: Option<NuveiResponsePaymentOption>,
}

// Build the RepeatPayment request from the router data.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiRepeatPaymentRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
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
        let resource_data = &router_data.resource_common_data;

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // G-RepeatPayment-05: /payment.do has one transactionType per call, so the
        // multi-capture and scheduled capture methods have nothing to map onto.
        if matches!(
            request.capture_method,
            Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        ) {
            return Err(IntegrationError::CaptureMethodNotSupported {
                context: nuvei_error_context(
                    "Use capture_method AUTOMATIC (transactionType Sale) or MANUAL (transactionType Auth) on a Nuvei merchant-initiated charge.",
                ),
            }
            .into());
        }

        // G-RepeatPayment-01: /payment.do requires a sessionToken and
        // RecurringPaymentServiceChargeRequest has no session_token field of its own, so
        // state.access_token is the only carrier. The error names that carrier.
        let session_token = resource_data
            .access_token
            .as_ref()
            .map(|access_token| access_token.access_token.clone())
            .ok_or_else(|| {
                Report::new(IntegrationError::MissingRequiredField {
                    field_name: "state.access_token",
                    context: nuvei_error_context(
                        "Call MerchantAuthenticationService/CreateServerSessionAuthenticationToken first and pass its session_token as state.access_token on the Charge request.",
                    ),
                })
            })?;

        // G-RepeatPayment-04: userTokenId resolves the stored payment option and is
        // required on every MIT arm.
        let user_token_id = resource_data
            .connector_customer
            .clone()
            .map(Secret::new)
            .ok_or_else(|| {
                Report::new(IntegrationError::MissingRequiredField {
                    field_name: "connector_customer_id",
                    context: nuvei_error_context(
                        "Send the same customer identifier the initial CIT used; Nuvei's userTokenId is what resolves the stored payment option.",
                    ),
                })
            })?;

        // The credential shape is chosen by the mandate reference, never by the payment
        // method: a stored UPO needs no card, and an NTID needs the credential alongside.
        let (payment_option, external_scheme_details, related_transaction_id, transaction_link_id) =
            match &request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(connector_mandate) => {
                    // G-RepeatPayment-02
                    let user_payment_option_id = connector_mandate
                        .get_connector_mandate_id()
                        .filter(|id| !id.is_empty())
                        .ok_or_else(|| {
                            Report::new(IntegrationError::MissingRequiredField {
                                field_name: "mandate_reference.connector_mandate_id",
                                context: nuvei_error_context(
                                    "Send the userPaymentOptionId the initial SetupMandate returned as mandate_reference.connector_mandate_id.",
                                ),
                            })
                        })?;

                    (
                        NuveiRepeatPaymentOptionTypes::StoredCredential(NuveiRepeatPaymentOption {
                            user_payment_option_id: Secret::new(user_payment_option_id),
                        }),
                        None,
                        // The original CIT authorization transactionId, not the settle's.
                        connector_mandate
                            .get_connector_mandate_request_reference_id()
                            .filter(|id| !id.is_empty()),
                        None,
                    )
                }
                MandateReferenceId::NetworkMandateId(nti_ref) => {
                    let card = match &request.payment_method_data {
                        PaymentMethodData::Card(card) => card,
                        // G-RepeatPayment-03: the mandate reference and the credential
                        // must agree.
                        PaymentMethodData::NetworkToken(_) => {
                            return Err(IntegrationError::NotSupported {
                                message: NUVEI_MIT_CREDENTIAL_MISMATCH.to_string(),
                                connector: "nuvei",
                                context: nuvei_error_context(
                                    "A NetworkMandateId names a raw-card credential: send payment_method.card on the Charge request, or switch the mandate reference to NetworkTokenWithNTI.",
                                ),
                            }
                            .into())
                        }
                        _ => {
                            return Err(IntegrationError::NotSupported {
                                message: NUVEI_MIT_UNSUPPORTED_PAYMENT_METHOD.to_string(),
                                connector: "nuvei",
                                context: nuvei_error_context(
                                    "Nuvei's merchant-initiated charge is card-only: send a stored connector_mandate_id, or a raw card / network token together with its NTID.",
                                ),
                            }
                            .into())
                        }
                    };

                    let brand = match card.card_network.clone() {
                        Some(network) => NuveiCardType::try_from(network)?,
                        None => NuveiCardType::try_from(&domain_types::utils::get_card_issuer(
                            card.card_number.peek(),
                        )?)?,
                    };

                    (
                        NuveiRepeatPaymentOptionTypes::RawCard(NuveiRepeatPaymentRawCardOption {
                            card: NuveiRepeatPaymentRawCard {
                                card_number: card.card_number.clone(),
                                card_holder_name: card
                                    .card_holder_name
                                    .clone()
                                    .or_else(|| resource_data.get_optional_billing_full_name()),
                                expiration_month: card.card_exp_month.clone(),
                                expiration_year: card.card_exp_year.clone(),
                                stored_credentials: Some(NuveiStoredCredentials {
                                    stored_credentials_mode: NuveiStoredCredentialsMode::Used,
                                }),
                            },
                        }),
                        Some(nuvei_external_scheme_details(
                            &nti_ref.network_transaction_id,
                            brand,
                        )),
                        None,
                        nti_ref
                            .transaction_link_id
                            .clone()
                            .filter(|id| !id.is_empty()),
                    )
                }
                MandateReferenceId::NetworkTokenWithNTI(nti_ref) => {
                    let token_data = match &request.payment_method_data {
                        PaymentMethodData::NetworkToken(token_data) => token_data,
                        // G-RepeatPayment-03, the mirror case.
                        PaymentMethodData::Card(_) => {
                            return Err(IntegrationError::NotSupported {
                                message: NUVEI_MIT_CREDENTIAL_MISMATCH.to_string(),
                                connector: "nuvei",
                                context: nuvei_error_context(
                                    "A NetworkTokenWithNTI mandate reference names a network-token credential: send payment_method.network_token on the Charge request, or switch the mandate reference to NetworkMandateId.",
                                ),
                            }
                            .into())
                        }
                        _ => {
                            return Err(IntegrationError::NotSupported {
                                message: NUVEI_MIT_UNSUPPORTED_PAYMENT_METHOD.to_string(),
                                connector: "nuvei",
                                context: nuvei_error_context(
                                    "Nuvei's merchant-initiated charge is card-only: send a stored connector_mandate_id, or a raw card / network token together with its NTID.",
                                ),
                            }
                            .into())
                        }
                    };

                    (
                        NuveiRepeatPaymentOptionTypes::NetworkToken(NuveiRepeatPaymentCardOption {
                            card: build_nuvei_network_token_card(token_data),
                        }),
                        Some(nuvei_external_scheme_details(
                            &nti_ref.network_transaction_id,
                            get_nuvei_card_brand(token_data)?,
                        )),
                        None,
                        nti_ref
                            .transaction_link_id
                            .clone()
                            .filter(|id| !id.is_empty()),
                    )
                }
            };

        let device_details = NuveiDeviceDetails {
            ip_address: nuvei_mit_ip_address(request)?,
        };

        let billing_address = get_billing_address(resource_data, request.email.clone());

        let time_stamp = NuveiAuthType::get_timestamp();
        let (client_request_id, client_unique_id) =
            nuvei_client_ids(&resource_data.connector_request_reference_id, &time_stamp)?;

        let currency = request.currency;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(request.minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: nuvei_error_context(
                    "Nuvei's /payment.do amount is a decimal major-unit string; the minor-unit amount could not be converted.",
                ),
            })?;

        // Default to Sale so the funds capture in one step; Auth only when the caller
        // explicitly asks for manual capture.
        let transaction_type = match request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => NuveiTransactionType::Auth,
            _ => NuveiTransactionType::Sale,
        };

        let rebilling_type = request.mit_category.clone().map(NuveiRebillingType::from);
        if rebilling_type.is_none() {
            tracing::debug!(
                connector = "nuvei",
                "nuvei: no mit_category on the Charge request; rebillingType is omitted and Nuvei applies its account default"
            );
        }

        let checksum = auth.generate_checksum(
            "payment.do / openOrder.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            session_token: Some(session_token),
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            client_unique_id: Some(client_unique_id),
            user_token_id: Some(user_token_id),
            payment_option,
            // Every arm of this flow is a merchant-initiated rebill.
            is_rebilling: Some("1".to_string()),
            rebilling_type,
            related_transaction_id,
            external_scheme_details,
            transaction_link_id,
            transaction_type,
            device_details,
            billing_address,
            time_stamp,
            checksum,
        })
    }
}

// Map the Nuvei RepeatPayment response onto the RepeatPayment RouterDataV2.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        // A rebill is never a zero-amount verification; that is SetupMandate's mechanism.
        let is_zero_amount = false;

        let raw_connector_response = nuvei_raw_response(response);

        // A 2xx is never success on its own: the envelope can say SUCCESS while the
        // gateway declined. Both shapes return Err(ErrorResponse).
        let is_envelope_error = matches!(response.status, NuveiPaymentStatus::Error);
        let is_gateway_failure = matches!(
            response.transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        );

        let status = nuvei_attempt_status(
            response.transaction_status.as_ref(),
            response.transaction_type.as_ref(),
            is_zero_amount,
            &response.status,
        );

        if is_envelope_error || is_gateway_failure {
            let error_fields = nuvei_error_fields(
                &response.status,
                response.transaction_status.as_ref(),
                response.err_code,
                response.reason.as_deref(),
                &response.gateway_error,
            );

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    connector_response: nuvei_connector_response_data(
                        response.payment_option.as_ref(),
                        response.auth_code.as_deref(),
                    ),
                    raw_connector_response: raw_connector_response.clone(),
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: error_fields.network_decline_code,
                    network_advice_code: error_fields.network_advice_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: None,
                    raw_connector_response,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // One reference id per payment: `transactionId` is the only value that is a valid
        // `relatedTransactionId` for a settle / void / refund of this rebill, and the only
        // one that chains the next MIT. `orderId` is not a substitute.
        let connector_transaction_id = response
            .transaction_id
            .clone()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                tracing::error!(
                    connector = "nuvei",
                    "nuvei: MIT /payment.do response carried no transactionId; refusing to fall back to orderId"
                );
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some(
                        "Nuvei's merchant-initiated /payment.do response carried no transactionId; the rebill cannot be settled, voided or refunded without it"
                            .to_string(),
                    ),
                ))
            })?;

        let redirection_data = nuvei_redirect_form(response.payment_option.as_ref())?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
            redirection_data,
            // Re-publish the stored credential and the CIT reference so a chained MIT can
            // be built from this response alone.
            mandate_reference: response
                .payment_option
                .as_ref()
                .and_then(|option| option.user_payment_option_id.clone())
                .filter(|id| !id.is_empty())
                .map(|user_payment_option_id| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(user_payment_option_id),
                        payment_method_id: None,
                        mandate_metadata: None,
                        connector_mandate_request_reference_id: Some(
                            connector_transaction_id.clone(),
                        ),
                    })
                }),
            connector_metadata: None,
            // The outbound NTID: Authorize proved live that Nuvei returns
            // externalSchemeTransactionId (spec gap G-06 / UD-11). An empty one is
            // dropped, never fabricated.
            network_txn_id: nuvei_network_txn_id(
                response.external_scheme_transaction_id.as_deref(),
            ),
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|id| !id.is_empty()),
            connector_response_reference_id: response.order_id.clone().filter(|id| !id.is_empty()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: nuvei_connector_response_data(
                    response.payment_option.as_ref(),
                    response.auth_code.as_deref(),
                ),
                raw_connector_response,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ============================================================================
// ThreeDS — PreAuthenticate / Authenticate / PostAuthenticate
// ============================================================================
//
// Nuvei's REST 1.0 3DS 2.x sequence is **two** endpoints, not three
// (spec "## API Call Sequences — Flow: ThreeDS"):
//
// | UCS flow          | Nuvei call                        | Distinguishing feature |
// |-------------------|-----------------------------------|------------------------|
// | `PreAuthenticate` | `POST /initPayment.do`            | `threeD.methodNotificationUrl` only; returns `v2supported`, `version`, `methodUrl`, `methodPayload`, `serverTransId` and a **refreshed** `sessionToken` |
// | `Authenticate`    | **first** `POST /payment.do`      | `relatedTransactionId` = the init `transactionId`, **plus** the full `threeD` challenge block |
// | `PostAuthenticate`| **second** `POST /payment.do`     | `relatedTransactionId` = the authenticate `transactionId`, `threeD` **omitted entirely** |
//
// `/initPayment.do → /authorize3d.do → /verify3d.do` is the **MPI-only** variant
// (spec "#### 5d. External MPI") and is deliberately *not* what these three legs
// implement; the merchant-supplied-MPI path rides on the single-call Authorize flow.
//
// The three legs share one piece of connector state — the refreshed `sessionToken`,
// the `relatedTransactionId` to chain on, the negotiated 3DS `version` and the
// `serverTransId` — which travels between them through `connector_feature_data`
// (`PaymentFlowData.connector_feature_data` on the way in,
// `PaymentsResponseData::{PreAuthenticate,Authenticate}Response` on the way out).
// Neither the gRPC surface nor `PaymentFlowData` carries a `session_token` on any of
// the three authentication RPCs, so this round-trip is the only channel there is.

/// The connector state the three authentication legs hand to each other, serialised
/// into `connector_feature_data` under the key [`NUVEI_THREE_DS_STATE_KEY`].
///
/// Everything in here is `PREVIOUS_API` data: it is produced by one Nuvei leg and
/// consumed by the next, and none of it is merchant-supplied.
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NuveiThreeDsState {
    /// The refreshed `sessionToken`. `/initPayment.do` mints a new one and the
    /// following `/payment.do` legs must use *that* token, not the original.
    pub session_token: Option<Secret<String>>,
    /// The previous leg's `transactionId`, sent as `relatedTransactionId` on the next
    /// leg. A stale or absent value is filter code 1155 — "3D Related transaction is
    /// missing or incorrect" — the single most common 3DS wiring failure.
    pub related_transaction_id: Option<String>,
    /// The negotiated 3DS message version, echoed back on the Authenticate leg.
    pub version: Option<String>,
    /// The 3DS server transaction id.
    pub server_trans_id: Option<String>,
    /// `threeD.v2supported` from `/initPayment.do`, parsed from the string Nuvei
    /// returns. `Some(false)` means the card is not enrolled for 3DS 2 and the
    /// sequence must not continue.
    pub v2_supported: Option<bool>,
}

/// Key the Nuvei 3DS state is nested under inside `connector_feature_data`, so that a
/// merchant-supplied feature blob is read alongside it rather than overwritten.
const NUVEI_THREE_DS_STATE_KEY: &str = "nuvei_three_ds";

/// Reads the Nuvei 3DS state out of `PaymentFlowData.connector_feature_data`.
///
/// A missing key, or a value this connector did not write, yields the default (empty)
/// state rather than an error: the guards downstream ([`nuvei_three_ds_session_token`],
/// [`nuvei_related_transaction_id`]) are what refuse, with a field name the caller can
/// act on.
fn nuvei_three_ds_state(feature_data: Option<&pii::SecretSerdeValue>) -> NuveiThreeDsState {
    let Some(feature_data) = feature_data else {
        return NuveiThreeDsState::default();
    };
    let Some(state_value) = feature_data.peek().get(NUVEI_THREE_DS_STATE_KEY) else {
        return NuveiThreeDsState::default();
    };
    match serde_json::from_value::<NuveiThreeDsState>(state_value.clone()) {
        Ok(state) => state,
        Err(error) => {
            tracing::warn!(
                connector = "nuvei",
                deserialization_error = ?error,
                "nuvei: connector_feature_data carried an unreadable nuvei_three_ds block; treating the 3DS state as empty"
            );
            NuveiThreeDsState::default()
        }
    }
}

/// Renders the Nuvei 3DS state back into the `connector_feature_data` shape the next
/// leg expects. Returns `None` when there is nothing to carry, so an empty object is
/// never attached.
fn nuvei_three_ds_state_value(state: &NuveiThreeDsState) -> Option<serde_json::Value> {
    if state.session_token.is_none()
        && state.related_transaction_id.is_none()
        && state.version.is_none()
        && state.server_trans_id.is_none()
        && state.v2_supported.is_none()
    {
        return None;
    }
    match serde_json::to_value(state) {
        Ok(value) => Some(serde_json::json!({ NUVEI_THREE_DS_STATE_KEY: value })),
        Err(error) => {
            tracing::warn!(
                connector = "nuvei",
                serialization_error = ?error,
                "nuvei: could not render the 3DS state for connector_feature_data"
            );
            None
        }
    }
}

/// G-ThreeDS-02: every 3DS leg is authenticated by a `sessionToken`.
///
/// Resolution order, most specific first:
/// 1. the token the previous leg carried in `connector_feature_data` —
///    `/initPayment.do` **refreshes** the token and the following `/payment.do` calls
///    must use the refreshed one;
/// 2. `PaymentFlowData.session_token`;
/// 3. `PaymentFlowData.access_token`, which is where
///    `PaymentMethodAuthenticationService/*Authenticate`'s `state.access_token` lands —
///    the only inbound channel the three authentication RPCs have for a session token,
///    since none of them carries a `session_token` field and
///    `ForeignTryFrom<..> for PaymentFlowData` sets `session_token: None` on all three.
fn nuvei_three_ds_session_token(
    resource_data: &PaymentFlowData,
    state: &NuveiThreeDsState,
) -> Result<Secret<String>, Report<IntegrationError>> {
    let non_empty = |token: String| {
        if token.trim().is_empty() {
            None
        } else {
            Some(Secret::new(token))
        }
    };

    state
        .session_token
        .clone()
        .and_then(|token| non_empty(token.peek().to_string()))
        .or_else(|| resource_data.session_token.clone().and_then(non_empty))
        .or_else(|| {
            resource_data
                .access_token
                .as_ref()
                .and_then(|token| non_empty(token.access_token.peek().to_string()))
        })
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: nuvei_error_context(
                    "Run MerchantAuthenticationService/CreateServerSessionAuthenticationToken first and pass its sessionToken as state.access_token.token; every Nuvei 3DS leg is authenticated by a sessionToken.",
                ),
            }
            .into()
        })
}

/// G-ThreeDS-03: the `relatedTransactionId` that chains one 3DS leg to the previous one.
///
/// Prefers the id the previous leg published in `connector_feature_data`, then the
/// `connector_order_reference_id` the composite service copies from the Authenticate
/// response onto the PostAuthenticate request (`PaymentFlowData.reference_id`).
/// A missing value is refused before the call: sending `/payment.do` without it — or
/// with a stale one — is gateway filter code 1155.
fn nuvei_related_transaction_id(
    state: &NuveiThreeDsState,
    fallback: Option<&str>,
) -> Result<String, Report<IntegrationError>> {
    state
        .related_transaction_id
        .clone()
        .or_else(|| fallback.map(ToOwned::to_owned))
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| {
            IntegrationError::MissingConnectorTransactionID {
                context: nuvei_error_context(
                    "The previous 3DS leg's transactionId must be carried forward in connector_feature_data; Nuvei rejects a /payment.do whose relatedTransactionId is missing or stale with gwErrorCode -1100 / gwExtendedErrorCode 1155.",
                ),
            }
            .into()
        })
}

/// G-ThreeDS-01: `deviceDetails.ipAddress` and `threeD.browserDetails.ip` are both
/// required for `platformType == "02"`, and both come from `browser_info.ip_address`.
fn nuvei_three_ds_ip_address(
    browser_info: Option<&domain_types::router_request_types::BrowserInformation>,
) -> Result<Secret<String, pii::IpAddress>, Report<IntegrationError>> {
    browser_info
        .and_then(|info| info.ip_address)
        .map(|ip| Secret::new(ip.to_string()))
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "browser_info.ip_address",
                context: nuvei_error_context(
                    "Send browser_info.ip_address; Nuvei requires deviceDetails.ipAddress on every 3DS leg and threeD.browserDetails.ip for the browser challenge (platformType 02).",
                ),
            }
            .into()
        })
}

/// Builds `threeD.browserDetails` (spec "#### 5b"). Nuvei takes every member as a
/// string, including the two booleans (`TRUE` / `FALSE`, uppercase) and the numeric
/// screen geometry, so the rendering happens here rather than on the wire type.
///
/// Only `ip` is mandatory — it is resolved by [`nuvei_three_ds_ip_address`], which is
/// G-ThreeDS-01. The remaining members are omitted when the caller sent nothing; they
/// are risk-scoring inputs, and a fabricated value is worse than an absent one.
fn nuvei_browser_details(
    browser_info: Option<&domain_types::router_request_types::BrowserInformation>,
    ip: Secret<String, pii::IpAddress>,
) -> NuveiBrowserDetails {
    let flag =
        |value: Option<bool>| value.map(|value| if value { "TRUE" } else { "FALSE" }.to_string());
    NuveiBrowserDetails {
        accept_header: browser_info.and_then(|info| info.accept_header.clone()),
        ip,
        java_enabled: flag(browser_info.and_then(|info| info.java_enabled)),
        java_script_enabled: flag(browser_info.and_then(|info| info.java_script_enabled)),
        language: browser_info.and_then(|info| info.language.clone()),
        color_depth: browser_info
            .and_then(|info| info.color_depth)
            .map(|depth| depth.to_string()),
        screen_height: browser_info
            .and_then(|info| info.screen_height)
            .map(|height| height.to_string()),
        screen_width: browser_info
            .and_then(|info| info.screen_width)
            .map(|width| width.to_string()),
        time_zone: browser_info
            .and_then(|info| info.time_zone)
            .map(|zone| zone.to_string()),
        user_agent: browser_info.and_then(|info| info.user_agent.clone()),
    }
}

/// `threeD.platformType`: `02` browser, `01` mobile app (spec "#### 5b").
///
/// `isDynamic3D`, `dynamic3DMode` and `deviceChannel` are **not** sent (UD-05):
/// `platformType` is the REST 1.0 channel field, and `manage_3d_mode` is a DMN
/// response echo rather than a request member.
fn nuvei_platform_type(
    device_channel: Option<&domain_types::connector_types::DeviceChannel>,
) -> String {
    match device_channel {
        Some(domain_types::connector_types::DeviceChannel::App) => "01",
        // Browser is also the default: `browserDetails` is what these legs send, and
        // Nuvei requires it exactly for platformType 02.
        Some(domain_types::connector_types::DeviceChannel::Browser) | None => "02",
    }
    .to_string()
}

/// `threeD.methodCompletionInd` (spec "#### 5b"): `Y` fingerprinting completed,
/// `N` failed, `U` unavailable.
///
/// The only signal UCS has is whether the cardholder came back from the device-data
/// collection form with a payload. A populated `redirect_response` is `Y`; anything
/// else is `U` (unavailable) — never `N`, which asserts a *failure* the connector
/// cannot actually observe.
fn nuvei_method_completion_ind(
    redirect_response: Option<&domain_types::connector_types::ContinueRedirectionResponse>,
) -> String {
    let completed = redirect_response.is_some_and(|response| {
        response
            .params
            .as_ref()
            .is_some_and(|params| !params.peek().trim().is_empty())
            || response.payload.is_some()
    });
    if completed { "Y" } else { "U" }.to_string()
}

/// The card the 3DS legs authenticate.
///
/// Nuvei's 3DS 2.x sequence is card-only: `/initPayment.do` takes a
/// `paymentOption.card` and nothing else, and the challenge block lives under
/// `paymentOption.card.threeD`. Every other `PaymentMethodData` arm — network token,
/// stored payment option, the wallets, the bank redirects and debits, UPI, crypto,
/// vouchers, gift cards and the real-time / open-banking methods — is refused here
/// rather than silently authorised without authentication.
fn nuvei_three_ds_card<T>(
    payment_method_data: Option<&PaymentMethodData<T>>,
) -> Result<&domain_types::payment_method_data::Card<T>, Report<IntegrationError>>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    match payment_method_data {
        Some(PaymentMethodData::Card(card)) => Ok(card),
        _ => Err(IntegrationError::NotImplemented(
            "3DS authentication is card-only for Nuvei".to_string(),
            nuvei_error_context(
                "Nuvei's /initPayment.do and the 3DS /payment.do legs accept only paymentOption.card; send a card, or run the payment without 3DS.",
            ),
        )
        .into()),
    }
}

/// `paymentOption.card.cardHolderName` is mandatory on every Nuvei card call, and on
/// the sandbox it doubles as the 3DS scenario lever (`CL-BRW1` forces a challenge,
/// `FL-BRW1` forces frictionless).
fn nuvei_three_ds_card_holder_name(
    resource_data: &PaymentFlowData,
    card_holder_name: Option<&Secret<String>>,
) -> Result<Secret<String>, Report<IntegrationError>> {
    resource_data
        .get_optional_billing_full_name()
        .or_else(|| card_holder_name.cloned())
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "billing_address.first_name and billing_address.last_name or payment_method.card.card_holder_name",
                context: nuvei_error_context(
                    "Send billing_address.first_name + billing_address.last_name, or card_holder_name; Nuvei requires paymentOption.card.cardHolderName on every 3DS leg, and on the sandbox it is also the 3DS scenario lever (CL-BRW1 / FL-BRW1).",
                ),
            }
            .into()
        })
}

/// `userTokenId` — the customer identifier Nuvei binds a stored payment option to.
/// Never derived by de-masking the payer email (P-foundation-06).
fn nuvei_three_ds_user_token_id(resource_data: &PaymentFlowData) -> Option<Secret<String>> {
    resource_data
        .connector_customer
        .clone()
        .or_else(|| {
            resource_data
                .customer_id
                .as_ref()
                .map(|customer_id| customer_id.get_string_repr().to_string())
        })
        .map(Secret::new)
}

/// `urlDetails` from the caller's return URL.
fn nuvei_three_ds_url_details(router_return_url: Option<&Url>) -> Option<NuveiUrlDetails> {
    router_return_url.map(|url| NuveiUrlDetails {
        success_url: url.to_string(),
        failure_url: url.to_string(),
        pending_url: url.to_string(),
    })
}

/// `threeD.v2supported` is the string `"true"` / `"false"`, not a JSON boolean
/// (spec "### 4. Initialise a 3DS payment — Response Fields"). An undocumented value
/// is traced and reported as unknown rather than guessed either way.
fn nuvei_v2_supported(v2supported: Option<&str>) -> Option<bool> {
    match v2supported.map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("true") => Some(true),
        Some(value) if value.eq_ignore_ascii_case("false") => Some(false),
        Some("") => None,
        Some(_) => {
            tracing::warn!(
                connector = "nuvei",
                "nuvei: threeD.v2supported carried a value that is neither \"true\" nor \"false\"; treating 3DS 2 support as unknown"
            );
            None
        }
        None => None,
    }
}

/// The `paymentOption.card.threeD` view of whichever leg's response we are reading.
fn nuvei_response_three_d(
    payment_option: Option<&NuveiResponsePaymentOption>,
) -> Option<&NuveiResponseThreeD> {
    payment_option
        .and_then(|option| option.card.as_ref())
        .and_then(|card| card.three_d.as_ref())
}

/// Maps `threeD.result` (`Y`/`N`/`C`/`U`/`A`/`R`) onto the domain transaction status.
/// An undocumented value yields `None` rather than a default verdict.
fn nuvei_trans_status(result: Option<&str>) -> Option<common_enums::TransactionStatus> {
    match result.map(str::trim) {
        Some("Y") => Some(common_enums::TransactionStatus::Success),
        Some("N") => Some(common_enums::TransactionStatus::Failure),
        Some("U") => Some(common_enums::TransactionStatus::VerificationNotPerformed),
        Some("A") => Some(common_enums::TransactionStatus::NotVerified),
        Some("R") => Some(common_enums::TransactionStatus::Rejected),
        Some("C") => Some(common_enums::TransactionStatus::ChallengeRequired),
        _ => None,
    }
}

/// Builds the domain `AuthenticationData` from a Nuvei `threeD` response block.
///
/// `message_version` is parsed, never defaulted: an unparsable version is dropped with
/// a trace rather than silently becoming `0.0.0`.
fn nuvei_authentication_data(
    three_d: Option<&NuveiResponseThreeD>,
) -> Option<domain_types::router_request_types::AuthenticationData> {
    let three_d = three_d?;
    let message_version = three_d.version.as_deref().and_then(|version| {
        match <common_utils::types::SemanticVersion as std::str::FromStr>::from_str(version) {
            Ok(version) => Some(version),
            Err(error) => {
                tracing::debug!(
                    connector = "nuvei",
                    parse_error = ?error,
                    "nuvei: threeD.version is not a semantic version; message_version is left unset"
                );
                None
            }
        }
    });

    let non_empty = |value: Option<&String>| {
        value
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };

    let authentication_data = domain_types::router_request_types::AuthenticationData {
        trans_status: nuvei_trans_status(three_d.result.as_deref()),
        eci: non_empty(three_d.eci.as_ref()),
        cavv: three_d
            .cavv
            .as_ref()
            .map(|cavv| cavv.peek().trim().to_string())
            .filter(|cavv| !cavv.is_empty())
            .map(Secret::new),
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: non_empty(three_d.server_trans_id.as_ref()),
        message_version,
        ds_trans_id: non_empty(three_d.ds_trans_id.as_ref()),
        acs_transaction_id: non_empty(three_d.acs_trans_id.as_ref()),
        transaction_id: None,
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        // `challengePreferenceReason` is documented twice in Nuvei's own material — once
        // as the numeric 1–15 table and once as a string enum — with no published
        // mapping between them, so it is deliberately not surfaced here.
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    };

    // Nothing worth carrying: every member the three legs can fill is empty.
    if authentication_data.trans_status.is_none()
        && authentication_data.eci.is_none()
        && authentication_data.cavv.is_none()
        && authentication_data.threeds_server_transaction_id.is_none()
        && authentication_data.message_version.is_none()
        && authentication_data.ds_trans_id.is_none()
        && authentication_data.acs_transaction_id.is_none()
    {
        return None;
    }
    Some(authentication_data)
}

/// `true` when the response is a failure that must be returned as `Err(ErrorResponse)`
/// rather than `Ok` (INV-11).
///
/// HTTP 200 alone is never success for Nuvei: a 2xx body carrying `status == ERROR`, or
/// `transactionStatus` in (`DECLINED`, `ERROR`), is a failure
/// (spec "### Error-code → connector-error mapping rules").
fn nuvei_three_ds_is_failure(response: &NuveiThreeDSResponse) -> bool {
    matches!(response.status, NuveiPaymentStatus::Error)
        || matches!(
            response.transaction_status,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
        )
}

/// Builds the `ErrorResponse` for a failed 3DS leg.
///
/// A `threeD.flow == "softDecline"` decline is surfaced in the reason rather than
/// retried silently: recovery needs a **new** `/initPayment.do` carrying the declined
/// `transactionId`, which is a caller decision, not a connector one
/// (spec "#### 5e. Soft-decline recovery").
fn nuvei_three_ds_error_response(
    response: &NuveiThreeDSResponse,
    status: common_enums::AttemptStatus,
    http_code: u16,
) -> domain_types::router_data::ErrorResponse {
    let error_fields = nuvei_error_fields(
        &response.status,
        response.transaction_status.as_ref(),
        response.err_code,
        response.reason.as_deref(),
        &response.gateway_error,
    );

    let three_d_flow = nuvei_response_three_d(response.payment_option.as_ref())
        .and_then(|three_d| three_d.flow.as_deref())
        .map(str::trim)
        .filter(|flow| !flow.is_empty());

    let reason = match three_d_flow {
        Some(flow) if flow.eq_ignore_ascii_case("softdecline") => {
            tracing::info!(
                connector = "nuvei",
                "nuvei: 3DS soft decline — the issuer wants SCA; recovery is a new /initPayment.do carrying the declined transactionId"
            );
            Some(match error_fields.reason.as_deref() {
                Some(reason) => format!("{reason} (threeD.flow=softDecline)"),
                None => "threeD.flow=softDecline".to_string(),
            })
        }
        _ => error_fields.reason,
    };

    domain_types::router_data::ErrorResponse {
        code: error_fields.code,
        message: error_fields.message,
        reason,
        status_code: http_code,
        attempt_status: Some(FlowStatus::Payment(status)),
        connector_transaction_id: response.transaction_id.clone(),
        network_advice_code: error_fields.network_advice_code,
        network_decline_code: error_fields.network_decline_code,
        network_error_message: error_fields.network_error_message,
        typed_connector_response: None,
        raw_connector_response: nuvei_raw_response(response),
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

// ---------------------------------------------------------------------------
// ThreeDS wire types
// ---------------------------------------------------------------------------

/// `POST /initPayment.do` — the PreAuthenticate (device-fingerprinting) leg.
///
/// UD-01: **no `checksum` is sent.** `/initPayment.do` is authenticated by the
/// `sessionToken` alone; neither reference implementation sends one, and the documented
/// form is recorded in the techspec's checksum table should Nuvei start enforcing it.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeDSInitRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: Option<String>,
    pub user_token_id: Option<Secret<String>>,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    /// Set only when re-initialising after a soft decline: the declined
    /// `/payment.do` `transactionId` (spec "#### 5e").
    pub related_transaction_id: Option<String>,
    pub payment_option: NuveiPaymentOption<T>,
    pub billing_address: Option<NuveiBillingAddress>,
    pub device_details: NuveiDeviceDetails,
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
}

/// The **first** `POST /payment.do` — the Authenticate leg. Carries
/// `relatedTransactionId` (the `/initPayment.do` `transactionId`) *and* the full
/// `threeD` challenge block; it is the presence of that block that makes Nuvei treat
/// the call as 3DS step 1 (`transactionType` `Auth3D`).
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeDSAuthenticateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: Option<String>,
    pub user_token_id: Option<Secret<String>>,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub payment_option: NuveiPaymentOption<T>,
    pub transaction_type: NuveiTransactionType,
    pub billing_address: Option<NuveiBillingAddress>,
    pub device_details: NuveiDeviceDetails,
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

/// `paymentOption.card` for the PostAuthenticate leg.
///
/// This type exists **only** so that the final `/payment.do` cannot express a `threeD`
/// block: it has no such member, so the omission the spec requires
/// (spec "#### 5c. 3DS step 2 — the `paymentOption.card.threeD` object must NOT be
/// sent") is enforced by construction rather than by a comment. Sending one is
/// `gwErrorCode -1100` / `gwExtendedErrorCode 1155`, "3D Related transaction is missing
/// or incorrect".
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeDSFinalCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    pub card_holder_name: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(rename = "CVV")]
    pub cvv: Secret<String>,
}

/// `paymentOption` for the PostAuthenticate leg — card only, and a card that cannot
/// carry a `threeD` block.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeDSFinalPaymentOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card: NuveiThreeDSFinalCard<T>,
    pub user_payment_option_id: Option<Secret<String>>,
}

/// The **second** `POST /payment.do` — the PostAuthenticate leg. `relatedTransactionId`
/// is the Authenticate leg's `transactionId`, and the whole `threeD` class is omitted.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeDSFinalRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: Option<String>,
    pub user_token_id: Option<Secret<String>>,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub payment_option: NuveiThreeDSFinalPaymentOption<T>,
    pub transaction_type: NuveiTransactionType,
    pub billing_address: Option<NuveiBillingAddress>,
    pub device_details: Option<NuveiDeviceDetails>,
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: Secret<String>,
}

/// The response envelope shared by all three 3DS legs.
///
/// `/initPayment.do` and `/payment.do` return the same envelope — the legs differ only
/// in which members of `paymentOption.card.threeD` are populated — so one struct is
/// parsed everywhere and the three flow-specific names below are aliases of it. That
/// keeps the error model, the status map and the `raw_connector_response` rendering
/// identical across the three legs by construction.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeDSResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub gateway_error: NuveiGatewayError,
    pub auth_code: Option<String>,
    /// `/initPayment.do` refreshes this; the following legs must use the refreshed one.
    pub session_token: Option<Secret<String>>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    #[serde(rename = "paymentOption")]
    pub payment_option: Option<NuveiResponsePaymentOption>,
}

/// `/initPayment.do` response (P-ThreeDS-01).
pub type NuveiThreeDSInitResponse = NuveiThreeDSResponse;
/// First `/payment.do` response (P-ThreeDS-01).
pub type NuveiThreeDSAuthenticateResponse = NuveiThreeDSResponse;
/// Second `/payment.do` response (P-ThreeDS-01).
pub type NuveiThreeDSFinalResponse = NuveiThreeDSResponse;

// ---------------------------------------------------------------------------
// PreAuthenticate — POST /initPayment.do
// ---------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiThreeDSInitRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let resource_data = &router_data.resource_common_data;
        let request = &router_data.request;

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;
        let state = nuvei_three_ds_state(resource_data.connector_feature_data.as_ref());

        // G-ThreeDS-02
        let session_token = nuvei_three_ds_session_token(resource_data, &state)?;
        // G-ThreeDS-01
        let ip_address = nuvei_three_ds_ip_address(request.browser_info.as_ref())?;

        let card = nuvei_three_ds_card(request.payment_method_data.as_ref())?;
        let card_holder_name =
            nuvei_three_ds_card_holder_name(resource_data, card.card_holder_name.as_ref())?;

        let currency = request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: nuvei_error_context(
                    "Send the amount currency; Nuvei's /initPayment.do requires both amount and currency.",
                ),
            })?;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(request.amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Nuvei /initPayment.do takes the amount in decimal major units as a string.",
                ),
            })?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let (client_request_id, client_unique_id) =
            nuvei_client_ids(&resource_data.connector_request_reference_id, &time_stamp)?;

        // The only `threeD` member `/initPayment.do` takes: where the issuer posts the
        // device-fingerprinting result. The challenge block belongs to the next leg.
        let three_d = request.continue_redirection_url.as_ref().map(|url| {
            Box::new(NuveiThreeD {
                method_notification_url: Some(url.to_string()),
                ..Default::default()
            })
        });

        Ok(Self {
            session_token,
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id: Some(client_unique_id),
            user_token_id: nuvei_three_ds_user_token_id(resource_data),
            amount,
            currency,
            // Soft-decline recovery re-initialises with the declined transactionId; on a
            // first attempt the carried state is empty and the member stays off the wire.
            related_transaction_id: state.related_transaction_id.clone(),
            payment_option: NuveiPaymentOption {
                card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                    card_number: card.card_number.clone(),
                    card_holder_name,
                    expiration_month: card.card_exp_month.clone(),
                    expiration_year: card.card_exp_year.clone(),
                    cvv: card.card_cvc.clone(),
                    three_d,
                })),
                alternative_payment_method: None,
                user_payment_option_id: None,
            },
            billing_address: get_billing_address(resource_data, request.email.clone()),
            device_details: NuveiDeviceDetails {
                ip_address: ip_address.clone(),
            },
            url_details: nuvei_three_ds_url_details(request.router_return_url.as_ref()),
            time_stamp,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiThreeDSInitResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiThreeDSInitResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let router_data = item.router_data;
        let response = item.response;
        let raw_connector_response = nuvei_raw_response(&response);

        let status = nuvei_attempt_status(
            response.transaction_status.as_ref(),
            response.transaction_type.as_ref(),
            false,
            &response.status,
        );

        // INV-11: a 2xx that carries a failure is an Err, never an Ok.
        if nuvei_three_ds_is_failure(&response) {
            let error_response = nuvei_three_ds_error_response(&response, status, http_code);
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_response,
                    ..router_data.resource_common_data
                },
                response: Err(error_response),
                ..router_data
            });
        }

        let three_d = nuvei_response_three_d(response.payment_option.as_ref());
        let v2_supported =
            nuvei_v2_supported(three_d.and_then(|three_d| three_d.v2supported.as_deref()));

        // `transactionId` is what the Authenticate leg must send as
        // `relatedTransactionId`; without it the sequence cannot continue at all.
        let transaction_id = response
            .transaction_id
            .clone()
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| {
                tracing::error!(
                    connector = "nuvei",
                    "nuvei: /initPayment.do response carried no transactionId; the 3DS sequence cannot be chained"
                );
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some(
                        "Nuvei's /initPayment.do response carried no transactionId; the Authenticate leg has no relatedTransactionId to send"
                            .to_string(),
                    ),
                ))
            })?;

        let state = NuveiThreeDsState {
            // The refreshed token: the following /payment.do legs must use this one.
            session_token: response.session_token.clone(),
            related_transaction_id: Some(transaction_id.clone()),
            version: three_d
                .and_then(|three_d| three_d.version.clone())
                .filter(|version| !version.trim().is_empty()),
            server_trans_id: three_d
                .and_then(|three_d| three_d.server_trans_id.clone())
                .filter(|id| !id.trim().is_empty()),
            v2_supported,
        };

        // G-ThreeDS-04: `v2supported == "false"` means the card is not enrolled for
        // 3DS 2. The sequence must stop here and the payment fall back to a plain
        // non-3DS /payment.do, so no device-fingerprinting form is emitted and the
        // attempt is left merely Pending rather than awaiting an authentication that
        // will never arrive. The refusal itself is on the Authenticate leg, which is
        // the only place the connector is asked to act on the branch.
        let (status, redirection_data) = if v2_supported == Some(false) {
            tracing::info!(
                connector = "nuvei",
                "nuvei: threeD.v2supported is false — the card is not enrolled for 3DS 2; the 3DS sequence stops after /initPayment.do"
            );
            (common_enums::AttemptStatus::Pending, None)
        } else {
            (
                common_enums::AttemptStatus::AuthenticationPending,
                // The 3DS method (device data collection) form: a POST of
                // `methodPayload` to `methodUrl` under the EMVCo field name
                // `threeDSMethodData`, the same shape airwallex and netcetera use.
                match (
                    three_d.and_then(|three_d| three_d.method_url.clone()),
                    three_d.and_then(|three_d| three_d.method_payload.clone()),
                ) {
                    (Some(method_url), Some(method_payload))
                        if !method_url.trim().is_empty() && !method_payload.trim().is_empty() =>
                    {
                        let mut form_fields = std::collections::HashMap::new();
                        form_fields.insert("threeDSMethodData".to_string(), method_payload);
                        Some(Box::new(RedirectForm::Form {
                            endpoint: method_url,
                            method: Method::Post,
                            form_fields,
                        }))
                    }
                    _ => {
                        tracing::debug!(
                            connector = "nuvei",
                            "nuvei: /initPayment.do returned no methodUrl/methodPayload pair; no device-fingerprinting form is emitted"
                        );
                        None
                    }
                },
            )
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: nuvei_three_ds_state_value(&state).map(Secret::new),
                raw_connector_response,
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(transaction_id)),
                authentication_data: nuvei_authentication_data(three_d),
                redirection_data,
                connector_response_reference_id: response
                    .order_id
                    .clone()
                    .filter(|id| !id.trim().is_empty()),
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

// ---------------------------------------------------------------------------
// Authenticate — the first POST /payment.do
// ---------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiThreeDSAuthenticateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let resource_data = &router_data.resource_common_data;
        let request = &router_data.request;

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;
        let state = nuvei_three_ds_state(resource_data.connector_feature_data.as_ref());

        // G-ThreeDS-04: `/initPayment.do` said the card is not enrolled for 3DS 2.
        // Continuing the sequence on a non-enrolled card is a guaranteed reject, so the
        // leg refuses instead of sending it; the caller falls back to a plain
        // non-3DS /payment.do.
        if state.v2_supported == Some(false) {
            return Err(IntegrationError::NotSupported {
                message: "3DS 2 authentication on a card whose /initPayment.do returned threeD.v2supported=false".to_string(),
                connector: "nuvei",
                context: nuvei_error_context(
                    "The card is not enrolled for 3DS 2; run the payment through PaymentService/Authorize without 3DS instead of continuing the authentication sequence.",
                ),
            }
            .into());
        }

        // G-ThreeDS-02
        let session_token = nuvei_three_ds_session_token(resource_data, &state)?;
        // G-ThreeDS-03
        let related_transaction_id =
            nuvei_related_transaction_id(&state, resource_data.reference_id.as_deref())?;
        // G-ThreeDS-01
        let ip_address = nuvei_three_ds_ip_address(request.browser_info.as_ref())?;

        let card = nuvei_three_ds_card(request.payment_method_data.as_ref())?;
        let card_holder_name =
            nuvei_three_ds_card_holder_name(resource_data, card.card_holder_name.as_ref())?;

        let currency = request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
            field_name: "currency",
            context: nuvei_error_context(
                "Send the amount currency; Nuvei's /payment.do requires both amount and currency.",
            ),
        })?;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(request.amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Nuvei /payment.do takes the amount in decimal major units as a string.",
                ),
            })?;

        // `notificationURL` is where the ACS posts the CRes once the challenge is done,
        // which is exactly the caller's continue-redirection URL. It is mandatory.
        let notification_url = request
            .continue_redirection_url
            .as_ref()
            .map(|url| url.to_string())
            .or_else(|| request.webhook_url.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: nuvei_error_context(
                    "Send continue_redirection_url (or webhook_url); Nuvei requires threeD.notificationURL — it is where the ACS posts the CRes after the challenge.",
                ),
            })?;

        let three_d = NuveiThreeD {
            method_completion_ind: Some(nuvei_method_completion_ind(
                request.redirect_response.as_ref(),
            )),
            // Echo the version `/initPayment.do` negotiated; Nuvei rejects a mismatch.
            version: state.version.clone(),
            notification_url: Some(notification_url),
            merchant_url: resource_data.return_url.clone(),
            platform_type: Some(nuvei_platform_type(request.device_channel.as_ref())),
            v2_additional_params: Some(NuveiV2AdditionalParams {
                // `05` = full screen, the only size that renders every issuer's
                // challenge page without clipping.
                challenge_window_size: Some("05".to_string()),
                // `rebillExpiry` / `rebillFrequency` are required only when
                // `isRebilling == "0"`, which the authentication legs never send:
                // establishing a credential on file is the SetupMandate flow's job.
                rebill_expiry: None,
                rebill_frequency: None,
            }),
            browser_details: Some(nuvei_browser_details(
                request.browser_info.as_ref(),
                ip_address.clone(),
            )),
            method_notification_url: None,
            external_mpi: None,
        };

        let time_stamp = NuveiAuthType::get_timestamp();
        let (client_request_id, client_unique_id) =
            nuvei_client_ids(&resource_data.connector_request_reference_id, &time_stamp)?;

        // /payment.do checksum: merchantId + merchantSiteId + clientRequestId + amount +
        // currency + timeStamp + merchantSecretKey (techspec checksum table, row 4).
        let checksum = auth.generate_checksum(
            "payment.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            session_token,
            merchant_id: auth.merchant_id.clone(),
            merchant_site_id: auth.merchant_site_id.clone(),
            client_request_id,
            client_unique_id: Some(client_unique_id),
            user_token_id: nuvei_three_ds_user_token_id(resource_data),
            amount,
            currency,
            related_transaction_id,
            payment_option: NuveiPaymentOption {
                card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                    card_number: card.card_number.clone(),
                    card_holder_name,
                    expiration_month: card.card_exp_month.clone(),
                    expiration_year: card.card_exp_year.clone(),
                    cvv: card.card_cvc.clone(),
                    three_d: Some(Box::new(three_d)),
                })),
                alternative_payment_method: None,
                user_payment_option_id: None,
            },
            transaction_type: NuveiTransactionType::get_from_capture_method(
                request.capture_method,
                request.amount,
            ),
            billing_address: get_billing_address(resource_data, request.email.clone()),
            device_details: NuveiDeviceDetails {
                ip_address: ip_address.clone(),
            },
            url_details: nuvei_three_ds_url_details(request.router_return_url.as_ref()),
            time_stamp,
            checksum,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiThreeDSAuthenticateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiThreeDSAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let router_data = item.router_data;
        let response = item.response;
        let raw_connector_response = nuvei_raw_response(&response);
        let mut state = nuvei_three_ds_state(
            router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        );

        let status = nuvei_attempt_status(
            response.transaction_status.as_ref(),
            response.transaction_type.as_ref(),
            false,
            &response.status,
        );

        // A `DECLINED` carrying `threeD.flow == "softDecline"` lands here: the shared
        // composite map already turns (DECLINED, Auth3D) into AuthenticationFailed, and
        // the soft-decline marker is surfaced in the error reason rather than retried.
        if nuvei_three_ds_is_failure(&response) {
            let error_response = nuvei_three_ds_error_response(&response, status, http_code);
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_response,
                    ..router_data.resource_common_data
                },
                response: Err(error_response),
                ..router_data
            });
        }

        let three_d = nuvei_response_three_d(response.payment_option.as_ref());

        // `transactionId` is the PostAuthenticate leg's `relatedTransactionId`.
        let transaction_id = response
            .transaction_id
            .clone()
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| {
                tracing::error!(
                    connector = "nuvei",
                    "nuvei: the 3DS authenticate /payment.do carried no transactionId; the final leg has nothing to chain to"
                );
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some(
                        "Nuvei's 3DS authenticate /payment.do response carried no transactionId; the PostAuthenticate leg has no relatedTransactionId to send"
                            .to_string(),
                    ),
                ))
            })?;

        // The challenge redirect: a POST form of `creq` = `cReq` to `acsUrl`, built by
        // the shared helper so the Authorize flow and this leg cannot drift apart.
        let redirection_data = nuvei_redirect_form(response.payment_option.as_ref())?;

        // P-ThreeDS-07: on this leg an APPROVED with no challenge to run *is* the
        // frictionless authentication result, which is more specific than the shared
        // composite map's (APPROVED, Auth3D) => AuthenticationPending.
        let status = match (&response.transaction_status, redirection_data.is_some()) {
            (Some(NuveiTransactionStatus::Approved), false) => {
                common_enums::AttemptStatus::AuthenticationSuccessful
            }
            _ => status,
        };

        state.related_transaction_id = Some(transaction_id.clone());
        if let Some(session_token) = response.session_token.clone() {
            state.session_token = Some(session_token);
        }
        if let Some(version) = three_d
            .and_then(|three_d| three_d.version.clone())
            .filter(|version| !version.trim().is_empty())
        {
            state.version = Some(version);
        }
        if let Some(server_trans_id) = three_d
            .and_then(|three_d| three_d.server_trans_id.clone())
            .filter(|id| !id.trim().is_empty())
        {
            state.server_trans_id = Some(server_trans_id);
        }

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: nuvei_connector_response_data(
                    response.payment_option.as_ref(),
                    response.auth_code.as_deref(),
                ),
                raw_connector_response,
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(transaction_id)),
                redirection_data,
                authentication_data: nuvei_authentication_data(three_d),
                connector_feature_data: nuvei_three_ds_state_value(&state),
                connector_response_reference_id: response
                    .order_id
                    .clone()
                    .filter(|id| !id.trim().is_empty()),
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

// ---------------------------------------------------------------------------
// PostAuthenticate — the second POST /payment.do, with `threeD` omitted
// ---------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiThreeDSFinalRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let resource_data = &router_data.resource_common_data;
        let request = &router_data.request;

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;
        let state = nuvei_three_ds_state(resource_data.connector_feature_data.as_ref());

        // G-ThreeDS-02
        let session_token = nuvei_three_ds_session_token(resource_data, &state)?;
        // G-ThreeDS-03: the Authenticate leg's transactionId, carried either in the 3DS
        // state or as the composite service's `connector_order_reference_id`.
        let related_transaction_id = nuvei_related_transaction_id(
            &state,
            request
                .connector_order_reference_id
                .as_deref()
                .or(resource_data.reference_id.as_deref()),
        )?;

        let card = nuvei_three_ds_card(request.payment_method_data.as_ref())?;
        let card_holder_name =
            nuvei_three_ds_card_holder_name(resource_data, card.card_holder_name.as_ref())?;

        let currency = request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
            field_name: "currency",
            context: nuvei_error_context(
                "Send the amount currency; Nuvei's /payment.do requires both amount and currency.",
            ),
        })?;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(request.amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Nuvei /payment.do takes the amount in decimal major units as a string.",
                ),
            })?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let (client_request_id, client_unique_id) =
            nuvei_client_ids(&resource_data.connector_request_reference_id, &time_stamp)?;

        let checksum = auth.generate_checksum(
            "payment.do",
            &[
                ("merchantId", auth.merchant_id.peek()),
                ("merchantSiteId", auth.merchant_site_id.peek()),
                ("clientRequestId", &client_request_id),
                ("amount", &amount.get_amount_as_string()),
                ("currency", &currency.to_string()),
                ("timeStamp", &time_stamp.to_string()),
            ],
        );

        Ok(Self {
            session_token,
            merchant_id: auth.merchant_id.clone(),
            merchant_site_id: auth.merchant_site_id.clone(),
            client_request_id,
            client_unique_id: Some(client_unique_id),
            user_token_id: nuvei_three_ds_user_token_id(resource_data),
            amount,
            currency,
            related_transaction_id,
            // `NuveiThreeDSFinalCard` has no `threeD` member at all, so the omission the
            // spec requires on this leg cannot be undone by a later edit.
            payment_option: NuveiThreeDSFinalPaymentOption {
                card: NuveiThreeDSFinalCard {
                    card_number: card.card_number.clone(),
                    card_holder_name,
                    expiration_month: card.card_exp_month.clone(),
                    expiration_year: card.card_exp_year.clone(),
                    cvv: card.card_cvc.clone(),
                },
                user_payment_option_id: None,
            },
            transaction_type: NuveiTransactionType::get_from_capture_method(
                request.capture_method,
                request.amount,
            ),
            billing_address: get_billing_address(resource_data, request.email.clone()),
            // The cardholder is no longer at the browser on the final leg, so
            // `deviceDetails` is sent only when the caller still has the IP.
            device_details: request
                .browser_info
                .as_ref()
                .and_then(|info| info.ip_address)
                .map(|ip| NuveiDeviceDetails {
                    ip_address: Secret::new(ip.to_string()),
                }),
            url_details: nuvei_three_ds_url_details(request.router_return_url.as_ref()),
            time_stamp,
            checksum,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiThreeDSFinalResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiThreeDSFinalResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let router_data = item.router_data;
        let response = item.response;
        let raw_connector_response = nuvei_raw_response(&response);
        let mut state = nuvei_three_ds_state(
            router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        );

        let status = nuvei_attempt_status(
            response.transaction_status.as_ref(),
            response.transaction_type.as_ref(),
            false,
            &response.status,
        );

        if nuvei_three_ds_is_failure(&response) {
            let error_response = nuvei_three_ds_error_response(&response, status, http_code);
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_response,
                    ..router_data.resource_common_data
                },
                response: Err(error_response),
                ..router_data
            });
        }

        // This is the leg that carries the liability-shift proof: `threeD.cavv`,
        // `threeD.eci` (5 Visa / 2 Mastercard, 7 for 3RI), `threeD.result = Y` and
        // `threeD.isLiabilityOnIssuer = "1"`.
        let three_d = nuvei_response_three_d(response.payment_option.as_ref());
        if three_d
            .and_then(|three_d| three_d.is_liability_on_issuer.as_deref())
            .map(str::trim)
            != Some("1")
        {
            tracing::info!(
                connector = "nuvei",
                "nuvei: the 3DS final /payment.do did not report threeD.isLiabilityOnIssuer=1; the attempt carries no liability shift"
            );
        }

        if let Some(transaction_id) = response
            .transaction_id
            .clone()
            .filter(|id| !id.trim().is_empty())
        {
            state.related_transaction_id = Some(transaction_id);
        }
        if let Some(session_token) = response.session_token.clone() {
            state.session_token = Some(session_token);
        }

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: nuvei_three_ds_state_value(&state).map(Secret::new),
                connector_response: nuvei_connector_response_data(
                    response.payment_option.as_ref(),
                    response.auth_code.as_deref(),
                ),
                raw_connector_response,
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data: nuvei_authentication_data(three_d),
                connector_response_reference_id: response
                    .order_id
                    .clone()
                    .filter(|id| !id.trim().is_empty()),
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

// ============================================================================
// IncomingWebhook — Direct Merchant Notifications (DMN)
// ============================================================================
//
// Nuvei does not have one webhook contract, it has three
// (spec "### The three DMN families — three different bodies, three different
// signatures"). They disagree on the body format, on the checksum input, and on
// whether the merchant secret is prepended or appended:
//
//   (a) Payment DMN          `application/x-www-form-urlencoded` (POST body) or a GET
//                            query string. Signature `advanceResponseChecksum`, a body
//                            parameter. Input: secret + totalAmount + currency +
//                            responseTimeStamp + PPP_TransactionID + Status + productId,
//                            no separators, secret PREPENDED. SHA-256 *or* MD5 per SiteId
//                            (UD-09) — both are tried. `+` decodes back to a space before
//                            hashing.
//   (b) Control Panel Event  JSON body, checksum in an HTTP header whose name Nuvei never
//                            publishes (UD-10 / spec gap G-04). Input: secret + every JSON
//                            value concatenated in payload order, SHA-256 over UTF-8.
//                            THIS is where chargebacks live.
//   (c) Withdrawal DMN       form-urlencoded, `checksum` body parameter. Input: every
//                            parameter NAME and value in the order sent, secret APPENDED.
//
// There is no dedicated refund DMN and no dedicated dispute DMN in family (a): a refund
// arrives as `transactionType = Credit` and a dispute as `transactionType = Chargeback`.
//
// DMN parameter names are *not* the synchronous `/payment.do` response names and the DMN
// status vocabulary is *not* the synchronous one, so none of the types below reuse the
// Authorize response structs or `nuvei_attempt_status`.

/// The deprecated predecessor of `advanceResponseChecksum`. Nuvei publishes no formula
/// for it, so it is never used for verification — only named here so the reason is
/// visible at the call site.
pub const NUVEI_DEPRECATED_CHECKSUM_PARAM: &str = "responseChecksum";

/// `productId`'s contribution to the Payment DMN checksum when the merchant sent none
/// (UD-04): the literal `NA`, matching the hyperswitch reference.
const NUVEI_DMN_ABSENT_PRODUCT_ID: &str = "NA";

/// The HTTP header carrying the family-(b) checksum. Nuvei does not publish the name
/// (spec gap G-04 / UD-10), so the lookup is case-insensitive over this value and the
/// header that actually arrived is traced.
const NUVEI_CONTROL_PANEL_CHECKSUM_HEADER: &str = "checksum";

/// The family-(c) body parameter carrying the Withdrawal DMN digest. It is excluded from
/// its own pre-image (RV-008).
const NUVEI_WITHDRAWAL_CHECKSUM_PARAM: &str = "checksum";

/// The fiscal `Status` of a Payment DMN.
///
/// Deliberately **not** [`NuveiTransactionStatus`]: the DMN vocabulary carries `SUCCESS`
/// and `UPDATE`, which the synchronous `transactionStatus` does not have, and no Nuvei
/// page maps the two vocabularies onto each other (spec gap G-10 / UD-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum NuveiDmnStatus {
    #[serde(rename = "APPROVED", alias = "Approved", alias = "approved")]
    Approved,
    /// Treated exactly as `APPROVED` (UD-18).
    #[serde(rename = "SUCCESS", alias = "Success", alias = "success")]
    Success,
    #[serde(rename = "DECLINED", alias = "Declined", alias = "declined")]
    Declined,
    #[serde(rename = "ERROR", alias = "Error", alias = "error")]
    Error,
    #[serde(rename = "PENDING", alias = "Pending", alias = "pending")]
    Pending,
    /// North-American bank-transfer status update. Non-terminal (UD-18).
    #[serde(rename = "UPDATE", alias = "Update", alias = "update")]
    Update,
    /// An undocumented `Status`. The event is refused rather than guessed.
    #[serde(other)]
    Unknown,
}

/// Family (a): the Payment DMN, decoded from the form body or the GET query string.
///
/// Only the members this connector acts on are modelled; serde ignores the rest of the
/// (very large) DMN parameter set. The checksum is **not** computed from these fields —
/// it is computed from [`NuveiDmnRawParams`], which preserves the verbatim wire spelling
/// and ordering.
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NuveiPaymentDmn {
    /// Notification-layer status: `OK`, `PENDING`, `FAIL`.
    pub ppp_status: Option<String>,
    /// Fiscal status. Participates in the checksum.
    #[serde(rename = "Status")]
    pub status: Option<NuveiDmnStatus>,
    #[serde(rename = "transactionType")]
    pub transaction_type: Option<NuveiTransactionType>,
    /// The fiscal transaction id — the value `/payment.do` returns as `transactionId`.
    #[serde(rename = "TransactionID", alias = "TransactionId")]
    pub transaction_id: Option<String>,
    /// The payment-page id. Participates in the checksum; it is *not* the fiscal id.
    #[serde(rename = "PPP_TransactionID", alias = "PPP_TransactionId")]
    pub ppp_transaction_id: Option<String>,
    #[serde(rename = "totalAmount")]
    pub total_amount: Option<StringMajorUnit>,
    pub currency: Option<String>,
    /// `YYYY-MM-DD.HH:mm:ss` GMT, used verbatim. Participates in the checksum.
    #[serde(rename = "responseTimeStamp")]
    pub response_time_stamp: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<String>,
    #[serde(rename = "advanceResponseChecksum")]
    pub advance_response_checksum: Option<Secret<String>>,
    #[serde(rename = "clientUniqueId")]
    pub client_unique_id: Option<String>,
    #[serde(rename = "clientRequestId")]
    pub client_request_id: Option<String>,
    pub merchant_unique_id: Option<String>,
    /// The payment a `Credit` (refund) or `Chargeback` belongs to.
    #[serde(rename = "relatedTransactionId")]
    pub related_transaction_id: Option<String>,
    #[serde(rename = "transactionLinkId")]
    pub transaction_link_id: Option<String>,
    #[serde(rename = "userPaymentOptionId")]
    pub user_payment_option_id: Option<String>,
    #[serde(rename = "ErrCode")]
    pub err_code: Option<String>,
    #[serde(rename = "ExErrCode")]
    pub ex_err_code: Option<String>,
    #[serde(rename = "Reason")]
    pub reason: Option<String>,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    pub message: Option<String>,
    #[serde(rename = "AuthCode")]
    pub auth_code: Option<String>,
    #[serde(rename = "AvsCode")]
    pub avs_code: Option<String>,
    #[serde(rename = "Cvv2Reply")]
    pub cvv2_reply: Option<String>,
    #[serde(rename = "merchantAdviceCode")]
    pub merchant_advice_code: Option<String>,
    /// See spec gap G-06: read opportunistically, never fabricated.
    #[serde(rename = "externalSchemeTransactionId")]
    pub external_scheme_transaction_id: Option<String>,
}

/// The verbatim, order-preserving `(name, value)` list of a form-encoded DMN.
///
/// The checksum of families (a) and (c) is computed from this, not from
/// [`NuveiPaymentDmn`]: family (a) hashes the wire spelling of `Status` (which a typed
/// enum would have normalised) and family (c) hashes parameter *names* as well as values,
/// in the order they were sent. `serde_urlencoded` has already decoded `+` back to a
/// space and resolved percent-escapes, which is exactly the pre-processing the Payment
/// DMN checksum requires.
#[derive(Debug, Clone, Default)]
pub struct NuveiDmnRawParams(Vec<(String, String)>);

impl NuveiDmnRawParams {
    /// Case-insensitive lookup; the first occurrence wins.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Every `name=value` pair in the order sent, `=` included, except the pairs whose
    /// name matches `exclude` (case-insensitively) — the family-(c) input.
    ///
    /// RV-008: the excluded name is the `checksum` parameter itself. A digest can never
    /// be part of its own pre-image, so including it made every Withdrawal DMN fail
    /// verification unconditionally.
    fn names_and_values_excluding(&self, exclude: &str) -> String {
        self.0
            .iter()
            .filter(|(key, _)| !key.eq_ignore_ascii_case(exclude))
            .map(|(key, value)| format!("{key}={value}"))
            .collect()
    }
}

/// Family (b): the Control Panel Event DMN envelope.
///
/// This is where chargebacks, pre-chargeback (Ethoca) alerts, RDR external alerts,
/// fraud-reported transactions and manual corrections arrive. Nuvei documents the
/// envelope below but never enumerates the event-specific nested members, and Retrieval
/// Request has no DMN event type at all (spec gap G-04), so the nested payload is kept as
/// raw JSON rather than modelled into fields that would be invented.
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NuveiControlPanelEvent {
    #[serde(rename = "EventId")]
    pub event_id: Option<String>,
    #[serde(rename = "EventType")]
    pub event_type: Option<String>,
    #[serde(rename = "EventDate")]
    pub event_date: Option<String>,
    #[serde(rename = "EventDateUTC")]
    pub event_date_utc: Option<String>,
    /// The retry counter — the only ordering signal Nuvei publishes (spec gap G-11).
    #[serde(rename = "AttemptNumber")]
    pub attempt_number: Option<i32>,
    #[serde(rename = "EventCorrelationId")]
    pub event_correlation_id: Option<String>,
    #[serde(rename = "ClientId")]
    pub client_id: Option<serde_json::Value>,
    #[serde(rename = "ClientName")]
    pub client_name: Option<String>,
    #[serde(rename = "ProcessingEntityType")]
    pub processing_entity_type: Option<String>,
    #[serde(rename = "ProcessingEntityId")]
    pub processing_entity_id: Option<serde_json::Value>,
    /// Every member Nuvei did not document, kept verbatim so the disputed transaction id,
    /// amount and currency can be read opportunistically without inventing a schema.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl NuveiControlPanelEvent {
    /// Case-insensitive lookup over the undocumented nested members, one level deep.
    pub fn extra_str(&self, names: &[&str]) -> Option<String> {
        fn as_str(value: &serde_json::Value) -> Option<String> {
            match value {
                serde_json::Value::String(value) => Some(value.clone()),
                serde_json::Value::Number(value) => Some(value.to_string()),
                _ => None,
            }
        }
        fn find(
            map: &serde_json::Map<String, serde_json::Value>,
            names: &[&str],
            depth: u8,
        ) -> Option<String> {
            for (key, value) in map {
                if names.iter().any(|name| key.eq_ignore_ascii_case(name)) {
                    if let Some(found) = as_str(value) {
                        return Some(found);
                    }
                }
                if let (Some(nested), true) = (value.as_object(), depth > 0) {
                    if let Some(found) = find(nested, names, depth - 1) {
                        return Some(found);
                    }
                }
            }
            None
        }
        find(&self.extra, names, 1)
    }

    /// Nuvei's Control Panel `EventType` values, normalised for matching.
    fn normalised_event_type(&self) -> Option<String> {
        self.event_type
            .as_deref()
            .map(|value| value.trim().to_ascii_lowercase())
    }

    /// Whether this event belongs to the chargeback / dispute family.
    pub fn is_dispute_event(&self) -> bool {
        self.normalised_event_type().is_some_and(|event_type| {
            event_type.contains("chargeback")
                || event_type.contains("dispute")
                || event_type.contains("rdr")
                || event_type.contains("fraud reported")
                || event_type.contains("retrieval")
        })
    }
}

/// Family (c): the Withdrawal DMN.
///
/// Out of this run's card-only scope for *processing*, but it is modelled and recognised
/// so that a withdrawal notification is classified rather than misread as a Payment
/// DMN whose checksum would then never verify.
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NuveiWithdrawalDmn {
    #[serde(rename = "settlementType")]
    pub settlement_type: Option<String>,
    pub checksum: Option<Secret<String>>,
    #[serde(rename = "wdRequestId")]
    pub wd_request_id: Option<String>,
    pub status: Option<String>,
}

/// One inbound DMN, discriminated by family.
#[derive(Debug, Clone)]
pub enum NuveiDmn {
    Payment(Box<NuveiPaymentDmn>, NuveiDmnRawParams),
    ControlPanelEvent(Box<NuveiControlPanelEvent>),
    Withdrawal(Box<NuveiWithdrawalDmn>, NuveiDmnRawParams),
}

/// Decode an inbound DMN into its family.
///
/// A POST carries the parameters in the body; a GET carries them in the query string with
/// an empty body, and both are supported. JSON is family (b); a form body carrying
/// `settlementType` is family (c); anything else that parses as a form is family (a).
/// G-IncomingWebhook-05: a body that matches none of the three is refused, never
/// half-parsed.
pub fn nuvei_parse_dmn(
    body: &[u8],
    query_params: Option<&str>,
) -> Result<NuveiDmn, Report<domain_types::errors::WebhookError>> {
    use domain_types::errors::WebhookError;

    // A DMN is delivered as a POST body or, when the account is configured for it, as a
    // GET query string with an empty body. Both carry the same parameters.
    let payload: &[u8] = match (body.is_empty(), query_params) {
        (false, _) => body,
        (true, Some(query_params)) => query_params.as_bytes(),
        (true, None) => b"",
    };

    if payload.is_empty() {
        tracing::warn!(
            connector = "nuvei",
            "nuvei webhook: empty DMN body and empty query string"
        );
        return Err(Report::new(WebhookError::WebhookBodyDecodingFailed));
    }

    // Family (b) is the only JSON family.
    if payload.iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b'{') {
        let event: NuveiControlPanelEvent = serde_json::from_slice(payload).map_err(|error| {
            tracing::warn!(
                connector = "nuvei",
                error = %error,
                "nuvei webhook: JSON body is not a Control Panel Event"
            );
            Report::new(WebhookError::WebhookBodyDecodingFailed)
        })?;
        return Ok(NuveiDmn::ControlPanelEvent(Box::new(event)));
    }

    let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(payload).map_err(|error| {
        tracing::warn!(
            connector = "nuvei",
            error = %error,
            "nuvei webhook: body is neither JSON nor form-urlencoded"
        );
        Report::new(WebhookError::WebhookBodyDecodingFailed)
    })?;
    let raw = NuveiDmnRawParams(pairs);

    if raw.get("settlementType").is_some() {
        let withdrawal: NuveiWithdrawalDmn =
            serde_urlencoded::from_bytes(payload).map_err(|error| {
                tracing::warn!(
                    connector = "nuvei",
                    error = %error,
                    "nuvei webhook: withdrawal DMN did not decode"
                );
                Report::new(WebhookError::WebhookBodyDecodingFailed)
            })?;
        return Ok(NuveiDmn::Withdrawal(Box::new(withdrawal), raw));
    }

    let dmn: NuveiPaymentDmn = serde_urlencoded::from_bytes(payload).map_err(|error| {
        tracing::warn!(
            connector = "nuvei",
            error = %error,
            "nuvei webhook: payment DMN did not decode"
        );
        Report::new(WebhookError::WebhookBodyDecodingFailed)
    })?;
    Ok(NuveiDmn::Payment(Box::new(dmn), raw))
}

/// The family-(a) checksum pre-image: the merchant secret **prepended** to
/// `totalAmount + currency + responseTimeStamp + PPP_TransactionID + Status + productId`,
/// with no separators (spec "### Critical notes on row 10 (DMN)").
///
/// The six values are taken from the raw wire parameters so that `Status` keeps its
/// verbatim spelling; `+` has already been decoded back to a space by the form decoder.
/// An absent `productId` contributes the literal `NA` (UD-04).
pub fn nuvei_payment_dmn_checksum_message(raw: &NuveiDmnRawParams, secret: &str) -> String {
    let mut message = String::from(secret);
    // The pre-image is the concatenation of the six values *as they arrived*: a parameter
    // the DMN did not carry contributes nothing, which is not a fallback for a missing
    // value but the definition of the concatenation.
    for name in [
        "totalAmount",
        "currency",
        "responseTimeStamp",
        "PPP_TransactionID",
        "Status",
    ] {
        if let Some(value) = raw.get(name) {
            message.push_str(value);
        }
    }
    // `productId` is the one exception: an absent parameter contributes the literal `NA`
    // (UD-04), matching the hyperswitch reference, which is the only implementation
    // reported working against real Nuvei DMNs. A present-but-empty `productId=`
    // contributes the empty string, as it does in that reference.
    match raw.get("productId") {
        Some(product_id) => message.push_str(product_id),
        None => message.push_str(NUVEI_DMN_ABSENT_PRODUCT_ID),
    }
    message
}

/// The family-(b) checksum pre-image: the merchant secret **prepended** to every JSON
/// value concatenated in payload order, SHA-256 over UTF-8.
///
/// `serde_json` is built with `preserve_order` in this crate, so the object iteration
/// order is the payload order.
pub fn nuvei_control_panel_checksum_message(
    body: &[u8],
    secret: &str,
) -> Result<String, Report<domain_types::errors::WebhookError>> {
    use domain_types::errors::WebhookError;

    fn push_values(value: &serde_json::Value, out: &mut String) {
        match value {
            serde_json::Value::Null => {}
            serde_json::Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            serde_json::Value::Number(value) => out.push_str(&value.to_string()),
            serde_json::Value::String(value) => out.push_str(value),
            serde_json::Value::Array(values) => {
                values.iter().for_each(|value| push_values(value, out))
            }
            serde_json::Value::Object(map) => {
                map.values().for_each(|value| push_values(value, out))
            }
        }
    }

    let value: serde_json::Value = serde_json::from_slice(body)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("nuvei: control panel event body is not JSON")?;
    let mut message = String::from(secret);
    push_values(&value, &mut message);
    Ok(message)
}

/// The family-(c) checksum pre-image: every parameter name and value in the order sent,
/// `=` included, with the merchant secret **appended**.
///
/// The `checksum` parameter carrying the digest under verification is excluded: it is not
/// part of its own pre-image (RV-008). Everything else the DMN carried is kept verbatim,
/// in wire order.
pub fn nuvei_withdrawal_dmn_checksum_message(raw: &NuveiDmnRawParams, secret: &str) -> String {
    let mut message = raw.names_and_values_excluding(NUVEI_WITHDRAWAL_CHECKSUM_PARAM);
    message.push_str(secret);
    message
}

/// The header name family (b) actually used, looked up case-insensitively because Nuvei
/// never published it (UD-10 / spec gap G-04). Returns the `(name, value)` that arrived
/// so the observed spelling can be traced and confirmed against a real chargeback DMN.
pub fn nuvei_control_panel_checksum_header(
    headers: &std::collections::HashMap<String, String>,
) -> Option<(&str, &str)> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(NUVEI_CONTROL_PANEL_CHECKSUM_HEADER))
        .map(|(name, value)| (name.as_str(), value.as_str()))
}

/// The DMN event, as `(Status × transactionType)` (spec "### Event → status mapping").
///
/// Exhaustive on both axes: there is no `_ =>` arm on `NuveiDmnStatus`, and an
/// undocumented value arrives as [`NuveiDmnStatus::Unknown`], which the caller refuses.
/// `SUCCESS` is `APPROVED` and `UPDATE` is non-terminal processing (UD-18); `Settle` is
/// mapped although no settle DMN has been observed (UD-14).
pub fn nuvei_dmn_event_type(
    status: NuveiDmnStatus,
    transaction_type: NuveiTransactionType,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;

    // A chargeback is a dispute whatever the fiscal status says.
    if matches!(transaction_type, NuveiTransactionType::Chargeback) {
        return EventType::DisputeOpened;
    }

    match status {
        NuveiDmnStatus::Approved | NuveiDmnStatus::Success => match transaction_type {
            NuveiTransactionType::Auth
            | NuveiTransactionType::PreAuth
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::VerifyAuth3D => EventType::PaymentIntentAuthorizationSuccess,
            NuveiTransactionType::Sale => EventType::PaymentIntentSuccess,
            NuveiTransactionType::Settle => EventType::PaymentIntentCaptureSuccess,
            NuveiTransactionType::Void => EventType::PaymentIntentCancelled,
            NuveiTransactionType::Credit | NuveiTransactionType::VoidCredit => {
                EventType::RefundSuccess
            }
            NuveiTransactionType::Chargeback => EventType::DisputeOpened,
            NuveiTransactionType::Modification | NuveiTransactionType::Unknown => {
                EventType::IncomingWebhookEventUnspecified
            }
        },
        NuveiDmnStatus::Declined | NuveiDmnStatus::Error => match transaction_type {
            NuveiTransactionType::Auth
            | NuveiTransactionType::PreAuth
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::VerifyAuth3D => EventType::PaymentIntentAuthorizationFailure,
            NuveiTransactionType::Sale => EventType::PaymentIntentFailure,
            NuveiTransactionType::Settle => EventType::PaymentIntentCaptureFailure,
            NuveiTransactionType::Void => EventType::PaymentIntentCancelFailure,
            NuveiTransactionType::Credit | NuveiTransactionType::VoidCredit => {
                EventType::RefundFailure
            }
            NuveiTransactionType::Chargeback => EventType::DisputeOpened,
            NuveiTransactionType::Modification | NuveiTransactionType::Unknown => {
                EventType::IncomingWebhookEventUnspecified
            }
        },
        NuveiDmnStatus::Pending => match transaction_type {
            NuveiTransactionType::Credit | NuveiTransactionType::VoidCredit => {
                EventType::RefundProcessing
            }
            NuveiTransactionType::Auth
            | NuveiTransactionType::PreAuth
            | NuveiTransactionType::Sale
            | NuveiTransactionType::Settle
            | NuveiTransactionType::Void
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::VerifyAuth3D => EventType::PaymentIntentProcessing,
            NuveiTransactionType::Chargeback => EventType::DisputeOpened,
            NuveiTransactionType::Modification | NuveiTransactionType::Unknown => {
                EventType::IncomingWebhookEventUnspecified
            }
        },
        // UD-18: the North-American bank-transfer status update. Non-terminal.
        NuveiDmnStatus::Update => EventType::PaymentIntentProcessing,
        NuveiDmnStatus::Unknown => EventType::IncomingWebhookEventUnspecified,
    }
}

/// The dispute event a Control Panel `EventType` carries (family b).
///
/// Nuvei's published list is `Chargeback`, `Pre-Chargeback Alert`, `RDR External Alert`,
/// `Fraud reported Transaction`, `Manual Correction`, `Dispute API Callbacks`, ... — the
/// resolution-bearing spellings below are matched on substrings because Nuvei documents
/// the family but not an exhaustive value set (spec gap G-04).
pub fn nuvei_control_panel_event_type(
    event: &NuveiControlPanelEvent,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;

    let Some(event_type) = event.normalised_event_type() else {
        tracing::warn!(
            connector = "nuvei",
            "nuvei webhook: control panel event carried no EventType"
        );
        return EventType::IncomingWebhookEventUnspecified;
    };

    // Order matters: the resolution spellings are checked before the generic ones.
    if event_type.contains("won") || event_type.contains("reversal") {
        EventType::DisputeWon
    } else if event_type.contains("lost") {
        EventType::DisputeLost
    } else if event_type.contains("accepted") {
        EventType::DisputeAccepted
    } else if event_type.contains("challenge") || event_type.contains("represent") {
        EventType::DisputeChallenged
    } else if event_type.contains("cancel") {
        EventType::DisputeCancelled
    } else if event_type.contains("expired") {
        EventType::DisputeExpired
    } else if event.is_dispute_event() {
        EventType::DisputeOpened
    } else {
        tracing::info!(
            connector = "nuvei",
            "nuvei webhook: control panel event outside the dispute family, unspecified"
        );
        EventType::IncomingWebhookEventUnspecified
    }
}

/// The dispute status a family-(b) event carries.
pub fn nuvei_control_panel_dispute_status(
    event: &NuveiControlPanelEvent,
) -> common_enums::DisputeStatus {
    use common_enums::DisputeStatus;
    use domain_types::connector_types::EventType;

    match nuvei_control_panel_event_type(event) {
        EventType::DisputeWon => DisputeStatus::DisputeWon,
        EventType::DisputeLost => DisputeStatus::DisputeLost,
        EventType::DisputeAccepted => DisputeStatus::DisputeAccepted,
        EventType::DisputeChallenged => DisputeStatus::DisputeChallenged,
        EventType::DisputeCancelled => DisputeStatus::DisputeCancelled,
        EventType::DisputeExpired => DisputeStatus::DisputeExpired,
        _ => DisputeStatus::DisputeOpened,
    }
}

/// The dispute stage a family-(b) event carries: a pre-chargeback alert (Ethoca), an RDR
/// alert and a retrieval request are all pre-dispute; a chargeback is a dispute; a
/// second presentment is pre-arbitration.
pub fn nuvei_control_panel_dispute_stage(
    event: &NuveiControlPanelEvent,
) -> common_enums::DisputeStage {
    use common_enums::DisputeStage;

    match event.normalised_event_type() {
        Some(event_type)
            if event_type.contains("pre-chargeback")
                || event_type.contains("pre chargeback")
                || event_type.contains("rdr")
                || event_type.contains("retrieval")
                || event_type.contains("alert") =>
        {
            DisputeStage::PreDispute
        }
        Some(event_type)
            if event_type.contains("arbitration") || event_type.contains("second presentment") =>
        {
            DisputeStage::PreArbitration
        }
        _ => DisputeStage::Dispute,
    }
}

/// The attempt status a Payment DMN carries.
///
/// Kept separate from [`nuvei_attempt_status`] on purpose: the DMN status vocabulary is
/// not the synchronous one and Nuvei publishes no mapping between them (spec gap G-10).
/// Exhaustive on both axes, no `_ =>` arm on the status.
pub fn nuvei_dmn_attempt_status(
    status: NuveiDmnStatus,
    transaction_type: NuveiTransactionType,
) -> common_enums::AttemptStatus {
    use common_enums::AttemptStatus;

    match status {
        NuveiDmnStatus::Approved | NuveiDmnStatus::Success => match transaction_type {
            NuveiTransactionType::Auth | NuveiTransactionType::PreAuth => AttemptStatus::Authorized,
            NuveiTransactionType::Sale | NuveiTransactionType::Settle => AttemptStatus::Charged,
            NuveiTransactionType::Void | NuveiTransactionType::VoidCredit => AttemptStatus::Voided,
            NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::VerifyAuth3D => AttemptStatus::AuthenticationSuccessful,
            NuveiTransactionType::Credit
            | NuveiTransactionType::Chargeback
            | NuveiTransactionType::Modification
            | NuveiTransactionType::Unknown => AttemptStatus::Charged,
        },
        NuveiDmnStatus::Declined | NuveiDmnStatus::Error => match transaction_type {
            NuveiTransactionType::Auth | NuveiTransactionType::PreAuth => {
                AttemptStatus::AuthorizationFailed
            }
            NuveiTransactionType::Void | NuveiTransactionType::VoidCredit => {
                AttemptStatus::VoidFailed
            }
            NuveiTransactionType::Settle => AttemptStatus::CaptureFailed,
            NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::VerifyAuth3D => AttemptStatus::AuthenticationFailed,
            NuveiTransactionType::Sale
            | NuveiTransactionType::Credit
            | NuveiTransactionType::Chargeback
            | NuveiTransactionType::Modification
            | NuveiTransactionType::Unknown => AttemptStatus::Failure,
        },
        NuveiDmnStatus::Pending | NuveiDmnStatus::Update => AttemptStatus::Pending,
        NuveiDmnStatus::Unknown => {
            tracing::warn!(
                connector = "nuvei",
                "nuvei webhook: undocumented DMN Status, keeping the attempt non-terminal"
            );
            AttemptStatus::Pending
        }
    }
}

/// The refund status a `transactionType = Credit` DMN carries.
pub fn nuvei_dmn_refund_status(status: NuveiDmnStatus) -> common_enums::RefundStatus {
    use common_enums::RefundStatus;

    match status {
        NuveiDmnStatus::Approved | NuveiDmnStatus::Success => RefundStatus::Success,
        NuveiDmnStatus::Declined | NuveiDmnStatus::Error => RefundStatus::Failure,
        NuveiDmnStatus::Pending | NuveiDmnStatus::Update => RefundStatus::Pending,
        NuveiDmnStatus::Unknown => {
            tracing::warn!(
                connector = "nuvei",
                "nuvei webhook: undocumented DMN Status on a Credit, keeping the refund pending"
            );
            RefundStatus::Pending
        }
    }
}

impl NuveiPaymentDmn {
    /// `(code, message)` for a failing DMN: `ErrCode` falling back to `ExErrCode`, and
    /// `Reason` falling back to `message`. Never an empty string or a literal fallback.
    pub fn error_fields(&self) -> (Option<String>, Option<String>) {
        let non_empty = |value: Option<&str>| {
            value
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != "0")
                .map(ToOwned::to_owned)
        };
        let code = non_empty(self.err_code.as_deref())
            .or_else(|| non_empty(self.ex_err_code.as_deref()))
            .or_else(|| non_empty(self.reason_code.as_deref()));
        let message = self
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                self.message
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            });
        match (code, message) {
            (None, None) => (None, None),
            (code, message) => (
                Some(code.unwrap_or_else(|| consts::NO_ERROR_CODE.to_string())),
                Some(message.unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string())),
            ),
        }
    }
}
