use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, CountryAlpha2, Currency, RefundStatus};
use common_utils::{
    pii::Email,
    types::{MinorUnit, Money},
};
use domain_types::{
    connector_flow::{
        Authorize, CreateConnectorCustomer, CreateOrder, GetConnectorCustomer, PSync, RSync, Refund,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, EventType, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, ResponseId, WebhookDetailsResponse,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::PaymentSynIntegrityObject,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::connectors::glomopay::GlomopayAmountConvertor;

// ============================================================================
// Supported countries (see SupportedCountries.md)
// ============================================================================

// Countries Glomo will accept in the customer `country` field (ISO 3166-1
// alpha-3). Anything outside this set is rejected server-side; we fail fast
// with a NotSupported error instead of forwarding.
const GLOMOPAY_SUPPORTED_COUNTRIES: &[&str] = &[
    "ABW", "AGO", "AIA", "ALB", "AND", "ARE", "ARG", "ARM", "ASM", "ATA", "ATF", "ATG", "AUS",
    "AUT", "AZE", "BEL", "BEN", "BGD", "BHR", "BHS", "BLZ", "BMU", "BOL", "BRA", "BRB", "BRN",
    "BTN", "BVT", "BWA", "CAN", "CCK", "CHE", "CHL", "CIV", "COG", "COK", "COL", "COM", "CPV",
    "CRI", "CXR", "CYM", "CYP", "CZE", "DEU", "DJI", "DMA", "DNK", "DOM", "DZA", "ECU", "EGY",
    "ESH", "ESP", "EST", "FIN", "FJI", "FLK", "FRA", "FRO", "FSM", "GAB", "GBR", "GEO", "GHA",
    "GIB", "GLP", "GMB", "GNQ", "GRC", "GRD", "GRL", "GTM", "GUF", "GUM", "GUY", "HMD", "HND",
    "HUN", "IDN", "IND", "IOT", "IRL", "ISL", "ISR", "ITA", "JOR", "JPN", "KAZ", "KGZ", "KHM",
    "KIR", "KNA", "KOR", "KWT", "LAO", "LBR", "LCA", "LIE", "LKA", "LSO", "LTU", "LUX", "LVA",
    "MAC", "MAR", "MCO", "MDG", "MDV", "MEX", "MHL", "MKD", "MLT", "MNG", "MNP", "MRT", "MSR",
    "MTQ", "MUS", "MWI", "MYS", "MYT", "NCL", "NER", "NFK", "NIU", "NLD", "NOR", "NPL", "NRU",
    "NZL", "OMN", "PAK", "PAN", "PCN", "PER", "PLW", "PNG", "POL", "PRI", "PRT", "PRY", "PYF",
    "QAT", "REU", "ROU", "RWA", "SAU", "SGP", "SGS", "SHN", "SJM", "SLB", "SLE", "SLV", "SMR",
    "SPM", "STP", "SUR", "SVK", "SVN", "SWE", "SWZ", "SYC", "TCA", "TCD", "TGO", "THA", "TJK",
    "TKL", "TKM", "TON", "TTO", "TUV", "TWN", "UMI", "URY", "USA", "UZB", "VAT", "VCT", "VGB",
    "VIR", "VUT", "WLF", "WSM", "ZAF", "ZMB",
];

// Countries where Glomo requires the customer `pincode` field. Absent zip
// with one of these countries is a server-side 4xx.
const GLOMOPAY_PINCODE_REQUIRED_COUNTRIES: &[&str] = &[
    "AIA", "ALB", "AND", "ARG", "ARM", "ASM", "ATA", "ATF", "AUS", "AUT", "AZE", "BEL", "BGD",
    "BHR", "BLZ", "BMU", "BOL", "BRA", "BRB", "BRN", "BTN", "BVT", "CAN", "CCK", "CHE", "CHL",
    "CIV", "COK", "COL", "CPV", "CRI", "CXR", "CYM", "CYP", "CZE", "DEU", "DMA", "DNK", "DOM",
    "DZA", "ECU", "EGY", "ESH", "ESP", "EST", "FIN", "FJI", "FLK", "FRA", "FRO", "FSM", "GAB",
    "GBR", "GEO", "GIB", "GLP", "GMB", "GRC", "GRL", "GTM", "GUF", "GUM", "HMD", "HND", "HUN",
    "IDN", "IND", "IOT", "IRL", "ISL", "ISR", "ITA", "JOR", "JPN", "KAZ", "KGZ", "KHM", "KIR",
    "KOR", "KWT", "LAO", "LCA", "LIE", "LKA", "LSO", "LTU", "LUX", "LVA", "MAR", "MCO", "MDG",
    "MDV", "MEX", "MHL", "MKD", "MLT", "MNG", "MNP", "MSR", "MTQ", "MUS", "MYS", "MYT", "NCL",
    "NER", "NFK", "NLD", "NOR", "NPL", "NZL", "OMN", "PAK", "PAN", "PCN", "PER", "PLW", "PNG",
    "POL", "PRI", "PRT", "PRY", "PYF", "QAT", "REU", "ROU", "SAU", "SGP", "SGS", "SHN", "SJM",
    "SLB", "SLV", "SMR", "SPM", "SVK", "SVN", "SWE", "SWZ", "TCA", "THA", "TJK", "TKM", "TON",
    "TTO", "TWN", "UMI", "URY", "USA", "UZB", "VAT", "VCT", "VGB", "VIR", "WLF", "ZAF", "ZMB",
];

// ============================================================================
// Auth
// ============================================================================

#[derive(Debug, Clone)]
pub struct GlomopayAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GlomopayAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Glomopay { api_key, .. } => Ok(Self {
                api_key: api_key.clone(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// ============================================================================
// Error response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlomopayErrorResponse {
    pub error: Option<String>,
    pub message: Option<String>,
}

// ============================================================================
// Status enums
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopayPaymentStatus {
    InProgress,
    ActionRequired,
    Pending,
    Success,
    Failed,
}

impl From<GlomopayPaymentStatus> for AttemptStatus {
    fn from(status: GlomopayPaymentStatus) -> Self {
        match status {
            GlomopayPaymentStatus::InProgress => Self::AuthenticationPending,
            GlomopayPaymentStatus::ActionRequired | GlomopayPaymentStatus::Pending => Self::Pending,
            GlomopayPaymentStatus::Success => Self::Charged,
            GlomopayPaymentStatus::Failed => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopayRefundStatus {
    Success,
    Failed,
    UnderReview,
    Pending,
    ActionRequired,
}

impl From<GlomopayRefundStatus> for RefundStatus {
    fn from(status: GlomopayRefundStatus) -> Self {
        match status {
            GlomopayRefundStatus::Success => Self::Success,
            GlomopayRefundStatus::Failed => Self::Failure,
            GlomopayRefundStatus::Pending
            | GlomopayRefundStatus::ActionRequired
            | GlomopayRefundStatus::UnderReview => Self::Pending,
        }
    }
}

// ============================================================================
// Webhooks
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopayWebhookEventType {
    Success,
    InProgress,
    ActionRequired,
    Failed,
    FundsAvailable,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookFeeDetail {
    pub currency: Option<Currency>,
    pub amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookFees {
    pub txn_fee: Option<GlomopayWebhookFeeDetail>,
    pub fx_fee: Option<GlomopayWebhookFeeDetail>,
    pub referral_fee: Option<GlomopayWebhookFeeDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookPaymentData {
    pub id: String,
    pub payin_id: Option<String>,
    pub requested_amount: Option<MinorUnit>,
    pub requested_currency: Option<Currency>,
    pub payment_amount: Option<MinorUnit>,
    pub payment_currency: Option<Currency>,
    pub fees: Option<GlomopayWebhookFees>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_message: Option<String>,
    pub funds_available: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookPayload {
    pub entity_type: String,
    pub event_type: GlomopayWebhookEventType,
    pub data: GlomopayWebhookPaymentData,
}

impl GlomopayWebhookPayload {
    pub fn get_event_type(&self) -> EventType {
        match self.event_type {
            GlomopayWebhookEventType::Success | GlomopayWebhookEventType::FundsAvailable => {
                EventType::PaymentIntentSuccess
            }
            GlomopayWebhookEventType::Failed => EventType::PaymentIntentFailure,
            GlomopayWebhookEventType::InProgress => EventType::PaymentIntentProcessing,
            GlomopayWebhookEventType::ActionRequired => EventType::PaymentActionRequired,
        }
    }

    pub fn get_attempt_status(&self) -> AttemptStatus {
        match self.event_type {
            GlomopayWebhookEventType::Success | GlomopayWebhookEventType::FundsAvailable => {
                AttemptStatus::Charged
            }
            GlomopayWebhookEventType::Failed => AttemptStatus::Failure,
            GlomopayWebhookEventType::InProgress => AttemptStatus::AuthenticationPending,
            GlomopayWebhookEventType::ActionRequired => AttemptStatus::Pending,
        }
    }

    pub fn into_webhook_details_response(
        self,
        http_code: u16,
        raw_body: &[u8],
    ) -> WebhookDetailsResponse {
        let status = self.get_attempt_status();
        let (error_code, error_message) = match self.event_type {
            GlomopayWebhookEventType::Failed => (
                self.data.error_code.clone(),
                self.data
                    .error_message
                    .clone()
                    .or_else(|| self.data.error_description.clone()),
            ),
            _ => (None, None),
        };
        WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(self.data.id.clone())),
            status,
            connector_response_reference_id: Some(self.data.id.clone()),
            connector_request_reference_id: Some(self.data.id),
            mandate_reference: None,
            error_code,
            error_message,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
            status_code: http_code,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        }
    }
}

// ============================================================================
// Refund Webhooks
// ============================================================================

/// Minimal probe struct — reads only entity_type to route to the correct handler.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookEntityProbe {
    pub entity_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopayRefundWebhookEventType {
    Success,
    ActionRequired,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundWebhookFeeDetail {
    pub currency: Option<Currency>,
    pub amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundWebhookFees {
    pub txn_fees: Option<GlomopayRefundWebhookFeeDetail>,
    pub fx_fees: Option<GlomopayRefundWebhookFeeDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundWebhookData {
    pub id: String,
    pub payment_id: Option<String>,
    pub amount: Option<MinorUnit>,
    pub currency: Option<Currency>,
    pub fees: Option<GlomopayRefundWebhookFees>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundWebhookPayload {
    pub entity_type: String,
    pub event_type: GlomopayRefundWebhookEventType,
    pub data: GlomopayRefundWebhookData,
}

impl GlomopayRefundWebhookPayload {
    pub fn get_event_type(&self) -> EventType {
        match self.event_type {
            GlomopayRefundWebhookEventType::Success => EventType::RefundSuccess,
            GlomopayRefundWebhookEventType::Failed => EventType::RefundFailure,
            GlomopayRefundWebhookEventType::ActionRequired => EventType::RefundProcessing,
        }
    }

    pub fn get_refund_status(&self) -> RefundStatus {
        match self.event_type {
            GlomopayRefundWebhookEventType::Success => RefundStatus::Success,
            GlomopayRefundWebhookEventType::Failed => RefundStatus::Failure,
            GlomopayRefundWebhookEventType::ActionRequired => RefundStatus::Pending,
        }
    }

    pub fn into_refund_webhook_details_response(
        self,
        http_code: u16,
        raw_body: &[u8],
    ) -> RefundWebhookDetailsResponse {
        let (error_code, error_message) = match self.event_type {
            GlomopayRefundWebhookEventType::Failed => (
                self.data.error_code.clone(),
                self.data.error_message.clone(),
            ),
            _ => (None, None),
        };
        RefundWebhookDetailsResponse {
            connector_refund_id: Some(self.data.id.clone()),
            merchant_transaction_id: None,
            status: self.get_refund_status(),
            connector_response_reference_id: Some(self.data.id),
            error_code,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
            status_code: http_code,
            response_headers: None,
        }
    }
}

// ============================================================================
// GetConnectorCustomer (search by email — GET /customer?email_address=...)
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayCustomerItem {
    pub id: String,
    pub name: Option<Secret<String>>,
    pub email: Option<Email>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayGetCustomerResponse {
    pub data: Vec<GlomopayCustomerItem>,
}

impl TryFrom<ResponseRouterData<GlomopayGetCustomerResponse, Self>>
    for RouterDataV2<
        GetConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayGetCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        // If customer found in search results, return existing ID
        // If empty, return an error indicating the customer does not exist yet
        let router_response = match response.data.into_iter().next() {
            Some(customer) => Ok(ConnectorCustomerResponse {
                connector_customer_id: customer.id,
                status_code: item.http_code,
            }),
            None => Err(ErrorResponse {
                status_code: item.http_code,
                code: "CUSTOMER_NOT_FOUND".to_string(),
                message: "No existing customer found for the provided email".to_string(),
                reason: None,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };
        Ok(Self {
            response: router_response,
            ..item.router_data
        })
    }
}

// ============================================================================
// CreateConnectorCustomer
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayCreateCustomerRequest {
    pub name: Secret<String>,
    pub customer_type: String,
    pub email: Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    pub address: Secret<String>,
    pub city: Secret<String>,
    pub state: Secret<String>,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pincode: Option<Secret<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayCreateCustomerResponse {
    pub id: String,
    pub name: Option<Secret<String>>,
    pub email: Option<Email>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for GlomopayCreateCustomerRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;
        let req = &router_data.request;
        let flow_data = &router_data.resource_common_data;

        // Prefer the explicit customer.name field, but fall back to the
        // billing address's first + last name (via get_optional_billing_full_name)
        // when the caller only sends address-level identity — a common pattern
        // for merchants that don't collect a separate customer profile before
        // checkout. Glomopay requires a non-empty name, so bail out only if
        // neither source is populated.
        let name = req
            .get_name()
            .or_else(|_| flow_data.get_billing_full_name())?;

        let email = req.get_email()?;

        let address = flow_data.get_billing_line1()?;

        let city = flow_data.get_billing_city()?;

        let state = flow_data.get_billing_state()?;

        let country =
            CountryAlpha2::from_alpha2_to_alpha3(flow_data.get_billing_country()?).to_string();

        if !GLOMOPAY_SUPPORTED_COUNTRIES.contains(&country.as_str()) {
            return Err(error_stack::report!(
                errors::IntegrationError::NotSupported {
                    message: format!("country '{country}'"),
                    connector: "glomopay",
                    context: Default::default(),
                }
            ));
        }

        let zip = flow_data.get_optional_billing_zip();

        if zip.is_none() && GLOMOPAY_PINCODE_REQUIRED_COUNTRIES.contains(&country.as_str()) {
            return Err(crate::utils::missing_field_err("billing.address.pincode")());
        }

        Ok(Self {
            name,
            customer_type: "individual".to_string(),
            email,
            phone: req.phone.clone(),
            address,
            city,
            state,
            country,
            pincode: zip,
        })
    }
}

impl TryFrom<ResponseRouterData<GlomopayCreateCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayCreateCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: response.id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ============================================================================
// CreateOrder
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayCreateOrderRequest {
    pub customer_id: String,
    pub currency: Currency,
    pub amount: MinorUnit,
    pub product: GlomopayProduct,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayProduct {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayCreateOrderResponse {
    pub id: String,
    pub status: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for GlomopayCreateOrderRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let amount = GlomopayAmountConvertor::convert(
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let customer_id = router_data
            .resource_common_data
            .connector_customer
            .clone()
            .ok_or_else(crate::utils::missing_field_err("connector_customer"))?;

        Ok(Self {
            customer_id,
            currency: router_data.request.currency,
            amount,
            product: GlomopayProduct {
                name: "Payment".to_string(),
                description: "Payment via Glomopay".to_string(),
            },
            request_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

impl TryFrom<ResponseRouterData<GlomopayCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let order_id = response.id.clone();

        // If Glomopay returns a non-proceedable order status, return early so
        // the caller does not attempt POST /payment against an order that is
        // not ready. Per order.md the enum is:
        //   active | paid | failed | action_required | under_review | expired
        // Only `active` (and, rarely, `paid`) are safe to proceed with.
        // `action_required`, `failed`, and `expired` are terminal failures.
        // `under_review` means the order is pending manual review — return Pending.
        if let Some(status) = response.status.as_deref() {
            if matches!(status, "under_review") {
                return Ok(Self {
                    response: Err(ErrorResponse {
                        code: "ORDER_UNDER_REVIEW".to_string(),
                        message: format!("Glomopay order '{}' is under review.", order_id),
                        reason: Some(format!("Glomopay order '{}' is under review.", order_id)),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Pending)),
                        connector_transaction_id: Some(order_id),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Pending,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                });
            }
            if matches!(status, "action_required" | "failed" | "expired") {
                let code = format!("ORDER_{}", status.to_uppercase());
                let message = format!(
                    "Glomopay order '{}' has status '{}' — cannot proceed with payment.",
                    order_id, status
                );
                return Ok(Self {
                    response: Err(ErrorResponse {
                        code,
                        message: message.clone(),
                        reason: Some(message),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                        connector_transaction_id: Some(order_id),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                });
            }
        }

        Ok(Self {
            response: Ok(PaymentCreateOrderResponse {
                connector_order_id: order_id.clone(),
                session_data: None,
            }),
            resource_common_data: PaymentFlowData {
                connector_order_id: Some(order_id.clone()),
                reference_id: Some(order_id),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================================
// Authorize
// ============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopaySequence {
    Initial,
    Subsequent,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayAuthorizeRequest {
    pub order_id: String,
    pub method: String,
    pub sequence: GlomopaySequence,
    pub card: Option<GlomopayCard>,
    pub callback_url: Option<String>,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayCard {
    pub holder_name: Option<Secret<String>>,
    pub number: Secret<String>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvv: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayAuthorizeResponse {
    pub payment_id: String,
    pub status: GlomopayPaymentStatus,
    pub next_steps: Option<Vec<GlomopayNextStep>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayNextStep {
    pub action: String,
    pub payload: Option<GlomopayNextStepPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayNextStepPayload {
    pub url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlomopayAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let order_id = router_data
            .resource_common_data
            .connector_order_id
            .clone()
            .ok_or_else(crate::utils::missing_field_err("connector_order_id"))?;

        let (method, card) = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                // Reject India-issued cards. Euler's BIN lookup may set
                // card_issuing_country as alpha-2 ("IN") or alpha-3 ("IND"),
                // so match both case-insensitively.
                if let Some(issuer_country) = card.card_issuing_country.as_deref() {
                    let normalised = issuer_country.trim().to_ascii_uppercase();
                    if normalised == "IN" || normalised == "IND" {
                        return Err(error_stack::report!(
                            errors::IntegrationError::NotSupported {
                                message: "India-issued cards are not supported".to_string(),
                                connector: "glomopay",
                                context: Default::default(),
                            }
                        ));
                    }
                }

                let expiry_year = card.get_card_expiry_year_2_digit()?;

                (
                    "card".to_string(),
                    Some(GlomopayCard {
                        holder_name: card.card_holder_name.clone(),
                        number: Secret::new(card.card_number.peek().to_string()),
                        expiry_month: card.card_exp_month.clone(),
                        expiry_year,
                        cvv: card.card_cvc.clone(),
                    }),
                )
            }
            other => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        format!(
                            "Glomopay Authorize does not support payment method variant: {other:?}"
                        ),
                        Default::default(),
                    )
                ));
            }
        };

        let callback_url = Some(
            router_data
                .resource_common_data
                .return_url
                .clone()
                .ok_or_else(crate::utils::missing_field_err("return_url"))?,
        );

        Ok(Self {
            order_id,
            method,
            // Only Initial (CIT) is supported in phase 1. Mandate / MIT
            // (Subsequent) lands in phase 2 alongside RepeatPayment and
            // SetupMandate — currently marked `not_implemented` in
            // glomopay.rs. Phase 2 will derive this from the request's
            // sequence context instead of hardcoding.
            sequence: GlomopaySequence::Initial,
            card,
            callback_url,
            request_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GlomopayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = match response.status {
            GlomopayPaymentStatus::ActionRequired => AttemptStatus::Failure,
            other => AttemptStatus::from(other),
        };

        let redirect_url = response
            .next_steps
            .as_ref()
            .and_then(|steps| steps.iter().find(|s| s.action == "redirect"))
            .and_then(|step| step.payload.as_ref())
            .and_then(|p| p.url.clone());

        // GlomoPay's redirect is a plain GET with all params (paymentId, authToken, redirectUrl)
        // in the URL's query string. An HTML `<form method="GET">` submission would strip the
        // action URL's query string and submit empty form fields, dropping every param.
        // Use RedirectForm::Uri so callers navigate directly to the URL as-is.
        let redirection_data = redirect_url.map(|url| Box::new(RedirectForm::Uri { uri: url }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.payment_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================================
// PSync
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayPaymentSyncItem {
    pub id: String,
    pub status: GlomopayPaymentStatus,
    // Merchant-facing amount/currency echoed by Glomopay in the sync
    // response. These are the fields the caller compares against its stored
    // transaction values in the mandatory-sync integrity check, so we
    // must surface them from the connector response rather than
    // echoing the request context.
    pub requested_amount: Option<MinorUnit>,
    pub requested_currency: Option<Currency>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayPaymentSyncResponse {
    pub data: Vec<GlomopayPaymentSyncItem>,
}

impl TryFrom<ResponseRouterData<GlomopayPaymentSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayPaymentSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let payment = response.data.into_iter().next().ok_or_else(|| {
            error_stack::report!(crate::utils::response_deserialization_fail(
                item.http_code,
                "glomopay: PSync response contained no payment entries",
            ))
        })?;

        let status = AttemptStatus::from(payment.status);
        let merchant_ref = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Surface Glomopay's merchant-facing `requested_amount` /
        // `requested_currency` so the caller's mandatory-sync integrity
        // check compares against the connector-confirmed values rather
        // than an echo of the request context. Both fields are documented
        // as always present on a successful GET /payment response; if
        // either is absent we leave the downstream fields empty rather than
        // fabricating an amount from request state — a silent tautology is
        // worse than a visible integrity failure.
        let (response_amount, integrity_object) =
            match (payment.requested_amount, payment.requested_currency) {
                (Some(amount), Some(currency)) => (
                    Some(Money { amount, currency }),
                    Some(PaymentSynIntegrityObject { amount, currency }),
                ),
                _ => (None, None),
            };

        // On a terminal Failed status, promote the item's error_code /
        // error_message / error_description into an ErrorResponse so
        // downstream consumers get actionable codes rather than an opaque
        // failure.
        let response = if matches!(payment.status, GlomopayPaymentStatus::Failed) {
            let code = payment
                .error_code
                .clone()
                .unwrap_or_else(|| "failed".to_string());
            let message = payment
                .error_message
                .clone()
                .or_else(|| payment.error_description.clone())
                .unwrap_or_else(|| "Glomopay reported payment failure".to_string());
            Err(ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                connector_transaction_id: Some(payment.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(payment.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(merchant_ref),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                amount: response_amount,
                ..item.router_data.resource_common_data
            },
            request: PaymentsSyncData {
                integrity_object,
                ..item.router_data.request
            },
            ..item.router_data
        })
    }
}

// ============================================================================
// Refund
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayRefundRequest {
    pub payment_id: String,
    pub reason: String,
    pub amount: MinorUnit,
    pub request_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundResponse {
    pub id: String,
    pub status: GlomopayRefundStatus,
    pub amount: Option<MinorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for GlomopayRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let amount = GlomopayAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        let reason = router_data
            .request
            .reason
            .clone()
            .unwrap_or_else(|| "Requested By Customer".to_string());

        Ok(Self {
            payment_id: router_data.request.connector_transaction_id.clone(),
            reason,
            amount,
            request_id: router_data.request.refund_id.clone(),
        })
    }
}

impl TryFrom<ResponseRouterData<GlomopayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let refund_status = RefundStatus::from(response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
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

// ============================================================================
// RSync
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundSyncItem {
    pub id: String,
    pub status: GlomopayRefundStatus,
    pub amount: Option<MinorUnit>,
    pub utr: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundSyncResponse {
    pub data: Vec<GlomopayRefundSyncItem>,
}

impl TryFrom<ResponseRouterData<GlomopayRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;
        let connector_refund_id = router_data.request.connector_refund_id.clone();

        let refund_entry = response.data.iter().find(|r| r.id == connector_refund_id);

        // If RSync returns no matching refund, default to Pending so the
        // next sync retries rather than prematurely marking Success/Failure
        // — Glomopay's list endpoint may lag due to eventual consistency.
        let (refund_status, resolved_refund_id, acquirer_reference_number) = match refund_entry {
            Some(entry) => (
                RefundStatus::from(entry.status),
                entry.id.clone(),
                entry.utr.clone(),
            ),
            None => (RefundStatus::Pending, connector_refund_id, None),
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: resolved_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}
