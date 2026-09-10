use common_enums::{AttemptStatus, CaptureMethod, Currency, RefundStatus};
use common_utils::types::StringMajorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, RepeatPayment, Void},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, RefundFlowData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorResponseData, ConnectorSpecificConfig, FlowStatus},
    router_data_v2::RouterDataV2,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use super::EtisalatRouterData;
use crate::{types::ResponseRouterData, utils};

// -----------------------------------------------------------------------------
// Etisalat EPG semantics
//
// * All operations POST to the same base URL. The outer JSON key selects the
//   operation (Authorization, Capture, Reversal, Refund).
// * `UserName`, `Password` and `Customer` are carried in the body of every
//   request; there is no header-based auth.
// * `ResponseCode == "0"` is the sole authoritative success indicator; the
//   textual `ResponseClassDescription` is display-only per the spec.
// * Amounts travel as decimal strings ("10.00"), not minor units.
// -----------------------------------------------------------------------------

const SUCCESS_RESPONSE_CODE: &str = "0";

/// Response codes the gateway explicitly documents as still in-flight.
/// Anything else non-zero is a terminal failure for the current attempt.
const PENDING_RESPONSE_CODES: &[&str] = &["58", "62", "118", "199", "210", "999"];

/// Etisalat rejects `OrderName` longer than 25 characters with error code 6517
/// ("OrderName length limit is 25 digits"). PDF §14.2 also disallows leading /
/// trailing whitespace on the field.
const ORDER_NAME_MAX_CHARS: usize = 25;

/// Etisalat rejects `OrderID` with error 6515 ("length limit is 16 digits or it
/// contains special characters"). Only ASCII alphanumerics are accepted, up to
/// 16 chars.
const ORDER_ID_MAX_CHARS: usize = 16;

/// Build an `OrderName` value that satisfies EPG's 25-char cap. Prefers a
/// merchant-supplied source when present and non-empty, otherwise falls back
/// to the connector reference id. Truncation is by Unicode codepoints so a
/// non-ASCII merchant order id cannot panic the slice.
fn build_order_name(preferred: Option<&str>, fallback: &str) -> String {
    let raw = preferred
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback.trim());
    raw.chars().take(ORDER_NAME_MAX_CHARS).collect()
}

/// Derive a valid `OrderID` from prism's `connector_request_reference_id`.
///
/// Prism is stateless, so we cannot mint and store a fresh id — the value must
/// be a pure function of the reference so PSync/reconciliation always produce
/// the same OrderID. Hyperswitch reference ids look like
/// `pay_<20-char-random>_<attempt-index>`; both the underscores and the length
/// violate EPG's constraints. We keep only ASCII alphanumerics (bytes and
/// codepoints coincide → safe slicing) and take the trailing 16 chars so the
/// attempt-index suffix that distinguishes retries is preserved.
fn build_order_id(reference: &str) -> String {
    let alnum: String = reference
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    let start = alnum.len().saturating_sub(ORDER_ID_MAX_CHARS);
    alnum[start..].to_string()
}

/// `TransactionHint` primitives used by the flows this connector implements.
mod hint {
    /// Authorization is captured immediately (Authorize + Capture in one call).
    pub const AUTO_CAPTURE: &str = "CPT:Y;";
    /// Authorization is held; a separate Capture call is required to settle.
    pub const MANUAL_CAPTURE: &str = "CPT:N;";
    /// Auto-reverse any leftover balance after a partial capture.
    pub const CAPTURE_AUTO_REVERSE: &str = "RVS:Y";
    /// Keep the leftover balance available after a partial capture.
    pub const CAPTURE_KEEP_BALANCE: &str = "RVS:N";
}

/// Etisalat payment channel codes accepted by the Authorization endpoint.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum EtisalatChannel {
    /// Web (customer-initiated card entry).
    #[serde(rename = "W")]
    Web,
    /// Recurring — merchant-initiated stored-credential payment.
    #[serde(rename = "R")]
    Recurring,
}

