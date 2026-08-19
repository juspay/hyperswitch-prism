use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, RepeatPayment, SetupMandate},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, RefundFlowData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors,
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
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

/// Worldpay RAFT operation-level success code (ReturnCode).
const RETURN_CODE_SUCCESS: &str = "0000";
/// Worldpay RAFT issuer-level success code (ResponseCode).
const RESPONSE_CODE_SUCCESS: &str = "000";

/// E-commerce channel entry mode.
const ENTRY_MODE_ECOMM: &str = "E-COMM";
/// POS condition code for e-commerce.
const POS_CONDITION_CODE_ECOMM: &str = "59";
/// Terminal entry capability (not applicable for e-commerce).
const TERMINAL_ENTRY_CAP_DEFAULT: &str = "0";
/// E-commerce indicator: 3DS authenticated.
const ECOMMERCE_INDICATOR_SECURE: &str = "07";
/// CVV2/CVC2 indicator: value provided.
const CVV_INDICATOR_PRESENT: &str = "0";
/// Flag value indicating yes/enabled for proc flag fields.
const MIT_YES: &str = "Y";

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
                    context: errors::IntegrationErrorContext::default()
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
    let now = time::OffsetDateTime::now_utc();
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

/// Derive a 6-digit numeric SystemTraceNumber from an arbitrary ID string.
///
/// Takes the last 6 numeric characters from the string. If fewer than
/// 6 numeric characters exist, pads with zeros on the left.
fn derive_system_trace_number(id: &str) -> String {
    let digits: String = id.chars().filter(|c| c.is_ascii_digit()).collect();
    let len = digits.len();
    if len >= 6 {
        digits[len - 6..].to_string()
    } else {
        format!("{:0>6}", digits)
    }
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

/// Derive AttemptStatus for Authorize and Capture flows.
///
/// - `ReturnCode == RETURN_CODE_SUCCESS` AND `ResponseCode == RESPONSE_CODE_SUCCESS` → success
/// - Anything else → Failure
fn map_payment_status(return_code: &str, response_code: &str) -> bool {
    return_code == RETURN_CODE_SUCCESS && response_code == RESPONSE_CODE_SUCCESS
}

// =============================================================================
// SHARED STRUCTS
// =============================================================================

#[derive(Debug, Serialize)]
pub struct WorldpayraftAmounts {
    #[serde(rename = "TransactionAmount")]
    pub transaction_amount: common_utils::types::StringMajorUnit,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftCardInfo<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "PAN")]
    pub pan: RawCardNumber<T>,
    #[serde(rename = "ExpirationDate")]
    pub expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftCardVerificationData {
    #[serde(rename = "Cvv2Cvc2CIDIndicator")]
    pub cvv_indicator: String,
    #[serde(rename = "CVV2CVC2")]
    pub cvv2_cvc2: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftAddressVerificationData {
    #[serde(skip_serializing_if = "Option::is_none", rename = "AVSZIPCode")]
    pub avs_zip_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "AVSAddress")]
    pub avs_address: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftTerminalData {
    #[serde(rename = "EntryMode")]
    pub entry_mode: String,
    #[serde(rename = "POSConditionCode")]
    pub pos_condition_code: String,
    #[serde(rename = "TerminalEntryCap")]
    pub terminal_entry_cap: String,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftEcommerceData {
    #[serde(rename = "E-commerceIndicator")]
    pub ecommerce_indicator: String,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftRequestTraceNumbers {
    #[serde(rename = "SystemTraceNumber")]
    pub system_trace_number: String,
}

// =============================================================================
// AUTHORIZE REQUEST
// =============================================================================

/// Inner fields shared by both creditauth and debitpreauth requests.
#[derive(Debug, Serialize)]
pub struct WorldpayraftCardAuthInner<
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
    #[serde(rename = "TerminalData")]
    pub terminal_data: WorldpayraftTerminalData,
    #[serde(rename = "E-commerceData")]
    pub ecommerce_data: WorldpayraftEcommerceData,
    #[serde(rename = "ReferenceTraceNumbers")]
    pub reference_trace_numbers: WorldpayraftRequestTraceNumbers,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
    pub local_date_time: String,
}

/// Outer wrapper for authorize requests.
///
/// Credit cards use `{ "creditauth": { ... } }` → POST /credit/authorization
/// Debit cards use  `{ "debitpreauth": { ... } }` → POST /debit/preauth
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayraftAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Credit {
        creditauth: WorldpayraftCardAuthInner<T>,
    },
    Debit {
        debitpreauth: WorldpayraftCardAuthInner<T>,
    },
}

