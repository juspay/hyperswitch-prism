use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{
    pii,
    request::Method,
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, MinorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
    },
    errors,
    payment_method_data::{
        BankDebitData, BankRedirectData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

// Import the connector's RouterData wrapper type created by the macro
use super::Shift4RouterData;

/// Convert epoch days (days since 1970-01-01) to (year, month, day)
fn epoch_days_to_date(days: u64) -> (u64, u64, u64) {
    // Algorithm based on civil_from_days
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[derive(Debug, Clone)]
pub struct Shift4AuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for Shift4AuthType {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Shift4 { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::ConnectorError::FailedToObtainAuthType
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shift4ErrorResponse {
    pub error: ApiErrorResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Shift4PaymentsRequest<T: PaymentMethodDataTypes> {
    /// Card/BankRedirect/Wallet payments via /charges endpoint
    Standard(Shift4StandardPaymentsRequest<T>),
    /// ACH Bank Debit payments via /ach/sale endpoint
    AchSale(Shift4AchSaleRequest),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4StandardPaymentsRequest<T: PaymentMethodDataTypes> {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(flatten)]
    pub payment_method: Shift4PaymentMethod<T>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Shift4PaymentMethod<T: PaymentMethodDataTypes> {
    Card(Shift4CardPayment<T>),
    BankRedirect(Shift4BankRedirectPayment),
}

// ===== ACH BANK DEBIT (SALE) STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4AchSaleRequest {
    pub date_time: String,
    pub amount: Shift4AchAmount,
    pub transaction: Shift4AchTransaction,
    pub ach: Shift4AchDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Shift4AchAmount {
    pub total: FloatMajorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4AchTransaction {
    pub invoice: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4AchDetails {
    pub account_number: Secret<String>,
    pub routing_number: Secret<String>,
    pub account_type: String,
    pub account_holder_name: Secret<String>,
}

// ===== ACH SALE RESPONSE STRUCTURES =====

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4AchSaleResponse {
    pub result: Vec<Shift4AchSaleResultItem>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4AchSaleResultItem {
    pub date_time: Option<String>,
    pub amount: Option<Shift4AchResponseAmount>,
    pub merchant: Option<Shift4AchMerchant>,
    pub token: Option<Shift4AchToken>,
    pub transaction: Option<Shift4AchTransactionResponse>,
    pub notification_id: Option<String>,
    pub ach: Option<Shift4AchResponseDetails>,
    pub server: Option<Shift4AchServer>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4AchResponseAmount {
    pub total: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4AchMerchant {
    pub mid: Option<i64>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4AchToken {
    pub value: Option<String>,
    #[serde(rename = "type")]
    pub token_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4AchTransactionResponse {
    pub auth_source: Option<String>,
    pub invoice: Option<String>,
    pub response_code: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4AchResponseDetails {
    pub balance_verification_result: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4AchServer {
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CardPayment<T: PaymentMethodDataTypes> {
    pub card: Shift4CardData<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CardData<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub cardholder_name: Secret<String>,
}

// BankRedirect Payment Structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4BankRedirectPayment {
    pub payment_method: Shift4BankRedirectMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<Shift4FlowRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4FlowRequest {
    pub return_url: String,
}

#[derive(Debug, Serialize)]
pub struct Shift4BankRedirectMethod {
    #[serde(rename = "type")]
    pub payment_type: String,
    pub billing: Shift4Billing,
}

#[derive(Debug, Serialize)]
pub struct Shift4Billing {
    pub name: Option<Secret<String>>,
    pub email: Option<pii::Email>,
    pub address: Option<Shift4Address>,
}

#[derive(Debug, Serialize)]
pub struct Shift4Address {
    pub line1: Option<Secret<String>>,
    pub line2: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country: Option<String>,
}

// BankRedirect Data Transformation
impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for Shift4BankRedirectMethod
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_type = match &router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(bank_redirect_data) => match bank_redirect_data {
                BankRedirectData::Ideal { .. } => "ideal",
                BankRedirectData::Eps { .. } => "eps",
                _ => {
                    return Err(error_stack::report!(errors::ConnectorError::NotSupported {
                        message: format!(
                            "BankRedirect type {:?} is not supported by Shift4",
                            bank_redirect_data
                        ),
                        connector: "Shift4",
                    }))
                }
            },
            _ => {
                return Err(error_stack::report!(errors::ConnectorError::NotSupported {
                    message: "Non-bank redirect payment method".to_string(),
                    connector: "Shift4",
                }))
            }
        };

        // Extract billing information
        let billing = router_data
            .resource_common_data
            .address
            .get_payment_method_billing();
        let name = billing.as_ref().and_then(|b| b.get_optional_full_name());

        // Extract email from request - prioritize from payment data, fallback to address
        let email = router_data
            .request
            .email
            .as_ref()
            .cloned()
            .or_else(|| billing.as_ref().and_then(|b| b.email.as_ref()).cloned());

        let address = billing
            .as_ref()
            .and_then(|b| b.address.as_ref())
            .map(|addr| Shift4Address {
                line1: addr.line1.clone(),
                line2: addr.line2.clone(),
                city: addr.city.clone(),
                state: addr.state.clone(),
                zip: addr.zip.clone(),
                country: addr.country.as_ref().map(|c| c.to_string()),
            });

        let billing_info = Shift4Billing {
            name,
            email,
            address,
        };

        Ok(Self {
            payment_type: payment_type.to_string(),
            billing: billing_info,
        })
    }
}

/// Helper function to build an ACH Sale request for Shift4 BankDebit payments
fn build_ach_sale_request<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    bank_debit_data: &BankDebitData,
) -> Result<Shift4PaymentsRequest<T>, error_stack::Report<errors::ConnectorError>> {
    match bank_debit_data {
        BankDebitData::AchBankDebit {
            account_number,
            routing_number,
            bank_account_holder_name,
            bank_type,
            bank_holder_type,
            ..
        } => {
            // Convert amount from minor units to major unit float (e.g., 13587 cents -> 135.87)
            let converter = FloatMajorUnitForConnector;
            let total = converter
                .convert(item.request.minor_amount, item.request.currency)
                .change_context(errors::ConnectorError::AmountConversionFailed)?;

            // Determine account type: PC (Personal Checking), PS (Personal Savings),
            // CC (Corporate Checking), CS (Corporate Savings)
            let account_type = match (bank_holder_type, bank_type) {
                (
                    Some(common_enums::BankHolderType::Personal),
                    Some(common_enums::BankType::Savings),
                ) => "PS",
                (
                    Some(common_enums::BankHolderType::Business),
                    Some(common_enums::BankType::Checking),
                ) => "CC",
                (
                    Some(common_enums::BankHolderType::Business),
                    Some(common_enums::BankType::Savings),
                ) => "CS",
                // Default to Personal Checking
                _ => "PC",
            };

            // Get account holder name, fallback to billing name
            let account_holder_name = bank_account_holder_name
                .clone()
                .or_else(|| {
                    item.resource_common_data
                        .address
                        .get_payment_method_billing()
                        .and_then(|billing| billing.get_optional_full_name())
                })
                .ok_or_else(|| {
                    error_stack::report!(errors::ConnectorError::MissingRequiredField {
                        field_name: "bank_account_holder_name"
                    })
                })?;

            // Use current UTC time in ISO 8601 format
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let secs = now.as_secs();
            // Simple ISO 8601 UTC format: YYYY-MM-DDTHH:MM:SS.000+00:00
            let days = secs / 86400;
            let day_secs = secs % 86400;
            let hours = day_secs / 3600;
            let minutes = (day_secs % 3600) / 60;
            let seconds = day_secs % 60;
            // Approximate date calculation from epoch days
            let (year, month, day) = epoch_days_to_date(days);
            let date_time = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000+00:00",
                year, month, day, hours, minutes, seconds
            );

            // Use connector_request_reference_id as invoice number (max 10 chars)
            let invoice = item
                .resource_common_data
                .connector_request_reference_id
                .chars()
                .take(10)
                .collect::<String>();

            Ok(Shift4PaymentsRequest::AchSale(Shift4AchSaleRequest {
                date_time,
                amount: Shift4AchAmount { total },
                transaction: Shift4AchTransaction { invoice },
                ach: Shift4AchDetails {
                    account_number: account_number.clone(),
                    routing_number: routing_number.clone(),
                    account_type: account_type.to_string(),
                    account_holder_name,
                },
                source_ip: None,
            }))
        }
        _ => Err(error_stack::report!(errors::ConnectorError::NotSupported {
            message: "Only ACH Bank Debit is supported for Shift4".to_string(),
            connector: "Shift4",
        })),
    }
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for Shift4PaymentsRequest<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // Route BankDebit payments to the ACH Sale request builder
        if let PaymentMethodData::BankDebit(ref bank_debit_data) = item.request.payment_method_data
        {
            return build_ach_sale_request(item, bank_debit_data);
        }

        let captured = item
            .request
            .is_auto_capture()
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Get cardholder name from address/billing info if available
                let cardholder_name = item
                    .resource_common_data
                    .address
                    .get_payment_method_billing()
                    .and_then(|billing| billing.get_optional_full_name())
                    .or_else(|| {
                        item.request
                            .customer_name
                            .as_ref()
                            .map(|name| Secret::new(name.clone()))
                    })
                    .ok_or_else(|| {
                        error_stack::report!(errors::ConnectorError::MissingRequiredField {
                            field_name: "billing_address.first_name"
                        })
                    })?;

                Shift4PaymentMethod::Card(Shift4CardPayment {
                    card: Shift4CardData {
                        number: card_data.card_number.clone(),
                        exp_month: card_data.card_exp_month.clone(),
                        exp_year: card_data.card_exp_year.clone(),
                        cardholder_name,
                    },
                })
            }
            PaymentMethodData::BankRedirect(_bank_redirect_data) => {
                let bank_redirect_method = Shift4BankRedirectMethod::try_from(item)?;
                let return_url = item.request.get_router_return_url().change_context(
                    errors::ConnectorError::MissingRequiredField {
                        field_name: "return_url",
                    },
                )?;

                Shift4PaymentMethod::BankRedirect(Shift4BankRedirectPayment {
                    payment_method: bank_redirect_method,
                    flow: Some(Shift4FlowRequest { return_url }),
                })
            }
            _ => {
                return Err(error_stack::report!(errors::ConnectorError::NotSupported {
                    message: "Payment method".to_string(),
                    connector: "Shift4",
                }))
            }
        };

        Ok(Self::Standard(Shift4StandardPaymentsRequest {
            amount: item.request.minor_amount,
            currency: item.request.currency,
            captured,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            payment_method,
        }))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Shift4PaymentsResponse {
    /// Standard card/redirect/wallet response from /charges endpoint
    Standard(Shift4StandardPaymentsResponse),
    /// ACH Sale response from /ach/sale endpoint
    AchSale(Shift4AchSaleResponse),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4StandardPaymentsResponse {
    pub id: String,
    pub currency: Currency,
    pub amount: MinorUnit,
    pub status: Shift4PaymentStatus,
    pub captured: bool,
    pub refunded: bool,
    pub flow: Option<FlowResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowResponse {
    pub next_action: Option<NextAction>,
    pub redirect: Option<RedirectResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectResponse {
    pub redirect_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NextAction {
    Redirect,
    Wait,
    None,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shift4PaymentStatus {
    Successful,
    Pending,
    Failed,
}

/// Helper: map a standard (card/redirect/wallet) Shift4 response to status + transaction ID + redirection
fn map_standard_response(
    response: &Shift4StandardPaymentsResponse,
) -> (AttemptStatus, String, Option<Box<RedirectForm>>) {
    let status = match response.status {
        Shift4PaymentStatus::Successful => {
            if response.captured {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        }
        Shift4PaymentStatus::Failed => AttemptStatus::Failure,
        Shift4PaymentStatus::Pending => {
            match response
                .flow
                .as_ref()
                .and_then(|flow| flow.next_action.as_ref())
            {
                Some(NextAction::Redirect) => AttemptStatus::AuthenticationPending,
                Some(NextAction::Wait) | Some(NextAction::None) | None => AttemptStatus::Pending,
            }
        }
    };

    let redirection_data = response
        .flow
        .as_ref()
        .and_then(|flow| flow.redirect.as_ref())
        .and_then(|redirect| {
            Url::parse(&redirect.redirect_url)
                .ok()
                .map(|url| Box::new(RedirectForm::from((url, Method::Get))))
        });

    (status, response.id.clone(), redirection_data)
}

/// Helper: map an ACH Sale response to status + transaction ID
fn map_ach_sale_response(response: &Shift4AchSaleResponse) -> (AttemptStatus, String) {
    let result = response.result.first();

    let status = result
        .and_then(|r| r.transaction.as_ref())
        .and_then(|t| t.response_code.as_ref())
        .map(|code| match code.as_str() {
            // A = Approved, S = Success
            "A" | "S" => AttemptStatus::Charged,
            // P = Pending
            "P" => AttemptStatus::Pending,
            // D = Declined, anything else = failure
            _ => AttemptStatus::Failure,
        })
        .unwrap_or(AttemptStatus::Failure);

    // Use notification_id or token value as the transaction ID
    let transaction_id = result
        .and_then(|r| r.notification_id.clone())
        .or_else(|| result.and_then(|r| r.token.as_ref().and_then(|t| t.value.clone())))
        .unwrap_or_else(|| "unknown".to_string());

    (status, transaction_id)
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, transaction_id, redirection_data) = match &item.response {
            Shift4PaymentsResponse::Standard(standard) => {
                let (s, id, rd) = map_standard_response(standard);
                (s, id, rd)
            }
            Shift4PaymentsResponse::AchSale(ach) => {
                let (s, id) = map_ach_sale_response(ach);
                (s, id, None)
            }
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(transaction_id),
                incremental_authorization_allowed: None,
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

// PSync response transformation - reuses Shift4PaymentsResponse and status mapping logic
impl TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, transaction_id) = match &item.response {
            Shift4PaymentsResponse::Standard(standard) => {
                let (s, id, _) = map_standard_response(standard);
                (s, id)
            }
            Shift4PaymentsResponse::AchSale(ach) => map_ach_sale_response(ach),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(transaction_id),
                incremental_authorization_allowed: None,
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

// Capture response transformation - reuses Shift4PaymentsResponse
impl TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, transaction_id) = match &item.response {
            Shift4PaymentsResponse::Standard(standard) => {
                let (s, id, _) = map_standard_response(standard);
                (s, id)
            }
            Shift4PaymentsResponse::AchSale(ach) => map_ach_sale_response(ach),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(transaction_id),
                incremental_authorization_allowed: None,
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

// ===== REFUND FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4RefundRequest {
    pub charge_id: String,
    pub amount: MinorUnit,
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for Shift4RefundRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            charge_id: item.request.connector_transaction_id.clone(),
            amount: item.request.minor_refund_amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4RefundResponse {
    pub id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    pub charge: String,
    pub status: Shift4RefundStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shift4RefundStatus {
    Successful,
    Failed,
    Processing,
}

impl TryFrom<ResponseRouterData<Shift4RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<Shift4RefundResponse, Self>) -> Result<Self, Self::Error> {
        // CRITICAL: Explicitly check the status field from the response
        // Do NOT assume success based solely on HTTP 200 response
        let refund_status = match item.response.status {
            Shift4RefundStatus::Successful => RefundStatus::Success,
            Shift4RefundStatus::Failed => RefundStatus::Failure,
            Shift4RefundStatus::Processing => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// RSync (Refund Sync) response transformation - reuses Shift4RefundResponse
impl TryFrom<ResponseRouterData<Shift4RefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<Shift4RefundResponse, Self>) -> Result<Self, Self::Error> {
        // CRITICAL: Explicitly check the status field from the response
        // Do NOT assume success based solely on HTTP 200 response
        let refund_status = match item.response.status {
            Shift4RefundStatus::Successful => RefundStatus::Success,
            Shift4RefundStatus::Failed => RefundStatus::Failure,
            Shift4RefundStatus::Processing => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== SYNC REQUEST STRUCTURES =====
// Sync operations (GET requests) typically don't send a body, but we need these for the macro

#[derive(Debug, Serialize, Default)]
pub struct Shift4PSyncRequest {}

#[derive(Debug, Serialize, Default)]
pub struct Shift4RSyncRequest {}

// ===== MACRO-COMPATIBLE TRYFROM IMPLEMENTATIONS =====
// The macro creates a Shift4RouterData wrapper type. We need TryFrom implementations
// that work with this wrapper.

// PSync Request - converts from Shift4RouterData to empty request struct
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for Shift4PSyncRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        _item: Shift4RouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

// RSync Request - converts from Shift4RouterData to empty request struct
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for Shift4RSyncRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        _item: Shift4RouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

// Authorize Request - delegates to existing implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Shift4PaymentsRequest<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom<&RouterDataV2> implementation
        Self::try_from(&item.router_data)
    }
}

// Capture Request - we need a separate request type
#[derive(Debug, Serialize)]
pub struct Shift4CaptureRequest {
    // Shift4 capture is done via POST to /charges/{id}/capture with no body
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for Shift4CaptureRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        _item: Shift4RouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

// Refund Request - delegates to existing implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for Shift4RefundRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom<&RouterDataV2> implementation
        Self::try_from(&item.router_data)
    }
}