// ----------------------------------------------------------------------------
// Auth
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EtisalatAuthType {
    pub(super) user_name: Secret<String>,
    pub(super) password: Secret<String>,
    pub(super) customer: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for EtisalatAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Etisalat {
                user_name,
                password,
                customer,
                ..
            } => Ok(Self {
                user_name: user_name.to_owned(),
                password: password.to_owned(),
                customer: customer.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure the Etisalat connector with a user_name, password \
                             and customer identifier (ConnectorSpecificConfig::Etisalat)."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "The connector_config passed to EtisalatAuthType::try_from was not \
                             the ConnectorSpecificConfig::Etisalat variant — either the wrong \
                             connector's config was routed to Etisalat, or the merchant account \
                             is missing Etisalat credentials."
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

// ----------------------------------------------------------------------------
// Authorize
// ----------------------------------------------------------------------------

/// The outer envelope Etisalat expects for the Authorization endpoint.
#[derive(Debug, Serialize)]
pub struct EtisalatAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "Authorization")]
    pub authorization: EtisalatAuthorizePayload<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatAuthorizePayload<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    customer: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    store: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal: Option<String>,
    channel: EtisalatChannel,
    currency: Currency,
    amount: StringMajorUnit,
    #[serde(rename = "OrderID")]
    order_id: String,
    order_name: String,
    transaction_hint: String,
    card_number: RawCardNumber<T>,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verify_code: Option<Secret<String>>,
    user_name: Secret<String>,
    password: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EtisalatRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for EtisalatAuthorizeRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: EtisalatRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let router_data = &item.router_data;

        let card = match request.payment_method_data.clone() {
            PaymentMethodData::Card(card) => card,
            other => {
                return Err(IntegrationError::NotImplemented(
                    "Etisalat authorize supports only raw card payments".to_string(),
                    IntegrationErrorContext {
                        suggested_action: Some(
                            "Send `payment_method_data` as PaymentMethodData::Card. \
                             Etisalat does not accept wallet tokens (ApplePay / GooglePay) \
                             on this integration."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(format!(
                            "Received payment_method_data variant: {other:?}"
                        )),
                    },
                )
                .into())
            }
        };

        let auth = EtisalatAuthType::try_from(&router_data.connector_config)?;
        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: utils::amount_conversion_ctx(
                    "authorize",
                    &request.minor_amount,
                    &request.currency,
                ),
            })?;

        let transaction_hint = if request.is_auto_capture() {
            hint::AUTO_CAPTURE.to_string()
        } else {
            hint::MANUAL_CAPTURE.to_string()
        };

        let reference_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .as_str();
        let order_id = build_order_id(reference_id);
        let order_name = build_order_name(request.merchant_order_id.as_deref(), reference_id);

        // NOTE: mandate setup is intentionally not attempted on this path even
        // if `setup_future_usage=OffSession` is present on the CIT — EPG's
        // MOTO endpoint rejects `Recurrence: {Type: M}` with 6801 unless the
        // merchant has an explicit enablement flag set by Etisalat, and the
        // documented path for recurrence registration is the 3DS Registration
        // flow (PDF §9.1) which this integration does not implement.
        //
        // The RepeatPayment MIT still works when the caller supplies a
        // RecurrenceID sourced out of band from a 3DS Registration.

        let payload = EtisalatAuthorizePayload {
            customer: auth.customer,
            store: None,
            terminal: None,
            channel: EtisalatChannel::Web,
            currency: request.currency,
            amount,
            order_id,
            order_name,
            transaction_hint,
            card_number: card.card_number.clone(),
            expiry_month: card.card_exp_month.clone(),
            expiry_year: card.get_expiry_year_4_digit(),
            verify_code: Some(card.card_cvc.clone()),
            user_name: auth.user_name,
            password: auth.password,
        };

        Ok(Self {
            authorization: payload,
        })
    }
}

