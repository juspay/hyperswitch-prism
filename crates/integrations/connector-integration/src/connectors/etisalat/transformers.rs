use common_enums::{AttemptStatus, Currency, RefundStatus};
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
    router_data::{ConnectorSpecificConfig, FlowStatus},
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
    transaction_hint: &'static str,
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
            hint::AUTO_CAPTURE
        } else {
            hint::MANUAL_CAPTURE
        };

        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Etisalat caps OrderName at 25 printable chars. Use the merchant order
        // id if provided and short enough, otherwise reuse the connector
        // reference id (which is always present and length-bounded).
        let order_name = request
            .merchant_order_id
            .as_deref()
            .filter(|s| !s.is_empty() && s.chars().count() <= 25)
            .map(str::to_string)
            .unwrap_or_else(|| order_id.clone());

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
    transaction_hint: &'static str,
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

        // Partial captures keep the residual balance (merchant may capture it later
        // or void it explicitly). Full captures release the residual so no funds
        // stay uselessly reserved on the payer's card.
        let is_partial = match item
            .router_data
            .resource_common_data
            .minor_amount_capturable
        {
            Some(authorized) => request.minor_amount_to_capture < authorized,
            None => false,
        };
        let transaction_hint = if is_partial {
            hint::CAPTURE_KEEP_BALANCE
        } else {
            hint::CAPTURE_AUTO_REVERSE
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
    transaction_hint: &'static str,
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

        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            authorization: EtisalatRepeatPaymentPayload {
                customer: auth.customer,
                channel: EtisalatChannel::Recurring,
                currency: request.currency,
                amount,
                order_id: order_id.clone(),
                order_name: order_id,
                transaction_hint: hint::AUTO_CAPTURE,
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
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub printable: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatPayer {
    #[serde(default)]
    pub information: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EtisalatTransactionBody {
    pub response_code: String,
    #[serde(default)]
    pub response_class: Option<String>,
    #[serde(default)]
    pub response_description: Option<String>,
    #[serde(default)]
    pub response_class_description: Option<String>,
    #[serde(default, rename = "TransactionID")]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub approval_code: Option<String>,
    #[serde(default, rename = "OrderID")]
    pub order_id: Option<String>,
    #[serde(default)]
    pub amount: Option<EtisalatMoney>,
    #[serde(default)]
    pub balance: Option<EtisalatMoney>,
    #[serde(default)]
    pub fees: Option<EtisalatMoney>,
    #[serde(default)]
    pub card_number: Option<String>,
    #[serde(default)]
    pub card_token: Option<Secret<String>>,
    #[serde(default)]
    pub card_brand: Option<String>,
    #[serde(default)]
    pub card_type: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default, rename = "UniqueID")]
    pub unique_id: Option<String>,
    #[serde(default)]
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
fn map_attempt_status(body: &EtisalatTransactionBody, on_success: AttemptStatus) -> AttemptStatus {
    if body.is_success() {
        on_success
    } else if body.is_pending() {
        AttemptStatus::Pending
    } else {
        AttemptStatus::Failure
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
        network_txn_id: body.approval_code.clone(),
        network_txn_link_id: None,
        connector_response_reference_id: Some(transaction_id),
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
        let status = map_attempt_status(&body, success_status);

        let response = if body.is_success() {
            // Etisalat returns a CardToken on the direct Authorization; carry it
            // as the mandate reference when the payer opted into mandate setup.
            // Note: true recurrence provisioning requires the 3DS Registration
            // flow — this token is best-effort for CIT card-on-file scenarios.
            let mandate_reference = if item
                .router_data
                .request
                .is_customer_initiated_mandate_payment()
            {
                body.transaction_id.clone().map(|txn_id| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(txn_id),
                        payment_method_id: None,
                        mandate_metadata: None,
                        connector_mandate_request_reference_id: None,
                    })
                })
            } else {
                None
            };
            Ok(success_payments_response(
                &body,
                item.http_code,
                mandate_reference,
            )?)
        } else {
            Err(build_error_response(&body, item.http_code, Some(status)))
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
// Capture response
// ----------------------------------------------------------------------------

impl TryFrom<ResponseRouterData<EtisalatResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<EtisalatResponse, Self>) -> Result<Self, Self::Error> {
        let body = item.response.transaction;

        // A partial capture that reports Success settles to PartialCharged;
        // a full capture settles to Charged.
        let success_status = match item
            .router_data
            .resource_common_data
            .minor_amount_capturable
        {
            Some(authorized) if item.router_data.request.minor_amount_to_capture < authorized => {
                AttemptStatus::PartialCharged
            }
            _ => AttemptStatus::Charged,
        };
        let status = map_attempt_status(&body, success_status);

        let response = if body.is_success() {
            Ok(success_payments_response(&body, item.http_code, None)?)
        } else {
            Err(build_error_response(&body, item.http_code, Some(status)))
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
// Void response
// ----------------------------------------------------------------------------

impl TryFrom<ResponseRouterData<EtisalatResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<EtisalatResponse, Self>) -> Result<Self, Self::Error> {
        let body = item.response.transaction;
        let status = map_attempt_status(&body, AttemptStatus::Voided);

        let response = if body.is_success() {
            // Reversal responses do not include a fresh TransactionID; reuse
            // the original one supplied on the request.
            let txn_id = item.router_data.request.connector_transaction_id.clone();
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: body.approval_code.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: body.unique_id.clone().or(Some(txn_id)),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        } else {
            Err(build_error_response(&body, item.http_code, Some(status)))
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
        let status = map_attempt_status(&body, AttemptStatus::Charged);

        let response = if body.is_success() {
            Ok(success_payments_response(&body, item.http_code, None)?)
        } else {
            Err(build_error_response(&body, item.http_code, Some(status)))
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
        let refund_status = if body.is_success() {
            RefundStatus::Success
        } else if body.is_pending() {
            RefundStatus::Pending
        } else {
            RefundStatus::Failure
        };

        let response = if body.is_success() {
            // Etisalat does not return a distinct refund id; fall back to the
            // UniqueID (per-call unique reference) or the original txn id.
            let refund_id = body
                .unique_id
                .clone()
                .or_else(|| body.transaction_id.clone())
                .unwrap_or_else(|| item.router_data.request.connector_transaction_id.clone());
            Ok(RefundsResponseData {
                connector_refund_id: refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        } else {
            Err(build_error_response(&body, item.http_code, None))
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
    attempt_status: Option<AttemptStatus>,
) -> domain_types::router_data::ErrorResponse {
    let message = body.error_message();
    domain_types::router_data::ErrorResponse {
        status_code,
        code: body.response_code.clone(),
        message: message.clone(),
        reason: Some(message),
        attempt_status: attempt_status.map(FlowStatus::Payment),
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