// =============================================================================
// AUTHORIZE RESPONSE
// =============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldpayraftResponseTraceNumbers {
    #[serde(rename = "AuthorizationNumber")]
    pub authorization_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
    #[serde(rename = "SystemTraceNumber")]
    pub system_trace_number: Option<String>,
    #[serde(rename = "NetworkRefNumber")]
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
pub struct WorldpayraftCardAuthResponseInner {
    #[serde(rename = "ReturnCode")]
    pub return_code: String,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ReferenceTraceNumbers")]
    pub reference_trace_numbers: Option<WorldpayraftResponseTraceNumbers>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
    #[serde(rename = "EncryptionTokenData")]
    pub encryption_token_data: Option<WorldpayraftEncryptionTokenData>,
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

        let is_debit = card
            .card_type
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case(CARD_TYPE_DEBIT))
            .unwrap_or(false);

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

        let card_verification_data = {
            let cvv_str = card.card_cvc.peek();
            if !cvv_str.is_empty() {
                Some(WorldpayraftCardVerificationData {
                    cvv_indicator: CVV_INDICATOR_PRESENT.to_string(),
                    cvv2_cvc2: card.card_cvc.clone(),
                })
            } else {
                None
            }
        };

        // AVS is only sent for credit cards (debit doesn't support it)
        let address_verification_data = if is_debit {
            None
        } else {
            let billing = router_data.resource_common_data.get_optional_billing();
            if let Some(billing_addr) = billing {
                let zip = billing_addr.address.as_ref().and_then(|a| a.zip.clone());
                let address_line = billing_addr.address.as_ref().and_then(|a| a.line1.clone());
                if zip.is_some() || address_line.is_some() {
                    Some(WorldpayraftAddressVerificationData {
                        avs_zip_code: zip,
                        avs_address: address_line,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        };

        let payment_id = &router_data
            .resource_common_data
            .connector_request_reference_id;
        let system_trace_number = derive_system_trace_number(payment_id);
        let api_transaction_id = truncate_api_transaction_id(payment_id);
        let local_date_time = get_local_datetime();

        let inner = WorldpayraftCardAuthInner {
            misc_amounts_balances: WorldpayraftAmounts { transaction_amount },
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
            },
            ecommerce_data: WorldpayraftEcommerceData {
                ecommerce_indicator: ECOMMERCE_INDICATOR_SECURE.to_string(),
            },
            reference_trace_numbers: WorldpayraftRequestTraceNumbers {
                system_trace_number,
            },
            world_pay_merchant_id: auth.merchant_id,
            api_transaction_id,
            local_date_time,
        };

        if is_debit {
            Ok(Self::Debit {
                debitpreauth: inner,
            })
        } else {
            Ok(Self::Credit { creditauth: inner })
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
        };

        let status = if map_payment_status(&inner.return_code, &inner.response_code) {
            AttemptStatus::Authorized
        } else {
            AttemptStatus::Failure
        };

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

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: inner.api_transaction_id.clone(),
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
// CAPTURE REQUEST
// =============================================================================

/// Trace numbers carried in the Capture request body.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldpayraftCaptureTraceNumbers {
    #[serde(rename = "AuthorizationNumber")]
    pub authorization_number: String,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: String,
    #[serde(rename = "SystemTraceNumber")]
    pub system_trace_number: String,
}

/// Inner fields shared by creditcompletion and debitcompletion requests.
#[derive(Debug, Serialize)]
pub struct WorldpayraftCompletionInner {
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(rename = "ReferenceTraceNumbers")]
    pub reference_trace_numbers: WorldpayraftCaptureTraceNumbers,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
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
pub struct WorldpayraftCompletionResponseInner {
    #[serde(rename = "ReturnCode")]
    pub return_code: String,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ReferenceTraceNumbers")]
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

        let api_transaction_id = truncate_api_transaction_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );
        let local_date_time = get_local_datetime();

        let inner = WorldpayraftCompletionInner {
            misc_amounts_balances: WorldpayraftAmounts { transaction_amount },
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

        let status = if map_payment_status(&inner.return_code, &inner.response_code) {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Failure
        };

        // Preserve the original composite connector_transaction_id from the Authorize flow
        // so downstream flows (refund) can still use the stored trace numbers.
        let connector_transaction_id = match &item.router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(txn_id) => txn_id.clone(),
            _ => String::new(),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: inner.api_transaction_id.clone(),
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
// REFUND REQUEST
// =============================================================================

/// Trace numbers carried in the Refund request body.
#[derive(Debug, Serialize)]
pub struct WorldpayraftRefundTraceNumbers {
    #[serde(rename = "AuthorizationNumber")]
    pub authorization_number: String,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: String,
    #[serde(rename = "SystemTraceNumber")]
    pub system_trace_number: String,
}

/// Inner fields shared by creditrefund and debitrefund requests.
#[derive(Debug, Serialize)]
pub struct WorldpayraftRefundInner {
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(rename = "ReferenceTraceNumbers")]
    pub reference_trace_numbers: WorldpayraftRefundTraceNumbers,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
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
pub struct WorldpayraftRefundResponseTraceNumbers {
    #[serde(rename = "AuthorizationNumber")]
    pub authorization_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
}

/// Inner fields shared by creditrefundresponse and debitrefundresponse.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldpayraftRefundResponseInner {
    #[serde(rename = "ReturnCode")]
    pub return_code: String,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
    #[serde(rename = "ReferenceTraceNumbers")]
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
pub struct WorldpayraftSetupMandateCardInfo {
    #[serde(rename = "PAN")]
    pub pan: Secret<String>,
    #[serde(rename = "ExpirationDate")]
    pub expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftSetupMandateInner {
    #[serde(rename = "CardInfo")]
    pub card_info: WorldpayraftSetupMandateCardInfo,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
    pub local_date_time: String,
}

/// Wrapper for the tokenize request body.
#[derive(Debug, Serialize)]
pub struct WorldpayraftSetupMandateRequest {
    pub tokenize: WorldpayraftSetupMandateInner,
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
pub struct WorldpayraftSetupMandateTraceNumbers {
    #[serde(rename = "AuthorizationNumber")]
    pub authorization_number: Option<String>,
    #[serde(rename = "RetrievalREFNumber")]
    pub retrieval_ref_number: Option<String>,
    #[serde(rename = "SystemTraceNumber")]
    pub system_trace_number: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WorldpayraftSetupMandateResponseInner {
    #[serde(rename = "ReturnCode")]
    pub return_code: String,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "EncryptionTokenData")]
    pub encryption_token_data: Option<WorldpayraftSetupMandateTokenData>,
    #[serde(rename = "ReferenceTraceNumbers")]
    pub reference_trace_numbers: Option<WorldpayraftSetupMandateTraceNumbers>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
}

/// Wrapper for the tokenizeresponse body.
#[derive(Debug, Deserialize, Serialize)]
pub struct WorldpayraftSetupMandateResponse {
    pub tokenizeresponse: WorldpayraftSetupMandateResponseInner,
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

        let pan = Secret::new(card.card_number.peek().to_string());

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

        let status = if map_payment_status(&response.return_code, &response.response_code) {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Failure
        };

        let tokenized_pan = response
            .encryption_token_data
            .as_ref()
            .and_then(|etd| etd.tokenized_pan.clone());

        let mandate_reference = tokenized_pan.as_ref().map(|pan| {
            Box::new(MandateReference {
                connector_mandate_id: Some(pan.clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });

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
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.api_transaction_id.clone(),
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
// REPEAT PAYMENT REQUEST
// =============================================================================
// Worldpay RAFT MIT using stored TokenizedPAN via POST /credit/authorization
// with MIT flags.

#[derive(Debug, Serialize)]
pub struct WorldpayraftRepeatProcFlags {
    #[serde(rename = "MerchantInitiatedTransaction")]
    pub merchant_initiated_transaction: String,
    #[serde(rename = "RecurringBillPay")]
    pub recurring_bill_pay: Option<String>,
}

/// CardInfo for RepeatPayment — uses a plain Secret<String> PAN (the stored TokenizedPAN),
/// not a generic RawCardNumber<T>.
#[derive(Debug, Serialize)]
pub struct WorldpayraftRepeatCardInfo {
    #[serde(rename = "PAN")]
    pub pan: Secret<String>,
    #[serde(rename = "ExpirationDate")]
    pub expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayraftRepeatCreditAuth {
    #[serde(rename = "MiscAmountsBalances")]
    pub misc_amounts_balances: WorldpayraftAmounts,
    #[serde(rename = "CardInfo")]
    pub card_info: WorldpayraftRepeatCardInfo,
    #[serde(rename = "TerminalData")]
    pub terminal_data: WorldpayraftTerminalData,
    #[serde(rename = "E-commerceData")]
    pub ecommerce_data: WorldpayraftEcommerceData,
    #[serde(rename = "ProcFlagsIndicators")]
    pub proc_flags_indicators: WorldpayraftRepeatProcFlags,
    #[serde(rename = "ReferenceTraceNumbers")]
    pub reference_trace_numbers: WorldpayraftRequestTraceNumbers,
    #[serde(rename = "WorldPayMerchantID")]
    pub world_pay_merchant_id: Secret<String>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: String,
    #[serde(rename = "LocalDateTime")]
    pub local_date_time: String,
}

/// Wrapper for the creditauth request body (RepeatPayment).
#[derive(Debug, Serialize)]
pub struct WorldpayraftRepeatPaymentRequest {
    pub creditauth: WorldpayraftRepeatCreditAuth,
}

/// RepeatPayment response — same outer shape as credit authorize.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldpayraftRepeatPaymentResponseInner {
    #[serde(rename = "ReturnCode")]
    pub return_code: String,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "ReferenceTraceNumbers")]
    pub reference_trace_numbers: Option<WorldpayraftResponseTraceNumbers>,
    #[serde(rename = "APITransactionID")]
    pub api_transaction_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldpayraftRepeatPaymentResponse {
    pub creditauthresponse: WorldpayraftRepeatPaymentResponseInner,
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

        let tokenized_pan = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
                .get_connector_mandate_id()
                .ok_or_else(|| {
                    error_stack::report!(errors::IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Worldpay RAFT RepeatPayment requires a stored TokenizedPAN as the connector mandate ID".to_string(),
                            ),
                            ..Default::default()
                        },
                    })
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "Only ConnectorMandateId is supported for Worldpay RAFT RepeatPayment"
                            .to_string(),
                        errors::IntegrationErrorContext::default(),
                    )
                ))
            }
        };

        let payment_id = &router_data
            .resource_common_data
            .connector_request_reference_id;
        let system_trace_number = derive_system_trace_number(payment_id);
        let api_transaction_id = truncate_api_transaction_id(payment_id);
        let local_date_time = get_local_datetime();

        Ok(Self {
            creditauth: WorldpayraftRepeatCreditAuth {
                misc_amounts_balances: WorldpayraftAmounts { transaction_amount },
                card_info: WorldpayraftRepeatCardInfo {
                    pan: Secret::new(tokenized_pan),
                    // RepeatPayment does not carry raw expiry; use a placeholder.
                    // Worldpay RAFT uses the stored token which already embeds expiry.
                    expiration_date: Secret::new(String::new()),
                },
                terminal_data: WorldpayraftTerminalData {
                    entry_mode: ENTRY_MODE_ECOMM.to_string(),
                    pos_condition_code: POS_CONDITION_CODE_ECOMM.to_string(),
                    terminal_entry_cap: TERMINAL_ENTRY_CAP_DEFAULT.to_string(),
                },
                ecommerce_data: WorldpayraftEcommerceData {
                    ecommerce_indicator: ECOMMERCE_INDICATOR_SECURE.to_string(),
                },
                proc_flags_indicators: WorldpayraftRepeatProcFlags {
                    merchant_initiated_transaction: MIT_YES.to_string(),
                    recurring_bill_pay: Some(MIT_YES.to_string()),
                },
                reference_trace_numbers: WorldpayraftRequestTraceNumbers {
                    system_trace_number,
                },
                world_pay_merchant_id: auth.merchant_id,
                api_transaction_id,
                local_date_time,
            },
        })
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
        let response = &item.response.creditauthresponse;

        let status = if map_payment_status(&response.return_code, &response.response_code) {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Failure
        };

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

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.api_transaction_id.clone(),
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