// ----------------------------------------------------------------------------
// Capture
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct EtisalatCaptureRequest {
    #[serde(rename = "Capture")]
    pub capture: EtisalatCapturePayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatCapturePayload {
    customer: Secret<String>,
    #[serde(rename = "TransactionID")]
    transaction_id: String,
    amount: StringMajorUnit,
    currency: Currency,
    transaction_hint: String,
    user_name: Secret<String>,
    password: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EtisalatRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for EtisalatCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: EtisalatRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let auth = EtisalatAuthType::try_from(&item.router_data.connector_config)?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount_to_capture, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: utils::amount_conversion_ctx(
                    "capture",
                    &request.minor_amount_to_capture,
                    &request.currency,
                ),
            })?;

        // `minor_amount_capturable` is None on this path (domain_types hardcodes it);
        // derive partial vs full from the authorized amount in resource_common_data.amount.
        // Partial captures keep the residual balance (RVS:N). Full captures release it (RVS:Y).
        let authorized = item
            .router_data
            .resource_common_data
            .amount
            .as_ref()
            .map(|m| m.amount);
        let is_partial = match authorized {
            Some(auth_amount) => request.minor_amount_to_capture < auth_amount,
            None => false,
        };
        let transaction_hint = if is_partial {
            hint::CAPTURE_KEEP_BALANCE.to_string()
        } else {
            hint::CAPTURE_AUTO_REVERSE.to_string()
        };

        Ok(Self {
            capture: EtisalatCapturePayload {
                customer: auth.customer,
                transaction_id: request.get_connector_transaction_id()?,
                amount,
                currency: request.currency,
                transaction_hint,
                user_name: auth.user_name,
                password: auth.password,
            },
        })
    }
}

// ----------------------------------------------------------------------------
// Void (Etisalat Reversal)
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct EtisalatReversalRequest {
    #[serde(rename = "Reversal")]
    pub reversal: EtisalatReversalPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatReversalPayload {
    customer: Secret<String>,
    #[serde(rename = "TransactionID")]
    transaction_id: String,
    user_name: Secret<String>,
    password: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EtisalatRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for EtisalatReversalRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: EtisalatRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = EtisalatAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            reversal: EtisalatReversalPayload {
                customer: auth.customer,
                transaction_id: item.router_data.request.connector_transaction_id.clone(),
                user_name: auth.user_name,
                password: auth.password,
            },
        })
    }
}

// ----------------------------------------------------------------------------
// Refund
//
// ⚠ EPG's Refund flow charges the merchant account and credits the payer's
// card, so it only accepts amounts that have already **settled to the merchant
// account** (PDF §8). Auto-captured payments (`CPT:Y`) are captured at the API
// layer immediately but settle in a nightly batch — until then EPG rejects
// refunds with error code `6888 "Not enough captured amount"`.
//
// For same-day cancellation of an auto-captured or authorized-only payment,
// use the Void flow (EPG's Reversal endpoint) instead. Refund is the right
// call once the transaction has settled (typically T+1).
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct EtisalatRefundRequest {
    #[serde(rename = "Refund")]
    pub refund: EtisalatRefundPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatRefundPayload {
    customer: Secret<String>,
    #[serde(rename = "TransactionID")]
    transaction_id: String,
    amount: StringMajorUnit,
    currency: Currency,
    user_name: Secret<String>,
    password: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EtisalatRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for EtisalatRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: EtisalatRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let auth = EtisalatAuthType::try_from(&item.router_data.connector_config)?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_refund_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: utils::amount_conversion_ctx(
                    "refund",
                    &request.minor_refund_amount,
                    &request.currency,
                ),
            })?;

        Ok(Self {
            refund: EtisalatRefundPayload {
                customer: auth.customer,
                transaction_id: request.connector_transaction_id.clone(),
                amount,
                currency: request.currency,
                user_name: auth.user_name,
                password: auth.password,
            },
        })
    }
}

