use common_enums;
use domain_types::{
    connector_flow::{Authorize, CreateOrder},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::{report, Report};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// ============================================================================
// Authentication
// ============================================================================

#[derive(Debug, Clone)]
pub struct CashfreeAuthType {
    pub app_id: Secret<String>,     // X-Client-Id
    pub secret_key: Secret<String>, // X-Client-Secret
}

impl TryFrom<&ConnectorSpecificConfig> for CashfreeAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Cashfree {
                app_id, secret_key, ..
            } => Ok(Self {
                app_id: app_id.to_owned(),
                secret_key: secret_key.to_owned(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: Default::default()
            })),
        }
    }
}

// ============================================================================
// Error Response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashfreeErrorResponse {
    pub message: String,
    pub code: String,
    #[serde(rename = "type")]
    pub error_type: String,
}

// ============================================================================
// Order Creation (Phase 1)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct CashfreeOrderCreateRequest {
    pub order_id: String,
    pub order_amount: f64,
    pub order_currency: String,
    pub customer_details: CashfreeCustomerDetails,
    pub order_meta: CashfreeOrderMeta,
    pub order_note: Option<String>,
    pub order_expiry_time: Option<String>,
}

// Supporting types for Order Response (missing from original implementation)
#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeOrderCreateUrlResponse {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeOrderTagsType {
    pub metadata1: Option<String>,
    pub metadata2: Option<String>,
    pub metadata3: Option<String>,
    pub metadata4: Option<String>,
    pub metadata5: Option<String>,
    pub metadata6: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeOrderSplitsType {
    pub vendor_id: String,
    pub amount: f64,
    pub percentage: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeCustomerDetails {
    pub customer_id: String,
    pub customer_email: Option<String>,
    pub customer_phone: Secret<String>,
    pub customer_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeOrderMeta {
    pub return_url: String,
    pub notify_url: String,
    pub payment_methods: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeOrderCreateResponse {
    pub payment_session_id: String, // KEY: Used in Authorize flow
    pub cf_order_id: i64,
    pub order_id: String,
    pub entity: String, // ADDED: Missing field from Haskell
    pub order_amount: f64,
    pub order_currency: String,
    pub order_status: String,
    pub order_expiry_time: String,  // ADDED: Missing field from Haskell
    pub order_note: Option<String>, // ADDED: Missing optional field from Haskell
    pub customer_details: CashfreeCustomerDetails,
    pub order_meta: CashfreeOrderMeta,
    pub payments: CashfreeOrderCreateUrlResponse, // ADDED: Missing field from Haskell
    pub settlements: CashfreeOrderCreateUrlResponse, // ADDED: Missing field from Haskell
    pub refunds: CashfreeOrderCreateUrlResponse,  // ADDED: Missing field from Haskell
    pub order_tags: Option<CashfreeOrderTagsType>, // ADDED: Missing optional field from Haskell
    pub order_splits: Option<Vec<CashfreeOrderSplitsType>>, // ADDED: Missing optional field from Haskell
}

// ADDED: Union type for handling success/failure responses (matches Haskell pattern)
// #[derive(Debug, Deserialize)]
// #[serde(untagged)]
// pub enum CashfreeOrderCreateResponseWrapper {
//     Success(CashfreeOrderCreateResponse),
//     Error(CashfreeErrorResponse),
// }

// ============================================================================
// Payment Authorization (Phase 2)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct CashfreePaymentRequest {
    pub payment_session_id: String, // From order creation response
    pub payment_method: CashfreePaymentMethod,
    pub payment_surcharge: Option<CashfreePaymentSurcharge>,
}

#[derive(Debug, Serialize)]
pub struct CashfreePaymentMethod {
    pub upi: Option<CashfreeUpiDetails>,
    // ADDED: All other payment methods (set to None for UPI-only implementation)
    // This matches Haskell CashfreePaymentMethodType structure exactly
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<()>, // CashFreeAPPType - None for UPI-only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netbanking: Option<()>, // CashFreeNBType - None for UPI-only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<()>, // CashFreeCARDType - None for UPI-only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emi: Option<()>, // CashfreeEmiType - None for UPI-only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paypal: Option<()>, // CashfreePaypalType - None for UPI-only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paylater: Option<()>, // CashFreePaylaterType - None for UPI-only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardless_emi: Option<()>, // CashFreeCardlessEmiType - None for UPI-only
}

fn secret_is_empty(secret: &Secret<String>) -> bool {
    secret.peek().is_empty()
}

#[derive(Debug, Serialize)]
pub struct CashfreeUpiDetails {
    pub channel: String, // "link" for Intent, "collect" for Collect
    #[serde(skip_serializing_if = "secret_is_empty")]
    pub upi_id: Secret<String>, // VPA for collect, empty for intent
}

#[derive(Debug, Serialize)]
pub struct CashfreePaymentSurcharge {
    pub surcharge_amount: f64,
    pub surcharge_percentage: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreePaymentResponse {
    pub payment_method: String,
    pub channel: String,
    pub action: String,
    pub data: CashfreeResponseData,
    pub cf_payment_id: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeResponseData {
    pub url: Option<String>,
    pub payload: Option<CashfreePayloadData>,
    pub content_type: Option<String>,
    pub method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreePayloadData {
    #[serde(rename = "default")]
    pub default_link: String, // Universal deep link for Intent
    pub gpay: Option<String>,
    pub phonepe: Option<String>,
    pub paytm: Option<String>,
    pub bhim: Option<String>,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn get_cashfree_payment_method_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<CashfreePaymentMethod, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::Upi(upi_data) => {
            match upi_data {
                domain_types::payment_method_data::UpiData::UpiCollect(collect_data) => {
                    // Extract VPA for collect flow - maps to upi_id field in Cashfree
                    let vpa = collect_data
                        .vpa_id
                        .as_ref()
                        .map(|vpa| vpa.peek().to_string())
                        .unwrap_or_default();

                    if vpa.is_empty() {
                        return Err(IntegrationError::MissingRequiredField {
                            field_name: "vpa_id for UPI collect",
                            context: Default::default(),
                        });
                    }

                    Ok(CashfreePaymentMethod {
                        upi: Some(CashfreeUpiDetails {
                            channel: "collect".to_string(),
                            upi_id: Secret::new(vpa),
                        }),
                        app: None,
                        netbanking: None,
                        card: None,
                        emi: None,
                        paypal: None,
                        paylater: None,
                        cardless_emi: None,
                    })
                }
                domain_types::payment_method_data::UpiData::UpiIntent(_)
                | domain_types::payment_method_data::UpiData::UpiQr(_) => {
                    // Intent flow: channel = "link", no UPI ID needed
                    Ok(CashfreePaymentMethod {
                        upi: Some(CashfreeUpiDetails {
                            channel: "link".to_string(),
                            upi_id: Secret::new("".to_string()),
                        }),
                        app: None,
                        netbanking: None,
                        card: None,
                        emi: None,
                        paypal: None,
                        paylater: None,
                        cardless_emi: None,
                    })
                }
            }
        }
        _ => Err(IntegrationError::NotSupported {
            message: "Only UPI payment methods are supported for Cashfree V3".to_string(),
            connector: "Cashfree",
            context: Default::default(),
        }),
    }
}

// ============================================================================
// Request Transformations
// ============================================================================

// TryFrom implementation for macro-generated CashfreeRouterData wrapper
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for CashfreeOrderCreateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Convert MinorUnit to FloatMajorUnit properly
        let amount_i64 = wrapper.router_data.request.amount.get_amount_as_i64();
        #[allow(clippy::as_conversions)]
        let converted_amount = common_utils::types::FloatMajorUnit(amount_i64 as f64 / 100.0);
        Self::try_from((converted_amount, &wrapper.router_data))
    }
}

// Keep the original TryFrom for backward compatibility
impl
    TryFrom<
        &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    > for CashfreeOrderCreateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> Result<Self, Self::Error> {
        // Convert MinorUnit to FloatMajorUnit properly
        let amount_i64 = item.request.amount.get_amount_as_i64();
        #[allow(clippy::as_conversions)]
        let converted_amount = common_utils::types::FloatMajorUnit(amount_i64 as f64 / 100.0);
        Self::try_from((converted_amount, item))
    }
}

impl
    TryFrom<(
        common_utils::types::FloatMajorUnit,
        &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    )> for CashfreeOrderCreateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        (converted_amount, item): (
            common_utils::types::FloatMajorUnit,
            &RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        let billing = item
            .resource_common_data
            .address
            .get_payment_method_billing()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing_address",
                context: Default::default(),
            })?;

        // Build customer details
        let customer_details = CashfreeCustomerDetails {
            customer_id: item
                .resource_common_data
                .customer_id
                .as_ref()
                .map(|id| id.get_string_repr().to_string())
                .unwrap_or_else(|| "guest".to_string()),
            customer_email: billing.email.as_ref().map(|e| e.peek().to_string()),
            customer_phone: Secret::new(
                billing
                    .phone
                    .as_ref()
                    .and_then(|phone| phone.number.as_ref())
                    .map(|number| number.peek().to_string())
                    .unwrap_or_else(|| "9999999999".to_string()),
            ),
            customer_name: billing.get_optional_full_name().map(|name| name.expose()),
        };

        // Build order meta with return and notify URLs
        let return_url = item.resource_common_data.return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            },
        )?;

        // Get webhook URL from request - required for Cashfree V3
        let notify_url =
            item.request
                .webhook_url
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "webhook_url",
                    context: Default::default(),
                })?;

        let order_meta = CashfreeOrderMeta {
            return_url,
            notify_url,
            payment_methods: Some("upi".to_string()),
        };

        Ok(Self {
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(), // FIXED: Use payment_id not connector_request_reference_id
            order_amount: converted_amount.0,
            order_currency: item.request.currency.to_string(),
            customer_details,
            order_meta,
            order_note: item.resource_common_data.description.clone(),
            order_expiry_time: None,
        })
    }
}

