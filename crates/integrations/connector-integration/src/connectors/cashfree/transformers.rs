use common_enums;
use domain_types::{
    connector_flow::{Authorize, Capture, CreateOrder, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentVoidData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upi: Option<CashfreeUpiDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<CashfreeAppDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netbanking: Option<CashfreeNBDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<()>, // CashFreeCARDType - not yet implemented
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emi: Option<()>, // CashfreeEmiType - not yet implemented
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paypal: Option<()>, // CashfreePaypalType - not yet implemented
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paylater: Option<()>, // CashFreePaylaterType - not yet implemented
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardless_emi: Option<()>, // CashFreeCardlessEmiType - not yet implemented
}

/// CashFreeAPPType — wallet/app payment method (channel: "link", provider, phone)
#[derive(Debug, Serialize)]
pub struct CashfreeAppDetails {
    pub channel: String,   // "link"
    pub provider: String,  // e.g. "phonepe", "paytm", "amazon", "gpay"
    pub phone: Secret<String>, // customer phone number
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

/// CashFreeNBType — net banking payment method (channel: "link", netbanking_bank_code: Int)
/// Matches Haskell CashFreeNBType from Types.hs:2411
#[derive(Debug, Serialize)]
pub struct CashfreeNBDetails {
    pub channel: String,              // Always "link"
    pub netbanking_bank_code: i32,    // Cashfree-specific integer bank code
}

/// Maps the bank_code from NetbankingData to Cashfree's integer bank code.
/// Cashfree uses numeric bank codes for netbanking. This mapping is based
/// on the Cashfree API documentation and the Haskell tech spec.
fn map_to_cashfree_bank_code(bank_code: &str) -> Result<i32, ConnectorError> {
    match bank_code {
        // Major Indian banks
        "SBIN" | "SBI" => Ok(3044),           // State Bank of India
        "HDFC" => Ok(3021),                     // HDFC Bank
        "ICIC" | "ICICI" => Ok(3022),          // ICICI Bank
        "UTIB" | "AXIS" => Ok(3003),           // Axis Bank
        "KKBK" | "KOTAK" => Ok(3032),          // Kotak Mahindra Bank
        "PUNB" | "PNB" => Ok(3035),            // Punjab National Bank
        "BARB" | "BOB" => Ok(3005),            // Bank of Baroda
        "YESB" | "YES" => Ok(3053),            // Yes Bank
        "UBIN" | "UNION" => Ok(3055),          // Union Bank of India
        "INDB" | "INDUSIND" => Ok(3024),       // IndusInd Bank
        "FDRL" | "FEDERAL" => Ok(3016),        // Federal Bank
        "IBKL" | "IDBI" => Ok(3023),           // IDBI Bank
        "CNRB" | "CANARA" => Ok(3009),         // Canara Bank
        "CBIN" | "CENTRAL" => Ok(3010),        // Central Bank of India
        "BKID" | "BOI" => Ok(3006),            // Bank of India
        "IOBA" | "IOB" => Ok(3025),            // Indian Overseas Bank
        "MAHB" | "BOMAh" => Ok(3007),          // Bank of Maharashtra
        "SYNB" | "SYNDICATE" => Ok(3049),      // Syndicate Bank
        "ALLA" | "ALLAHABAD" => Ok(3002),      // Allahabad Bank
        "UCBA" | "UCO" => Ok(3054),            // UCO Bank
        "KARB" | "KARNATAKA" => Ok(3029),      // Karnataka Bank
        "RATN" | "RBL" => Ok(3037),            // RBL Bank
        "SCBL" | "SCB" => Ok(3043),            // Standard Chartered Bank
        "CITI" | "CITIBANK" => Ok(3011),       // Citibank
        "DCBL" | "DCB" => Ok(3014),            // DCB Bank
        "DLXB" | "DLB" => Ok(3015),            // Dhanlaxmi Bank
        "TMBL" | "TMB" => Ok(3050),            // Tamilnad Mercantile Bank
        "JAKA" | "JK" => Ok(3028),             // Jammu & Kashmir Bank
        "KVBL" | "KVB" => Ok(3031),            // Karur Vysya Bank
        "SIBL" | "SIB" => Ok(3046),            // South Indian Bank
        "IDFB" | "IDFC" => Ok(3056),           // IDFC FIRST Bank
        // Pass through numeric codes directly
        _ => bank_code.parse::<i32>().map_err(|_| ConnectorError::NotSupported {
            message: format!("Bank code '{}' is not supported for Cashfree netbanking", bank_code),
            connector: "Cashfree",
        }),
    }
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
    phone: Option<Secret<String>>,
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
        PaymentMethodData::Wallet(wallet_data) => {
            // Map wallet variants to Cashfree APP type (channel: "link", provider: <name>)
            let provider = match wallet_data {
                WalletData::AmazonPayRedirect(_) => "amazon",
                WalletData::PaypalRedirect(_) => "paypal",
                WalletData::GooglePayRedirect(_)
                | WalletData::GooglePayThirdPartySdk(_) => "gpay",
                WalletData::PhonePeRedirect(_) => "phonepe",
                WalletData::LazyPayRedirect(_) => "lazypay",
                WalletData::BillDeskRedirect(_) => "billdesk",
                WalletData::CashfreeRedirect(_) => "cashfreepay",
                WalletData::PayURedirect(_) => "payu",
                WalletData::EaseBuzzRedirect(_) => "easebuzz",
                _ => {
                    return Err(IntegrationError::not_implemented(
                        "This wallet type is not supported for Cashfree".to_string(),
                    ))
                }
            };
            let customer_phone = phone.unwrap_or_else(|| Secret::new("".to_string()));
            Ok(CashfreePaymentMethod {
                upi: None,
                app: Some(CashfreeAppDetails {
                    channel: "link".to_string(),
                    provider: provider.to_string(),
                    phone: customer_phone,
                }),
                netbanking: None,
                card: None,
                emi: None,
                paypal: None,
                paylater: None,
                cardless_emi: None,
            })
        }
        PaymentMethodData::Netbanking(nb_data) => {
            // Map the bank_code to Cashfree's integer bank code
            let cashfree_bank_code = map_to_cashfree_bank_code(&nb_data.bank_code)?;

            Ok(CashfreePaymentMethod {
                upi: None,
                app: None,
                netbanking: Some(CashfreeNBDetails {
                    channel: "link".to_string(),
                    netbanking_bank_code: cashfree_bank_code,
                }),
                card: None,
                emi: None,
                paypal: None,
                paylater: None,
                cardless_emi: None,
            })
        }
        _ => Err(IntegrationError::NotSupported {
            message: "Payment method not supported for Cashfree V3".to_string(),
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
        // Billing address is optional for CreateOrder - may not be available
        // when called from the separate CreateOrder gRPC endpoint
        let billing = item
            .resource_common_data
            .address
            .get_payment_method_billing();

        // Build customer details with optional billing data
        let customer_details = CashfreeCustomerDetails {
            customer_id: item
                .resource_common_data
                .customer_id
                .as_ref()
                .map(|id| id.get_string_repr().to_string())
                .unwrap_or_else(|| "guest".to_string()),
            customer_email: billing
                .as_ref()
                .and_then(|b| b.email.as_ref().map(|e| e.peek().to_string())),
            customer_phone: Secret::new(
                billing
                    .as_ref()
                    .and_then(|b| b.phone.as_ref())
                    .and_then(|phone| phone.number.as_ref())
                    .map(|number| number.peek().to_string())
                    .unwrap_or_else(|| "9999999999".to_string()),
            ),
            customer_name: billing
                .as_ref()
                .and_then(|b| b.get_optional_full_name().map(|name| name.expose())),
        };

        // return_url defaults to webhook_url if not available (CreateOrder gRPC
        // request doesn't carry return_url)
        let return_url = item
            .resource_common_data
            .return_url
            .clone()
            .or_else(|| item.request.webhook_url.clone())
            .unwrap_or_else(|| "https://example.com/return".to_string());

        // Get webhook URL from request - required for Cashfree V3
        let notify_url = item
            .request
            .webhook_url
            .clone()
            .unwrap_or_else(|| "https://example.com/webhook".to_string());

        let order_meta = CashfreeOrderMeta {
            return_url,
            notify_url,
            payment_methods: None, // Allow all payment methods; Cashfree filters based on the payment_method sent in the Authorize request
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
        // Extract payment_session_id: prefer session_token (set by CreateOrder
        // response passed via gRPC), then reference_id, then merchant_order_id
        let payment_session_id = item
            .resource_common_data
            .session_token
            .clone()
            .or_else(|| item.resource_common_data.reference_id.clone())
            .or_else(|| item.request.merchant_order_id.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "payment_session_id (pass session_token from CreateOrder response)",
                context: Default::default(),
            })?;

        // Extract customer phone from billing address (needed for wallet APP type)
        let phone = item
            .resource_common_data
            .address
            .get_payment_method_billing()
            .and_then(|billing| billing.phone.as_ref())
            .and_then(|phone| phone.number.as_ref())
            .map(|number| Secret::new(number.peek().to_string()));

        // Get Cashfree payment method data
        let payment_method =
            get_cashfree_payment_method_data(&item.request.payment_method_data, phone)?;

        Ok(Self {
            payment_session_id,
            payment_method,
            payment_surcharge: None,
        })
    }
}

// ============================================================================
// Response Transformations
// ============================================================================

impl TryFrom<CashfreeOrderCreateResponse> for PaymentCreateOrderResponse {
    type Error = Report<ConnectorError>;

    fn try_from(response: CashfreeOrderCreateResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            connector_order_id: response.payment_session_id,
            session_data: None,
        })
    }
}

// Add the missing TryFrom implementation for macro compatibility
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
        let response = item.response;
        let order_response = PaymentCreateOrderResponse::try_from(response)?;

        // Extract order_id before moving order_response
        let order_id = order_response.connector_order_id.clone();

        Ok(Self {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                // Update status to indicate successful order creation
                status: common_enums::AttemptStatus::Pending,
                // Set connector_order_id to the payment_session_id for use in authorize flow
                reference_id: Some(order_id.clone()),
                connector_order_id: Some(order_id),
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
                // Check payment_method to determine how to extract the redirect URL
                if response.payment_method.as_str() == "app"
                    || response.payment_method.as_str() == "netbanking"
                {
                    // Wallet/APP and Netbanking flows — redirect URL is in data.url
                    let redirect_url =
                        response.data.url.clone().ok_or(IntegrationError::MissingRequiredField {
                            field_name: "redirect_url",
                            context: Default::default(),
                        })?;
                    let redirection_data = Some(Box::new(Some(
                        domain_types::router_response_types::RedirectForm::Uri {
                            uri: redirect_url,
                        },
                    )));
                    (
                        common_enums::AttemptStatus::AuthenticationPending,
                        redirection_data,
                    )
                } else {
                    // UPI Intent/QR flow - extract deep link from payload.default
                    let deep_link = response.data.payload.map(|p| p.default_link).ok_or(
                        ConnectorError::MissingRequiredField {
                            field_name: "intent_link",
                        },
                    )?;

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
            }
            "collect" => {
                // Collect flow - return without redirection, status Pending
                (common_enums::AttemptStatus::Pending, None)
            }
            _ => (common_enums::AttemptStatus::Failure, None),
        };

        // For Cashfree, use the merchant order_id as connector_transaction_id
        // because all subsequent API calls (PSync, Capture, Void, Refund) use
        // the order_id in their URLs, not cf_payment_id.
        let order_id = item
            .router_data
            .resource_common_data
            .reference_id
            .clone()
            .or_else(|| {
                Some(
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                )
            })
            .unwrap_or_default();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(order_id.clone()),
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

// ============================================================================
// Payment Sync (PSync) — V3 GET /pg/orders/:orderid/payments
// ============================================================================

/// Unit struct for GET-based PSync (no request body)
#[derive(Debug, Serialize)]
pub struct CashfreeSyncRequest;

/// V2/V3 Payment Status Response — matches CashfreePaymentStatusSucResponse
#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreePaymentStatusItem {
    pub cf_payment_id: Option<serde_json::Value>,
    pub order_id: String,
    pub payment_status: String,
    pub payment_amount: Option<serde_json::Value>,
    pub payment_message: Option<String>,
    pub bank_reference: Option<String>,
    pub error_details: Option<CashfreeSyncErrorDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeSyncErrorDetails {
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

/// The API returns an array of payment objects
pub type CashfreeSyncResponse = Vec<CashfreePaymentStatusItem>;

/// Cashfree V2/V3 payment_status values
#[derive(Debug, Clone)]
pub enum CashfreePaymentStatus {
    Success,
    Pending,
    NotAttempted,
    Failed,
    Cancelled,
    UserDropped,
    Unknown(String),
}

impl From<&str> for CashfreePaymentStatus {
    fn from(s: &str) -> Self {
        match s {
            "SUCCESS" => Self::Success,
            "PENDING" => Self::Pending,
            "NOT_ATTEMPTED" => Self::NotAttempted,
            "FAILED" => Self::Failed,
            "CANCELLED" => Self::Cancelled,
            "USER_DROPPED" => Self::UserDropped,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl From<CashfreePaymentStatus> for common_enums::AttemptStatus {
    fn from(status: CashfreePaymentStatus) -> Self {
        match status {
            CashfreePaymentStatus::Success => Self::Charged,
            CashfreePaymentStatus::Pending | CashfreePaymentStatus::NotAttempted => Self::Pending,
            CashfreePaymentStatus::Failed
            | CashfreePaymentStatus::Cancelled
            | CashfreePaymentStatus::UserDropped => Self::Failure,
            CashfreePaymentStatus::Unknown(_) => Self::Pending,
        }
    }
}

// ============================================================================
// Capture (Pre-Auth Capture) — V3 POST /pg/orders/:orderid/authorization
// ============================================================================

/// Cashfree V3 capture/void request body — `CashfreeCaptureOrVoidRequestV3`
#[derive(Debug, Serialize)]
pub struct CashfreeCaptureRequest {
    pub action: String, // "CAPTURE"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>, // optional partial capture amount
}

/// Cashfree V3 pre-auth response — `CashfreeAuthorizeResponse` (AuthorizationInPayments)
#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeCaptureResponse {
    pub payment_status: Option<String>,
    pub cf_payment_id: Option<serde_json::Value>,
    pub order_id: Option<String>,
    pub action: Option<String>,
    pub status: Option<String>,
    pub captured_amount: Option<f64>,
}

/// Status mapping for capture flow per techspec section 9.4
fn map_capture_payment_status(payment_status: &str) -> common_enums::AttemptStatus {
    match payment_status {
        "SUCCESS" => common_enums::AttemptStatus::Charged,
        "FAILED" => common_enums::AttemptStatus::CaptureFailed,
        "PENDING" => common_enums::AttemptStatus::CaptureInitiated,
        // Pre-auth authorization status variants that indicate capture succeeded
        "CAPTURE" => common_enums::AttemptStatus::Charged,
        _ => common_enums::AttemptStatus::Pending,
    }
}

// TryFrom for macro-generated CashfreeRouterData wrapper → CashfreeCaptureRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for CashfreeCaptureRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        wrapper: crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        // Convert minor unit (paise) to major unit (rupees) for Cashfree API
        let amount_i64 = router_data.request.amount_to_capture;
        #[allow(clippy::as_conversions)]
        let amount_f64 = amount_i64 as f64 / 100.0;

        Ok(Self {
            action: "CAPTURE".to_string(),
            amount: Some(amount_f64),
        })
    }
}

// TryFrom for CashfreeCaptureResponse → RouterDataV2 (response parsing)
impl
    TryFrom<
        ResponseRouterData<
            CashfreeCaptureResponse,
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        >,
    >
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            CashfreeCaptureResponse,
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;

        // Map payment_status or authorization status to attempt status
        let status = response
            .payment_status
            .as_deref()
            .or(response.status.as_deref())
            .map(map_capture_payment_status)
            .unwrap_or(common_enums::AttemptStatus::Pending);

        // Use order_id as connector_transaction_id for consistency with Cashfree APIs
        let order_id = response
            .order_id
            .as_deref()
            .unwrap_or(&router_data.request.get_connector_transaction_id().unwrap_or_default())
            .to_string();

        let cf_payment_id = response
            .cf_payment_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_default();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(order_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(cf_payment_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

/// TryFrom for CashfreeSyncRequest (GET — empty body, URL built from connector_transaction_id)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for CashfreeSyncRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        _wrapper: crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

// ============================================================================
// Void (Pre-Auth Void) — V3 POST /pg/orders/:orderid/authorization
// ============================================================================

/// Cashfree V3 void request body — action: "VOID"
#[derive(Debug, Serialize)]
pub struct CashfreeVoidRequest {
    pub action: String, // "VOID"
}

/// Cashfree V3 void response — same shape as capture response
#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeVoidResponse {
    pub payment_status: Option<String>,
    pub cf_payment_id: Option<serde_json::Value>,
    pub order_id: Option<String>,
    pub action: Option<String>,
    pub status: Option<String>,
}

/// Status mapping for void flow per techspec section 9.5
fn map_void_payment_status(payment_status: &str) -> common_enums::AttemptStatus {
    match payment_status {
        "VOID" => common_enums::AttemptStatus::Voided,
        "FAILED" => common_enums::AttemptStatus::VoidFailed,
        "PENDING" => common_enums::AttemptStatus::Pending,
        _ => common_enums::AttemptStatus::Pending,
    }
}

// TryFrom for macro-generated CashfreeRouterData wrapper → CashfreeVoidRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for CashfreeVoidRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        _wrapper: crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            action: "VOID".to_string(),
        })
    }
}