// ----------------------------------------------------------------------------
// RepeatPayment (MIT — sends TransactionID of the master mandate txn,
// no card details, Channel=R)
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct EtisalatRepeatPaymentRequest {
    #[serde(rename = "Authorization")]
    pub authorization: EtisalatRepeatPaymentPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatRepeatPaymentPayload {
    customer: Secret<String>,
    channel: EtisalatChannel,
    currency: Currency,
    amount: StringMajorUnit,
    #[serde(rename = "OrderID")]
    order_id: String,
    order_name: String,
    transaction_hint: String,
    #[serde(rename = "TransactionID")]
    transaction_id: String,
    user_name: Secret<String>,
    password: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EtisalatRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for EtisalatRepeatPaymentRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: EtisalatRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let router_data = &item.router_data;

        // Etisalat MIT charges always auto-capture; there is no hold/manual-capture
        // variant for off-session transactions.
        if matches!(
            request.capture_method,
            Some(CaptureMethod::Manual) | Some(CaptureMethod::ManualMultiple)
        ) {
            return Err(Report::new(IntegrationError::NotImplemented(
                "Manual capture is not supported for Etisalat MIT charges".to_string(),
                IntegrationErrorContext {
                    additional_context: Some(
                        "Etisalat's recurring Authorization endpoint always settles immediately \
                         (CPT:Y); no Hold variant exists for off-session transactions."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Use capture_method: AUTOMATIC for merchant-initiated transactions."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            )));
        }

        // Etisalat's recurrence design keys off the master TransactionID (the one
        // returned by the original 3DS Registration + Finalization that set up the
        // mandate). Prism carries this as `connector_mandate_id`.
        let mandate_id = request.connector_mandate_id().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "RepeatPayment requires the RecurrenceID (master TransactionID) from \
                         the original Etisalat Registration + Finalization that provisioned \
                         the mandate. Pass it as connector_mandate_id."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Etisalat MIT reuses stored card data referenced by a prior \
                         TransactionID. Without connector_mandate_id there is nothing to charge."
                            .to_string(),
                    ),
                },
            }
        })?;

        let auth = EtisalatAuthType::try_from(&router_data.connector_config)?;
        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: utils::amount_conversion_ctx(
                    "repeat_payment",
                    &request.minor_amount,
                    &request.currency,
                ),
            })?;

        let reference_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .as_str();
        let order_id = build_order_id(reference_id);
        let order_name = build_order_name(None, reference_id);

        Ok(Self {
            authorization: EtisalatRepeatPaymentPayload {
                customer: auth.customer,
                channel: EtisalatChannel::Recurring,
                currency: request.currency,
                amount,
                order_id,
                order_name,
                transaction_hint: hint::AUTO_CAPTURE.to_string(),
                transaction_id: mandate_id,
                user_name: auth.user_name,
                password: auth.password,
            },
        })
    }
}

// ----------------------------------------------------------------------------
// Response types (shared shape — every Etisalat response is wrapped in
// `{ "Transaction": { ... } }`)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EtisalatMoney {
    pub value: Option<String>,
    pub printable: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatPayer {
    pub information: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatTransactionBody {
    pub response_code: String,
    pub response_class: Option<String>,
    pub response_description: Option<String>,
    pub response_class_description: Option<String>,
    #[serde(rename = "TransactionID")]
    pub transaction_id: Option<String>,
    pub approval_code: Option<String>,
    #[serde(rename = "OrderID")]
    pub order_id: Option<String>,
    pub amount: Option<EtisalatMoney>,
    pub balance: Option<EtisalatMoney>,
    pub fees: Option<EtisalatMoney>,
    pub card_number: Option<Secret<String>>,
    pub card_token: Option<Secret<String>>,
    pub card_brand: Option<String>,
    pub card_type: Option<String>,
    pub language: Option<String>,
    pub account: Option<String>,
    #[serde(rename = "UniqueID")]
    pub unique_id: Option<String>,
    pub payer: Option<EtisalatPayer>,
}

/// Every Etisalat operation returns `{"Transaction": {...}}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EtisalatResponse {
    #[serde(rename = "Transaction")]
    pub transaction: EtisalatTransactionBody,
}

impl EtisalatTransactionBody {
    fn is_success(&self) -> bool {
        self.response_code == SUCCESS_RESPONSE_CODE
    }

    fn is_pending(&self) -> bool {
        PENDING_RESPONSE_CODES.contains(&self.response_code.as_str())
    }

    fn error_message(&self) -> String {
        self.response_description
            .clone()
            .or_else(|| self.response_class_description.clone())
            .unwrap_or_else(|| format!("Etisalat responded with code {}", self.response_code))
    }
}

fn missing_transaction_id_err() -> ConnectorError {
    ConnectorError::ResponseDeserializationFailed {
        context: ResponseTransformationErrorContext {
            http_status_code: None,
            additional_context: Some(
                "Etisalat success response did not include a Transaction.TransactionID."
                    .to_string(),
            ),
        },
    }
}