// TryFrom implementation for macro-generated CashfreeRouterData wrapper
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for CashfreePaymentRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

// Keep original TryFrom implementation for backward compatibility
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for CashfreePaymentRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract payment_session_id from connector_order_id (set by CreateOrder response)
        let payment_session_id = item.resource_common_data.connector_order_id.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: Default::default(),
            },
        )?;

        // Get Cashfree payment method data directly
        let payment_method = get_cashfree_payment_method_data(&item.request.payment_method_data)?;

        Ok(Self {
            payment_session_id,
            payment_method,
            payment_surcharge: None, // TODO: Add surcharge logic if needed
        })
    }
}

// ============================================================================
// Response Transformations
// ============================================================================

impl TryFrom<ResponseRouterData<CashfreeOrderCreateResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CashfreeOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let order_response = PaymentCreateOrderResponse {
            merchant_order_id: item.router_data.request.merchant_order_id.clone(),
            connector_order_id: item.response.payment_session_id.clone(),
            session_data: None,
        };

        Ok(Self {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::Pending,
                connector_order_id: Some(item.response.payment_session_id.clone()),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<CashfreePaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CashfreePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let (status, redirection_data) = match response.channel.as_str() {
            "link" => {
                // Intent flow - extract deep link from payload._default
                let deep_link = response
                    .data
                    .payload
                    .map(|p| p.default_link)
                    .ok_or_else(|| {
                        Report::new(ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some(
                                "link channel: missing payload.default_link (UPI intent)"
                                    .to_string(),
                            ),
                        ))
                    })?;

                // Trim deep link at "?" as per Haskell: truncateIntentLink "?" link
                let trimmed_link = if let Some(pos) = deep_link.find('?') {
                    &deep_link[(pos + 1)..]
                } else {
                    &deep_link
                };

                // Create UPI intent redirection
                let redirection_data = Some(Box::new(Some(
                    domain_types::router_response_types::RedirectForm::Uri {
                        uri: trimmed_link.to_string(),
                    },
                )));

                (
                    common_enums::AttemptStatus::AuthenticationPending,
                    redirection_data,
                )
            }
            "collect" => {
                // Collect flow - return without redirection, status Pending
                (common_enums::AttemptStatus::Pending, None)
            }
            _ => (common_enums::AttemptStatus::Failure, None),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    response
                        .cf_payment_id
                        .as_ref()
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                ),
                redirection_data: redirection_data.and_then(|data| *data).map(Box::new),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response.cf_payment_id.map(|id| id.to_string()),
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
