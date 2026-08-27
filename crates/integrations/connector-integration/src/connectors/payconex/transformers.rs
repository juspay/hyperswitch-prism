use std::fmt::Debug;

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    types::{MinorUnit, StringMajorUnit},
};
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
};
use error_stack::{report, Report, ResultExt};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::payconex::PayconexRouterData, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================
#[derive(Debug, Clone)]
pub struct PayconexAuthType {
    pub api_accesskey: Secret<String>,
    pub account_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PayconexAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Payconex {
                api_key,
                account_id,
                ..
            } => Ok(Self {
                api_accesskey: api_key.to_owned(),
                account_id: account_id.to_owned(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: Default::default()
            })),
        }
    }
}

// =============================================================================
// COMMON ENUMS
// =============================================================================
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TenderType {
    Card,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TransactionType {
    Sale,
    Authorization,
    Capture,
    Refund,
    // PayConex names the reversal action "REVERSAL", not "REVERSE".
    #[serde(rename = "REVERSAL")]
    Reverse,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ResponseFormat {
    Json,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsapiAction {
    GetTransactionStatus,
}

/// PayConex QSAPI `payment_type` — the source/nature of the transaction.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaymentType {
    Ecommerce,
    Installment,
    Recurring,
    Moto,
}

// Maps the UCS request to PayConex `payment_type`. MOTO has a real signal
// (`payment_channel` = mail / telephone order); a merchant-initiated payment
// (`off_session`) maps to RECURRING, which PayConex accepts without a stored
// credential. INSTALLMENT needs `installment_count` / `installment_number`, which
// UCS does not surface, so it is not inferred here.
fn map_payment_type(
    payment_channel: Option<&common_enums::PaymentChannel>,
    off_session: Option<bool>,
) -> PaymentType {
    match payment_channel {
        Some(common_enums::PaymentChannel::MailOrder)
        | Some(common_enums::PaymentChannel::TelephoneOrder) => PaymentType::Moto,
        _ => match off_session {
            Some(true) => PaymentType::Recurring,
            _ => PaymentType::Ecommerce,
        },
    }
}

// =============================================================================
// FLEXIBLE BOOLEAN — PayConex returns "1"/"0"/true/false/null for booleans
// =============================================================================
fn de_flexible_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexibleBool {
        Bool(bool),
        Int(i64),
        Str(String),
    }
    let value = Option::<FlexibleBool>::deserialize(deserializer)?;
    Ok(match value {
        Some(FlexibleBool::Bool(b)) => b,
        Some(FlexibleBool::Int(i)) => i != 0,
        Some(FlexibleBool::Str(s)) => {
            let s = s.trim().to_ascii_lowercase();
            s == "1" || s == "true" || s == "y" || s == "yes"
        }
        None => false,
    })
}

// =============================================================================
// AUTHORIZE / SALE REQUEST
// =============================================================================
#[derive(Debug, Serialize)]
pub struct PayconexPaymentsRequest<T: PaymentMethodDataTypes + Serialize + Debug> {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub tender_type: TenderType,
    pub transaction_type: TransactionType,
    pub payment_type: PaymentType,
    pub transaction_amount: StringMajorUnit,
    pub card_number: RawCardNumber<T>,
    pub card_expiration: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_verification: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    pub custom_id: String,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayconexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayconexPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PayconexRouterData<
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
        let auth = PayconexAuthType::try_from(&router_data.connector_config)?;

        if router_data.resource_common_data.is_three_ds() {
            return Err(report!(IntegrationError::NotSupported {
                message: "3DS card payments".to_string(),
                connector: "Payconex",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Route 3DS card payments to a 3DS-capable connector; PayConex \
                         supports no-3DS card payments only."
                            .to_string(),
                    ),
                    additional_context: Some(
                        "Authorize received a card flagged for 3DS authentication, which PayConex \
                         QSAPI does not support."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }));
        }

        // Manual / scheduled capture maps to an AUTHORIZATION hold; everything else
        // (auto-capture) is a SALE. Mirrors `PaymentsAuthorizeData::is_auto_capture`.
        let transaction_type = match request.capture_method {
            Some(common_enums::CaptureMethod::Manual)
            | Some(common_enums::CaptureMethod::ManualMultiple)
            | Some(common_enums::CaptureMethod::Scheduled) => TransactionType::Authorization,
            _ => TransactionType::Sale,
        };

        let payment_type = map_payment_type(request.payment_channel.as_ref(), request.off_session);

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        match request.payment_method_data.clone() {
            PaymentMethodData::Card(card) => Ok(Self {
                account_id: auth.account_id,
                api_accesskey: auth.api_accesskey,
                tender_type: TenderType::Card,
                transaction_type,
                payment_type,
                transaction_amount,
                card_number: card.card_number.clone(),
                card_expiration: card
                    .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
                card_verification: Some(card.card_cvc.clone()),
                first_name: card.card_holder_name.clone(),
                last_name: None,
                custom_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                response_format: ResponseFormat::Json,
            }),
            _ => Err(report!(IntegrationError::NotImplemented(
                "Payment method".to_string(),
                IntegrationErrorContext {
                    suggested_action: Some(
                        "Use a card payment method; PayConex currently supports card \
                         payments only."
                            .to_string(),
                    ),
                    additional_context: Some(
                        "Received a non-card payment method for the PayConex Authorize flow."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            ))),
        }
    }
}

// =============================================================================
// CAPTURE REQUEST
// =============================================================================
#[derive(Debug, Serialize)]
pub struct PayconexCaptureRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub tender_type: TenderType,
    pub transaction_type: TransactionType,
    pub token_id: String,
    pub transaction_amount: StringMajorUnit,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayconexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PayconexCaptureRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PayconexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PayconexAuthType::try_from(&router_data.connector_config)?;
        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            tender_type: TenderType::Card,
            transaction_type: TransactionType::Capture,
            token_id: router_data.request.get_connector_transaction_id()?,
            transaction_amount,
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// VOID (REVERSAL) REQUEST
// =============================================================================
#[derive(Debug, Serialize)]
pub struct PayconexVoidRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub transaction_type: TransactionType,
    pub token_id: String,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayconexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PayconexVoidRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PayconexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PayconexAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            transaction_type: TransactionType::Reverse,
            token_id: router_data.request.connector_transaction_id.clone(),
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// PSYNC REQUEST (TSAPI)
// =============================================================================
#[derive(Debug, Serialize)]
pub struct PayconexSyncRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub action: TsapiAction,
    pub transaction_id: String,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayconexRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for PayconexSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PayconexRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PayconexAuthType::try_from(&router_data.connector_config)?;
        let transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            action: TsapiAction::GetTransactionStatus,
            transaction_id,
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// REFUND REQUEST
// =============================================================================
#[derive(Debug, Serialize)]
pub struct PayconexRefundRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub tender_type: TenderType,
    pub transaction_type: TransactionType,
    pub token_id: String,
    pub transaction_amount: StringMajorUnit,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayconexRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PayconexRefundRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PayconexRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PayconexAuthType::try_from(&router_data.connector_config)?;
        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                MinorUnit::new(router_data.request.refund_amount),
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            tender_type: TenderType::Card,
            transaction_type: TransactionType::Refund,
            token_id: router_data.request.connector_transaction_id.clone(),
            transaction_amount,
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// RSYNC REQUEST (TSAPI)
// =============================================================================
#[derive(Debug, Serialize)]
pub struct PayconexRefundSyncRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub action: TsapiAction,
    pub transaction_id: String,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayconexRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for PayconexRefundSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PayconexRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PayconexAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            action: TsapiAction::GetTransactionStatus,
            transaction_id: router_data.request.connector_refund_id.clone(),
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// RESPONSES
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayconexErrorResponse {
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub error: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub authorization_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayconexPaymentsResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl PayconexPaymentsResponse {
    fn is_approved(&self) -> bool {
        self.transaction_approved
    }

    fn get_transaction_id(&self, http_code: u16) -> CustomResult<String, ConnectorError> {
        self.transaction_id.clone().ok_or_else(|| {
            report!(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some("missing transaction_id in PayConex response".to_string()),
            ))
        })
    }
}

pub fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// All PayConex responses share the same `error_code` / `error_message` /
// `authorization_message` shape, so the error-string mapping lives here once
// rather than being re-implemented on every response struct.
fn error_code_string(error_code: &Option<serde_json::Value>) -> String {
    error_code
        .as_ref()
        .map(value_to_string)
        .unwrap_or_else(|| NO_ERROR_CODE.to_string())
}

fn error_message_string(
    error_message: &Option<String>,
    authorization_message: &Option<String>,
) -> String {
    error_message
        .clone()
        .or_else(|| authorization_message.clone())
        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string())
}

// Common payment-status mapping shared by the Authorize and PSync flows. `found` is
// false only for a sync query with no record yet; an approved transaction is `Charged`
// under auto-capture and `Authorized` under manual capture.
fn map_payment_status(
    found: bool,
    approved: bool,
    is_auto_capture: bool,
) -> common_enums::AttemptStatus {
    match (found, approved, is_auto_capture) {
        (false, _, _) => common_enums::AttemptStatus::Pending,
        (true, true, true) => common_enums::AttemptStatus::Charged,
        (true, true, false) => common_enums::AttemptStatus::Authorized,
        (true, false, _) => common_enums::AttemptStatus::Failure,
    }
}

// Common refund-status mapping shared by the Refund and RSync flows.
fn map_refund_status(found: bool, approved: bool) -> common_enums::RefundStatus {
    match (found, approved) {
        (false, _) => common_enums::RefundStatus::Pending,
        (true, true) => common_enums::RefundStatus::Success,
        (true, false) => common_enums::RefundStatus::Failure,
    }
}

// ---- Authorize response ----
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PayconexPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PayconexPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = item.router_data.request.is_auto_capture();
        let status = map_payment_status(true, item.response.is_approved(), is_auto_capture);
        let response = match status {
            common_enums::AttemptStatus::Failure => Err(ErrorResponse {
                code: error_code_string(&item.response.error_code),
                message: error_message_string(
                    &item.response.error_message,
                    &item.response.authorization_message,
                ),
                reason: item.response.authorization_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: item.response.transaction_id.clone(),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.get_transaction_id(item.http_code)?,
                ),
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

// ---- Capture response ----
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayconexCaptureResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl PayconexCaptureResponse {
    fn is_approved(&self) -> bool {
        self.transaction_approved
    }
    fn get_transaction_id(&self, http_code: u16) -> CustomResult<String, ConnectorError> {
        self.transaction_id.clone().ok_or_else(|| {
            report!(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some("missing transaction_id in PayConex capture response".to_string()),
            ))
        })
    }
}

impl TryFrom<ResponseRouterData<PayconexCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PayconexCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.is_approved() {
            let transaction_id = item.response.get_transaction_id(item.http_code)?;
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Charged,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(transaction_id),
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
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: error_code_string(&item.response.error_code),
                    message: error_message_string(
                        &item.response.error_message,
                        &item.response.authorization_message,
                    ),
                    reason: item.response.authorization_message.clone(),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ---- Void (REVERSAL) response ----
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayconexVoidResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl PayconexVoidResponse {
    fn is_approved(&self) -> bool {
        self.transaction_approved
    }
    fn get_transaction_id(&self, http_code: u16) -> CustomResult<String, ConnectorError> {
        self.transaction_id.clone().ok_or_else(|| {
            report!(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some("missing transaction_id in PayConex void response".to_string()),
            ))
        })
    }
}

impl TryFrom<ResponseRouterData<PayconexVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PayconexVoidResponse, Self>) -> Result<Self, Self::Error> {
        if item.response.is_approved() {
            let transaction_id = item.response.get_transaction_id(item.http_code)?;
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Voided,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(transaction_id),
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
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: error_code_string(&item.response.error_code),
                    message: error_message_string(
                        &item.response.error_message,
                        &item.response.authorization_message,
                    ),
                    reason: item.response.authorization_message.clone(),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ---- PSync response (TSAPI) ----
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayconexSyncResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    /// PayConex TSAPI `found` flag: `true` when `GET_TRANSACTION_STATUS` located a
    /// transaction record for the queried id. `false` means no record exists yet, so
    /// the status is treated as Pending (see `map_payment_status` / `map_refund_status`).
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub found: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl TryFrom<ResponseRouterData<PayconexSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PayconexSyncResponse, Self>) -> Result<Self, Self::Error> {
        // PayConex GET_TRANSACTION_STATUS does not expose whether a manual authorization
        // has been captured, so mirror the Authorize intent: approved auto-capture ->
        // Charged, approved manual-capture -> Authorized, no record yet -> Pending.
        let status = map_payment_status(
            item.response.found,
            item.response.transaction_approved,
            item.router_data.request.is_auto_capture(),
        );

        let response = match status {
            common_enums::AttemptStatus::Failure => Err(ErrorResponse {
                code: error_code_string(&item.response.error_code),
                message: error_message_string(
                    &item.response.error_message,
                    &item.response.authorization_message,
                ),
                reason: item.response.authorization_message.clone(),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: item.response.transaction_id.clone(),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item
                    .response
                    .transaction_id
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
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

// ---- Refund / RSync response ----
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayconexRefundResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    /// PayConex TSAPI `found` flag: `true` when `GET_TRANSACTION_STATUS` located a
    /// transaction record for the queried id. `false` means no record exists yet, so
    /// the status is treated as Pending (see `map_payment_status` / `map_refund_status`).
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub found: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl TryFrom<ResponseRouterData<PayconexRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PayconexRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = map_refund_status(true, item.response.transaction_approved);
        let response = match refund_status {
            common_enums::RefundStatus::Failure => Err(ErrorResponse {
                code: error_code_string(&item.response.error_code),
                message: error_message_string(
                    &item.response.error_message,
                    &item.response.authorization_message,
                ),
                reason: item.response.authorization_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id.clone().ok_or_else(|| {
                    report!(ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("missing transaction_id in PayConex refund response".to_string()),
                    ))
                })?,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
        };
        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayconexRefundSyncResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    /// PayConex TSAPI `found` flag: `true` when `GET_TRANSACTION_STATUS` located a
    /// transaction record for the queried id. `false` means no record exists yet, so
    /// the status is treated as Pending (see `map_payment_status` / `map_refund_status`).
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub found: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl TryFrom<ResponseRouterData<PayconexRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PayconexRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status =
            map_refund_status(item.response.found, item.response.transaction_approved);
        let response = match refund_status {
            common_enums::RefundStatus::Failure => Err(ErrorResponse {
                code: error_code_string(&item.response.error_code),
                message: error_message_string(
                    &item.response.error_message,
                    &item.response.authorization_message,
                ),
                reason: item.response.authorization_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            _ => Ok(RefundsResponseData {
                connector_refund_id: item
                    .response
                    .transaction_id
                    .clone()
                    .unwrap_or_else(|| item.router_data.request.connector_refund_id.clone()),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
        };
        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}