/// Map the Etisalat response code to an AttemptStatus, given the flow-specific
/// success mapping (Authorize can settle to Authorized or Charged depending on
/// capture mode; Capture → Charged; Void → Voided; RepeatPayment → Charged).
/// Map the Etisalat response code to an AttemptStatus. `on_failure` must be
/// flow-specific — a rejected Capture should surface as `CaptureFailed`, a
/// rejected Void as `VoidFailed`, not as a generic `Failure` that would imply
/// the authorization itself was declined.
fn map_attempt_status(
    body: &EtisalatTransactionBody,
    on_success: AttemptStatus,
    on_failure: AttemptStatus,
) -> AttemptStatus {
    if body.is_success() {
        on_success
    } else if body.is_pending() {
        AttemptStatus::Pending
    } else {
        on_failure
    }
}

/// Build a PaymentsResponseData for a success transaction body.
fn success_payments_response(
    body: &EtisalatTransactionBody,
    http_status_code: u16,
    mandate_reference: Option<Box<MandateReference>>,
) -> Result<PaymentsResponseData, Report<ConnectorError>> {
    let transaction_id = body
        .transaction_id
        .clone()
        .ok_or_else(|| Report::new(missing_transaction_id_err()))?;
    Ok(PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
        redirection_data: None,
        mandate_reference,
        connector_metadata: None,
        // approval_code is an acquirer auth code, not a network transaction id.
        // Callers set connector_response via ConnectorResponseData::with_auth_code.
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: body.order_id.clone().or(Some(transaction_id)),
        incremental_authorization_allowed: None,
        splits: None,
        status_code: http_status_code,
        payment_account_reference: None,
    })
}