// TryFrom for CashfreeVoidResponse → RouterDataV2 (response parsing)
impl
    TryFrom<
        ResponseRouterData<
            CashfreeVoidResponse,
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        >,
    >
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            CashfreeVoidResponse,
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;

        // Map payment_status or action status to attempt status
        let status = response
            .payment_status
            .as_deref()
            .or(response.status.as_deref())
            .or(response.action.as_deref())
            .map(map_void_payment_status)
            .unwrap_or(common_enums::AttemptStatus::Pending);

        // Use order_id as connector_transaction_id for consistency with Cashfree APIs
        let order_id = response
            .order_id
            .as_deref()
            .unwrap_or(&router_data.request.connector_transaction_id)
            .to_string();

        let cf_payment_id = response
            .cf_payment_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_default();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(order_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(cf_payment_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

/// TryFrom for PSync response — picks the first SUCCESS item, or falls back to first item
impl
    TryFrom<
        ResponseRouterData<
            CashfreeSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    >
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            CashfreeSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let payments = item.response;
        let router_data = item.router_data;

        // Pick the best payment record: SUCCESS first, then PENDING, then any
        let payment = payments
            .iter()
            .find(|p| p.payment_status == "SUCCESS")
            .or_else(|| payments.iter().find(|p| p.payment_status == "PENDING"))
            .or_else(|| payments.first())
            .ok_or(ConnectorError::ResponseDeserializationFailed)?;

        let attempt_status =
            common_enums::AttemptStatus::from(CashfreePaymentStatus::from(
                payment.payment_status.as_str(),
            ));

        // Use order_id as connector_transaction_id for consistency with Cashfree APIs
        let order_id = payment.order_id.clone();

        let cf_payment_id = payment
            .cf_payment_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_default();

        // Check for error details on failure
        if matches!(
            attempt_status,
            common_enums::AttemptStatus::Failure
        ) {
            if let Some(error) = &payment.error_details {
                return Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..router_data.resource_common_data
                    },
                    response: Err(domain_types::router_data::ErrorResponse {
                        status_code: item.http_code,
                        code: error.error_code.clone().unwrap_or_default(),
                        message: error
                            .error_description
                            .clone()
                            .unwrap_or_else(|| payment.payment_message.clone().unwrap_or_default()),
                        reason: error.error_description.clone(),
                        attempt_status: Some(attempt_status),
                        connector_transaction_id: Some(order_id),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    ..router_data
                });
            }
        }

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(order_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(cf_payment_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// ============================================================================
// Refund — V3 POST /pg/orders/:orderid/refunds
// ============================================================================

/// Cashfree V3 refund create request body — `CashfreeV2RefundReq`
#[derive(Debug, Serialize)]
pub struct CashfreeRefundRequest {
    pub refund_id: String,
    pub refund_amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_note: Option<String>,
}

/// Cashfree V3 refund create/sync response — `CashfreeV2ValidRefundResponse`
#[derive(Debug, Serialize, Deserialize)]
pub struct CashfreeRefundResponse {
    pub cf_refund_id: String,
    pub refund_id: String,
    pub refund_status: String,
    pub refund_amount: Option<f64>,
    pub failure_reason: Option<String>,
    pub order_id: Option<String>,
    pub refund_note: Option<String>,
}

/// Cashfree refund status → internal RefundStatus
fn map_refund_status(status: &str) -> common_enums::RefundStatus {
    match status {
        "SUCCESS" | "OK" => common_enums::RefundStatus::Success,
        "PENDING" => common_enums::RefundStatus::Pending,
        "CANCELLED" | "FAILED" => common_enums::RefundStatus::Failure,
        _ => common_enums::RefundStatus::Pending,
    }
}

// TryFrom for macro-generated CashfreeRouterData → CashfreeRefundRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for CashfreeRefundRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        wrapper: crate::connectors::cashfree::CashfreeRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        // Convert minor unit (paise) to major unit (rupees) as string for Cashfree V3 API
        let amount_i64 = router_data.request.minor_refund_amount.get_amount_as_i64();
        #[allow(clippy::as_conversions)]
        let amount_f64 = amount_i64 as f64 / 100.0;
        let refund_amount = format!("{amount_f64:.2}");

        Ok(Self {
            refund_id: router_data.request.refund_id.clone(),
            refund_amount,
            refund_note: router_data.request.reason.clone(),
        })
    }
}

// TryFrom for CashfreeRefundResponse → RouterDataV2 (response parsing)
impl
    TryFrom<
        ResponseRouterData<
            CashfreeRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    >
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            CashfreeRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;

        let refund_status = map_refund_status(&response.refund_status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                // Use merchant refund_id (not cf_refund_id) as connector_refund_id
                // because the RSync URL uses this value: pg/orders/{order_id}/refunds/{refund_id}
                // and Cashfree expects the merchant-provided refund_id in the URL
                connector_refund_id: response.refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ============================================================================
// RSync — V3 GET /pg/orders/:orderid/refunds/:refundid
// ============================================================================

/// Cashfree V3 refund sync response — same schema as CashfreeRefundResponse
pub type CashfreeRefundSyncResponse = CashfreeRefundResponse;

// TryFrom for CashfreeRefundSyncResponse → RouterDataV2 (RSync response parsing)
impl
    TryFrom<
        ResponseRouterData<
            CashfreeRefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    >
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            CashfreeRefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;

        let refund_status = map_refund_status(&response.refund_status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                // Use merchant refund_id (not cf_refund_id) as connector_refund_id
                // because the RSync URL uses this value: pg/orders/{order_id}/refunds/{refund_id}
                // and Cashfree expects the merchant-provided refund_id in the URL
                connector_refund_id: response.refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}