// ----------------------------------------------------------------------------
// Authorize response
// ----------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<EtisalatResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<EtisalatResponse, Self>) -> Result<Self, Self::Error> {
        let body = item.response.transaction;
        let is_auto_capture = item.router_data.request.is_auto_capture();
        let success_status = if is_auto_capture {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Authorized
        };
        let status = map_attempt_status(&body, success_status, AttemptStatus::Failure);

        // The plain Authorization TransactionID is not a chargeable recurrence handle
        // — Etisalat's MIT requires a 3DS Registration flow that this connector does
        // not yet implement. Emitting a mandate_reference here would record a saved
        // mandate that fails on every subsequent MIT charge.
        let response = if body.is_success() {
            Ok(success_payments_response(&body, item.http_code, None)?)
        } else {
            Err(build_error_response(
                &body,
                item.http_code,
                Some(FlowStatus::Payment(status)),
            ))
        };

        // Route the acquirer approval_code through connector_response rather than
        // network_txn_id, which is reserved for the network transaction identifier
        // used for MIT continuity.
        let connector_response = body.approval_code.clone().and_then(|code| {
            item.router_data
                .resource_common_data
                .payment_method_type
                .map(|pmt| ConnectorResponseData::with_auth_code(code, pmt))
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// ----------------------------------------------------------------------------
// Capture response
// ----------------------------------------------------------------------------

impl TryFrom<ResponseRouterData<EtisalatResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<EtisalatResponse, Self>) -> Result<Self, Self::Error> {
        let body = item.response.transaction;

        // `minor_amount_capturable` is None on the Capture path; derive partial/full
        // from the authorized amount in resource_common_data.amount instead.
        let authorized = item
            .router_data
            .resource_common_data
            .amount
            .as_ref()
            .map(|m| m.amount);
        let is_partial = match authorized {
            Some(auth_amount) => item.router_data.request.minor_amount_to_capture < auth_amount,
            None => false,
        };
        let success_status = if is_partial {
            AttemptStatus::PartialCharged
        } else {
            AttemptStatus::Charged
        };
        let status = map_attempt_status(&body, success_status, AttemptStatus::CaptureFailed);

        let response = if body.is_success() {
            // Capture responses do not include a fresh TransactionID (PDF §6.3
            // — the successful sample only carries ResponseCode, Balance,
            // UniqueID). Reuse the txn id we just captured against so the
            // resource stays addressable downstream.
            let txn_id = item
                .router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(ConnectorError::ResponseHandlingFailed {
                    context: ResponseTransformationErrorContext {
                        http_status_code: Some(item.http_code),
                        additional_context: Some(
                            "Etisalat Capture response omitted TransactionID and the request's \
                             connector_transaction_id was not a ConnectorTransactionId variant."
                                .to_string(),
                        ),
                    },
                })?;
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: body.unique_id.clone().or(Some(txn_id)),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        } else {
            Err(build_error_response(
                &body,
                item.http_code,
                Some(FlowStatus::Payment(status)),
            ))
        };

        let connector_response = body.approval_code.clone().and_then(|code| {
            item.router_data
                .resource_common_data
                .payment_method_type
                .map(|pmt| ConnectorResponseData::with_auth_code(code, pmt))
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// ----------------------------------------------------------------------------
// Void response
// ----------------------------------------------------------------------------

impl TryFrom<ResponseRouterData<EtisalatResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<EtisalatResponse, Self>) -> Result<Self, Self::Error> {
        let body = item.response.transaction;
        let status = map_attempt_status(&body, AttemptStatus::Voided, AttemptStatus::VoidFailed);

        let response = if body.is_success() {
            // Reversal responses do not include a fresh TransactionID; reuse
            // the original one supplied on the request.
            let txn_id = item.router_data.request.connector_transaction_id.clone();
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: body.unique_id.clone().or(Some(txn_id)),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        } else {
            Err(build_error_response(
                &body,
                item.http_code,
                Some(FlowStatus::Payment(status)),
            ))
        };

        let connector_response = body.approval_code.clone().and_then(|code| {
            item.router_data
                .resource_common_data
                .payment_method_type
                .map(|pmt| ConnectorResponseData::with_auth_code(code, pmt))
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// ----------------------------------------------------------------------------
// RepeatPayment response
// ----------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<EtisalatResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<EtisalatResponse, Self>) -> Result<Self, Self::Error> {
        let body = item.response.transaction;
        // Payer-not-present recurring charges always auto-capture on Etisalat.
        let status = map_attempt_status(&body, AttemptStatus::Charged, AttemptStatus::Failure);

        let response = if body.is_success() {
            Ok(success_payments_response(&body, item.http_code, None)?)
        } else {
            Err(build_error_response(
                &body,
                item.http_code,
                Some(FlowStatus::Payment(status)),
            ))
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

// ----------------------------------------------------------------------------
// Refund response
// ----------------------------------------------------------------------------

impl TryFrom<ResponseRouterData<EtisalatResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<EtisalatResponse, Self>) -> Result<Self, Self::Error> {
        let body = item.response.transaction;

        // EPG mints no refund-specific id. UniqueID is the per-call UUID (unique per
        // refund attempt; absent = spec violation). Use it for success and pending;
        // fall back to TransactionID only when both are absent.
        let unique_id_err = || {
            Report::new(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "Etisalat refund response did not include Transaction.UniqueID."
                            .to_string(),
                    ),
                },
            })
        };

        let response = if body.is_success() {
            let refund_id = body.unique_id.clone().ok_or_else(unique_id_err)?;
            Ok(RefundsResponseData {
                connector_refund_id: refund_id,
                refund_status: RefundStatus::Success,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        } else if body.is_pending() {
            // Pending refunds also carry a UniqueID per spec §8.
            let refund_id = body
                .unique_id
                .clone()
                .or_else(|| body.transaction_id.clone())
                .ok_or_else(unique_id_err)?;
            Ok(RefundsResponseData {
                connector_refund_id: refund_id,
                refund_status: RefundStatus::Pending,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        } else {
            Err(build_error_response(
                &body,
                item.http_code,
                Some(FlowStatus::Refund(RefundStatus::Failure)),
            ))
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// ----------------------------------------------------------------------------
// Error mapping
// ----------------------------------------------------------------------------

pub fn build_error_response(
    body: &EtisalatTransactionBody,
    status_code: u16,
    flow_status: Option<FlowStatus>,
) -> domain_types::router_data::ErrorResponse {
    let message = body.error_message();
    domain_types::router_data::ErrorResponse {
        status_code,
        code: body.response_code.clone(),
        message: message.clone(),
        reason: Some(message),
        attempt_status: flow_status,
        connector_transaction_id: body.transaction_id.clone(),
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}
