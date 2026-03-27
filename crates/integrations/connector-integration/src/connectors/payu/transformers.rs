use common_enums::{self, AttemptStatus, Currency, RefundStatus};
use common_utils::{pii::IpAddress, Email, Method};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentVoidData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::ConnectorError,
    payment_method_data::{NetbankingData, PaymentMethodData, PaymentMethodDataTypes, UpiData, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::AuthoriseIntegrityObject,
    router_response_types::RedirectForm,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

pub mod constants {
    // Payu API versions
    pub const API_VERSION: &str = "2.0";

    // Payu device info
    pub const DEVICE_INFO: &str = "web";

    // Payu UPI specific constants
    pub const PRODUCT_INFO: &str = "Payment"; // Default product info
    pub const UPI_PG: &str = "UPI"; // UPI payment gateway
    pub const UPI_COLLECT_BANKCODE: &str = "UPI"; // UPI Collect bank code
    pub const UPI_INTENT_BANKCODE: &str = "INTENT"; // UPI Intent bank code
    pub const UPI_S2S_FLOW: &str = "2"; // S2S flow type for UPI

    // Payu PSync specific constants
    pub const COMMAND: &str = "verify_payment";
}

// PayU Status enum to handle both integer and string status values
#[derive(Debug, Serialize, Clone)]
pub enum PayuStatusValue {
    IntStatus(i32),       // 1 for UPI Intent success
    StringStatus(String), // "success" for UPI Collect success
}

// Custom deserializer for PayU status field that can be either int or string
fn deserialize_payu_status<'de, D>(deserializer: D) -> Result<Option<PayuStatusValue>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde_json::Value;
    let value: Option<Value> = Option::deserialize(deserializer)?;

    match value {
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i32::try_from(i)
                    .ok()
                    .map(PayuStatusValue::IntStatus)
                    .map(Some)
                    .ok_or_else(|| serde::de::Error::custom("status value out of range for i32"))
            } else {
                Ok(None)
            }
        }
        Some(Value::String(s)) => Ok(Some(PayuStatusValue::StringStatus(s))),
        _ => Ok(None),
    }
}

// Authentication structure based on Payu analysis
#[derive(Debug, Clone)]
pub struct PayuAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>, // Merchant salt for signature
}

impl TryFrom<&ConnectorSpecificConfig> for PayuAuthType {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Payu {
                api_key,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

// Note: Integrity Framework implementation will be handled by the framework itself
// since we can't implement foreign traits for foreign types (orphan rules)

// Request structure based on Payu UPI analysis
#[derive(Debug, Serialize)]
pub struct PayuPaymentRequest {
    // Core payment fields
    pub key: String,                                  // Merchant key
    pub txnid: String,                                // Transaction ID
    pub amount: common_utils::types::StringMajorUnit, // Amount in string major units
    pub currency: Currency,                           // Currency code
    pub productinfo: String,                          // Product description

    // Customer information
    pub firstname: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastname: Option<Secret<String>>,
    pub email: Email,
    pub phone: Secret<String>,

    // URLs
    pub surl: String, // Success URL
    pub furl: String, // Failure URL

    // Payment method specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pg: Option<String>, // Payment gateway code (UPI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bankcode: Option<String>, // Bank code (TEZ, INTENT, TEZOMNI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpa: Option<Secret<String>>, // UPI VPA (for collect)

    // UPI specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_s2s_flow: Option<String>, // S2S flow type ("2" for UPI); None for redirect flows
    pub s2s_client_ip: Secret<String, IpAddress>, // Client IP
    pub s2s_device_info: String, // Device info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>, // API version ("2.0")

    // Security
    pub hash: String, // SHA-512 signature

    // User defined fields (10 fields as per PayU spec)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf3: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf4: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf6: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf7: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf8: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf9: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf10: Option<String>,

    // Optional PayU fields for UPI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_key: Option<String>, // Offer identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si: Option<i32>, // Standing instruction flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_details: Option<String>, // SI details JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beneficiarydetail: Option<String>, // TPV beneficiary details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_token: Option<String>, // User token for repeat transactions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_auto_apply: Option<i32>, // Auto apply offer flag (0 or 1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_charges: Option<String>, // Surcharge/fee amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_gst_charges: Option<String>, // GST charges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upi_app_name: Option<String>, // UPI app name for intent flows
}

// Response structure based on actual PayU API response
#[derive(Debug, Deserialize, Serialize)]
pub struct PayuPaymentResponse {
    // Success response fields - PayU can return status as either int or string
    #[serde(deserialize_with = "deserialize_payu_status")]
    pub status: Option<PayuStatusValue>, // Status can be 1 (int) or "success" (string)
    pub token: Option<String>, // PayU token
    #[serde(alias = "referenceId")]
    pub reference_id: Option<String>, // PayU reference ID
    #[serde(alias = "returnUrl")]
    pub return_url: Option<String>, // Return URL
    #[serde(alias = "merchantName")]
    pub merchant_name: Option<String>, // Merchant display name
    #[serde(alias = "merchantVpa")]
    pub merchant_vpa: Option<Secret<String>>, // Merchant UPI VPA
    pub amount: Option<String>, // Transaction amount
    #[serde(alias = "txnId", alias = "txnid")]
    pub txn_id: Option<String>, // Transaction ID
    #[serde(alias = "intentURIData")]
    pub intent_uri_data: Option<String>, // UPI intent URI data

    // UPI-specific fields
    pub apps: Option<Vec<PayuUpiApp>>, // Available UPI apps
    #[serde(alias = "upiPushDisabled")]
    pub upi_push_disabled: Option<String>, // UPI push disabled flag
    #[serde(alias = "pushServiceUrl")]
    pub push_service_url: Option<String>, // Push service URL
    #[serde(alias = "pushServiceUrlV2")]
    pub push_service_url_v2: Option<String>, // Push service URL V2
    #[serde(alias = "encodedPayuId")]
    pub encoded_payu_id: Option<String>, // Encoded PayU ID
    #[serde(alias = "vpaRegex")]
    pub vpa_regex: Option<String>, // VPA validation regex

    // Polling and timeout configuration
    #[serde(alias = "upiServicePollInterval")]
    pub upi_service_poll_interval: Option<String>, // Poll interval
    #[serde(alias = "sdkUpiPushExpiry")]
    pub sdk_upi_push_expiry: Option<String>, // Push expiry time
    #[serde(alias = "sdkUpiVerificationInterval")]
    pub sdk_upi_verification_interval: Option<String>, // Verification interval

    // Additional flags
    #[serde(alias = "disableIntentSeamlessFailure")]
    pub disable_intent_seamless_failure: Option<String>,
    #[serde(alias = "intentSdkCombineVerifyAndPayButton")]
    pub intent_sdk_combine_verify_and_pay_button: Option<String>,

    // Error response fields (actual PayU format)
    pub result: Option<PayuResult>, // PayU result field (null for errors)
    pub error: Option<String>,      // Error code like "EX158"
    pub message: Option<String>,    // Error message

    // Redirect form data extracted from HTML response (wallet/netbanking redirect flows)
    pub redirect_url: Option<String>,          // Form action URL extracted from HTML
    pub redirect_form_fields: Option<std::collections::HashMap<String, String>>, // Hidden input fields
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayuResult {
    // Common fields present in both UPI-collect and wallet/netbanking redirect responses
    pub status: String,             // e.g. "pending", "failure", "success"
    pub mihpayid: Option<String>,   // PayU payment ID (may be absent on hard failures)
    // Fields present in redirect wallet/netbanking responses
    #[serde(rename = "error_Message")]
    pub error_message: Option<String>, // Human-readable error description
    pub error: Option<String>,         // PayU error code, e.g. "E312"
    pub mode: Option<String>,          // Payment mode, e.g. "CASH"
    pub payment_source: Option<String>, // e.g. "payuPureS2S"
    #[serde(rename = "PG_TYPE")]
    pub pg_type: Option<String>,       // e.g. "CASH-PG"
    pub bankcode: Option<String>,      // e.g. "PHONEPE"
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayuUpiApp {
    pub name: String,    // App display name
    pub package: String, // Android package name
}

// Error response structure matching actual PayU format
#[derive(Debug, Deserialize, Serialize)]
pub struct PayuErrorResponse {
    pub result: Option<serde_json::Value>, // null for errors
    pub status: Option<String>,            // "failed" for errors
    pub error: Option<String>,             // Error code like "EX158", "EX311"
    pub message: Option<String>,           // Error description

    // Legacy fields for backward compatibility
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}

// Request conversion with Framework Integration
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PayuRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayuPaymentRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: super::PayuRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract router data
        let router_data = &item.router_data;

        // Use AmountConvertor framework for proper amount handling
        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(ConnectorError::AmountConversionFailed)?;

        // Extract authentication
        let auth = PayuAuthType::try_from(&router_data.connector_config)?;

        // Determine payment flow based on payment method
        let (pg, bankcode, vpa, s2s_flow) = determine_upi_flow(&router_data.request)?;

        // Generate UDF fields based on Haskell implementation
        let udf_fields = generate_udf_fields(
            &router_data.resource_common_data.connector_request_reference_id,
            router_data
                .resource_common_data
                .merchant_id
                .get_string_repr(),
            router_data,
        );

        // Build base request
        let mut request = Self {
            key: auth.api_key.peek().to_string(),
            txnid: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            currency: router_data.request.currency,
            productinfo: constants::PRODUCT_INFO.to_string(), // Default product info

            // Customer info - extract from billing address if available
            firstname: router_data.resource_common_data.get_billing_first_name()?,
            lastname: router_data
                .resource_common_data
                .get_optional_billing_last_name(),
            email: router_data.resource_common_data.get_billing_email()?,
            phone: router_data
                .resource_common_data
                .get_billing_phone_number()?,

            // URLs - use router return URL if available
            surl: router_data.request.get_router_return_url()?,
            furl: router_data.request.get_router_return_url()?,

            // Payment method specific
            pg,
            bankcode,
            vpa: vpa.map(Secret::new),

            // UPI specific - corrected based on PayU docs
            txn_s2s_flow: s2s_flow,
            s2s_client_ip: router_data
                .request
                .get_ip_address_as_optional()
                .ok_or_else(|| {
                    report!(ConnectorError::MissingRequiredField {
                        field_name: "IP address"
                    })
                })?,
            s2s_device_info: constants::DEVICE_INFO.to_string(),
            api_version: Some(constants::API_VERSION.to_string()), // As per PayU analysis

            // Will be calculated after struct creation
            hash: String::new(),

            // User defined fields based on Haskell implementation logic
            udf1: udf_fields.first().and_then(|f| f.clone()), // Transaction ID or metadata value
            udf2: udf_fields.get(1).and_then(|f| f.clone()),  // Merchant ID or metadata value
            udf3: udf_fields.get(2).and_then(|f| f.clone()),  // From metadata or order reference
            udf4: udf_fields.get(3).and_then(|f| f.clone()),  // From metadata or order reference
            udf5: udf_fields.get(4).and_then(|f| f.clone()),  // From metadata or order reference
            udf6: udf_fields.get(5).and_then(|f| f.clone()),  // From order reference (udf6)
            udf7: udf_fields.get(6).and_then(|f| f.clone()),  // From order reference (udf7)
            udf8: udf_fields.get(7).and_then(|f| f.clone()),  // From order reference (udf8)
            udf9: udf_fields.get(8).and_then(|f| f.clone()),  // From order reference (udf9)
            udf10: udf_fields.get(9).and_then(|f| f.clone()), // Always empty string

            // Optional PayU fields for UPI
            offer_key: None,
            si: None, // Not implementing mandate flows initially
            si_details: None,
            beneficiarydetail: None, // Not implementing TPV initially
            user_token: None,
            offer_auto_apply: None,
            additional_charges: None,
            additional_gst_charges: None,
            upi_app_name: determine_upi_app_name(&router_data.request)?,
        };

        // Generate hash signature
        request.hash = generate_payu_hash(&request, &auth.api_secret)?;

        Ok(request)
    }
}

// PayU Sync/Verify Payment Request structure
#[derive(Debug, Serialize)]
pub struct PayuSyncRequest {
    pub key: String,     // Merchant key
    pub command: String, // "verify_payment"
    pub var1: String,    // Transaction ID to verify
    pub hash: String,    // SHA-512 signature
}

// PayU Sync Response structure based on Haskell implementation
#[derive(Debug, Deserialize, Serialize)]
pub struct PayuSyncResponse {
    pub status: Option<i32>, // 0 = error, non-zero = success
    pub msg: Option<String>, // Status message
    pub transaction_details: Option<std::collections::HashMap<String, PayuTransactionDetail>>, // Map of txnId -> details
    pub result: Option<serde_json::Value>, // Optional result field
    #[serde(alias = "field3")]
    pub field3: Option<String>, // Additional field
}

// PayU Transaction Detail structure from sync response
#[derive(Debug, Deserialize, Serialize)]
pub struct PayuTransactionDetail {
    pub mihpayid: Option<String>,         // PayU transaction ID
    pub txnid: Option<String>,            // Merchant transaction ID
    pub amount: Option<String>,           // Transaction amount
    pub status: String, // Transaction status: "success", "failure", "pending", "cancel"
    pub firstname: Option<String>, // Customer first name
    pub lastname: Option<String>, // Customer last name
    pub email: Option<Secret<String>>, // Customer email
    pub phone: Option<Secret<String>>, // Customer phone
    pub productinfo: Option<String>, // Product description
    pub hash: Option<String>, // Response hash for verification
    pub field1: Option<String>, // UPI transaction ID
    pub field2: Option<String>, // Bank reference number
    pub field3: Option<String>, // Payment source
    pub field9: Option<String>, // Additional field
    pub error_code: Option<String>, // Error code if failed
    pub error_message: Option<String>, // Error message if failed
    pub card_token: Option<String>, // Card token if applicable
    pub card_category: Option<String>, // Card category
    pub offer_key: Option<String>, // Offer key used
    pub discount: Option<String>, // Discount applied
    pub net_amount_debit: Option<String>, // Net amount debited
    pub addedon: Option<String>, // Transaction timestamp
    pub payment_source: Option<String>, // Payment method used
    pub bank_ref_num: Option<String>, // Bank reference number
    pub upi_va: Option<String>, // UPI virtual address
    pub cardnum: Option<String>, // Masked card number
    pub issuing_bank: Option<String>, // Card issuing bank
}

// PayU Sync Request conversion from RouterData
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PayuRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for PayuSyncRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: super::PayuRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract authentication
        let auth = PayuAuthType::try_from(&router_data.connector_config)?;

        // Extract transaction ID from connector_transaction_id
        let transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(ConnectorError::MissingRequiredField {
                field_name: "connector_transaction_id",
            })?;

        let command = constants::COMMAND;

        // Build sync request
        let mut request = Self {
            key: auth.api_key.peek().to_string(),
            command: command.to_string(),
            var1: transaction_id,
            hash: String::new(), // Will be calculated below
        };

        // Generate hash signature for verification request
        // PayU verify hash: SHA512(key|command|var1|salt)
        request.hash = generate_payu_verify_hash(&request, &auth.api_secret)?;

        Ok(request)
    }
}

// Hash generation for PayU verify payment request
// Based on Haskell: makePayuVerifyHash payuDetails txnId command
fn generate_payu_verify_hash(
    request: &PayuSyncRequest,
    merchant_salt: &Secret<String>,
) -> Result<String, ConnectorError> {
    use sha2::{Digest, Sha512};

    // PayU verify hash format: key|command|var1|salt
    let hash_fields = [
        request.key.clone(),
        request.command.clone(),
        request.var1.clone(),
        merchant_salt.peek().to_string(),
    ];

    // Join with pipe separator
    let hash_string = hash_fields.join("|");

    // Log hash string for debugging (remove in production)
    #[cfg(debug_assertions)]
    {
        if let Some(fields_without_last) = hash_fields.get(..hash_fields.len().saturating_sub(1)) {
            let masked_hash = format!("{}|***MASKED***", fields_without_last.join("|"));
            tracing::debug!("PayU verify hash string (salt masked): {}", masked_hash);
            tracing::debug!("PayU verify expected format: key|command|var1|salt");
        }
    }

    // Generate SHA-512 hash
    let mut hasher = Sha512::new();
    hasher.update(hash_string.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// UDF field generation based on Haskell implementation
// Implements the logic from getUdf1-getUdf5 functions and orderReference fields
fn generate_udf_fields<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_id: &str,
    merchant_id: &str,
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> [Option<String>; 10] {
    // Based on Haskell implementation:
    // udf1-udf5 come from PayuMetaData (if available) or default values
    // udf6-udf9 come from orderReference fields
    // udf10 is always empty string

    // Extract metadata from request
    let metadata = router_data.request.metadata.as_ref();

    // Helper function to get string value from metadata
    let get_metadata_field = |field: &str| -> Option<String> {
        metadata
            .and_then(|m| m.peek().get(field))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    [
        // udf1: From metadata "udf1" or default to connector_request_reference_id (merchant txn id)
        get_metadata_field("udf1").or(Some(payment_id.to_string())),
        // udf2: From metadata "udf2" or default to merchant ID
        get_metadata_field("udf2").or(Some(merchant_id.to_string())),
        // udf3: From metadata "udf3" or empty
        get_metadata_field("udf3"),
        // udf4: From metadata "udf4" or empty
        get_metadata_field("udf4"),
        // udf5: From metadata "udf5" or empty
        get_metadata_field("udf5"),
        // udf6: From metadata "udf6" or empty
        get_metadata_field("udf6"),
        // udf7: From metadata "udf7" or empty
        get_metadata_field("udf7"),
        // udf8: From metadata "udf8" or empty
        get_metadata_field("udf8"),
        // udf9: From metadata "udf9" or empty
        get_metadata_field("udf9"),
        // udf9: From metadata "udf10" or empty
        get_metadata_field("udf10"),
    ]
}

// UPI app name determination based on Haskell getUpiAppName implementation
fn determine_upi_app_name<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<Option<String>, ConnectorError> {
    // From Haskell getUpiAppName implementation:
    // getUpiAppName txnDetail = case getJuspayBankCodeFromInternalMetadata txnDetail of
    //   Just "JP_PHONEPE"   -> "phonepe"
    //   Just "JP_GOOGLEPAY" -> "googlepay"
    //   Just "JP_BHIM"      -> "bhim"
    //   Just "JP_PAYTM"     -> "paytm"
    //   Just "JP_CRED"      -> "cred"
    //   Just "JP_AMAZONPAY" -> "amazonpay"
    //   Just "JP_WHATSAPP"  -> "whatsapp"
    //   _                   -> "genericintent"

    match &request.payment_method_data {
        PaymentMethodData::Upi(upi_data) => {
            match upi_data {
                UpiData::UpiIntent(_) | UpiData::UpiQr(_) => {
                    // For UPI Intent and UPI QR, return generic intent as fallback
                    // TODO: Extract bank code from metadata if available
                    Ok(None)
                }
                UpiData::UpiCollect(upi_collect_data) => {
                    // UPI Collect doesn't typically use app name
                    Ok(upi_collect_data.vpa_id.clone().map(|vpa| vpa.expose()))
                }
            }
        }
        PaymentMethodData::Wallet(_) | PaymentMethodData::Netbanking(_) => Ok(None),
        _ => Ok(None),
    }
}

// PayU flow determination based on Haskell getTxnS2SType implementation
#[allow(clippy::type_complexity)]
fn determine_upi_flow<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<(Option<String>, Option<String>, Option<String>, Option<String>), ConnectorError> {
    // Based on Haskell implementation:
    // getTxnS2SType :: Bool -> Bool -> Bool -> Bool -> Bool -> Maybe Text
    // getTxnS2SType isTxnS2SFlow4Enabled s2sEnabled isDirectOTPTxn isEmandateRegister isDirectAuthorization

    match &request.payment_method_data {
        PaymentMethodData::Upi(upi_data) => {
            match upi_data {
                UpiData::UpiCollect(collect_data) => {
                    if let Some(vpa) = &collect_data.vpa_id {
                        // UPI Collect flow - based on Haskell implementation
                        // For UPI Collect: pg = UPI, bankcode = UPI, VPA required
                        // The key is that VPA must be populated for sourceObject == "UPI_COLLECT"
                        Ok((
                            Some(constants::UPI_PG.to_string()),
                            Some(constants::UPI_COLLECT_BANKCODE.to_string()),
                            Some(vpa.peek().to_string()),
                            Some(constants::UPI_S2S_FLOW.to_string()), // UPI Collect uses S2S flow "2"
                        ))
                    } else {
                        // Missing VPA for UPI Collect - this should be an error
                        Err(ConnectorError::MissingRequiredField {
                            field_name: "vpa_id",
                        })
                    }
                }
                UpiData::UpiIntent(_) | UpiData::UpiQr(_) => {
                    // UPI Intent flow - uses S2S flow "2" for intent-based transactions
                    // pg=UPI, bankcode=INTENT for intent flows
                    Ok((
                        Some(constants::UPI_PG.to_string()),
                        Some(constants::UPI_INTENT_BANKCODE.to_string()),
                        None,
                        Some(constants::UPI_S2S_FLOW.to_string()),
                    ))
                }
            }
        }
        PaymentMethodData::Wallet(wallet_data) => {
            // Wallet redirect flows use pg=CASH with a wallet-specific bankcode.
            // These are standard redirect payments — PayU returns an HTML redirect page,
            // NOT an S2S JSON response. Do NOT set txn_s2s_flow for redirect wallet payments.
            let (pg, bankcode) = match wallet_data {
                WalletData::PayURedirect(_) => {
                    // PayU generic wallet redirect — use LAZYPAY (BNPL) as the default
                    // bankcode since PAYTM is not enabled in the PayU test environment.
                    // LAZYPAY is available on test merchants and confirmed working.
                    ("BNPL".to_string(), "LAZYPAY".to_string())
                }
                WalletData::PhonePeRedirect(_) => {
                    ("CASH".to_string(), "PHONEPE".to_string())
                }
                WalletData::LazyPayRedirect(_) => {
                    ("BNPL".to_string(), "LAZYPAY".to_string())
                }
                WalletData::BillDeskRedirect(_) => {
                    ("CASH".to_string(), "BILLDESK".to_string())
                }
                WalletData::CashfreeRedirect(_) => {
                    ("CASH".to_string(), "CASHFREE".to_string())
                }
                WalletData::EaseBuzzRedirect(_) => {
                    ("CASH".to_string(), "EASEBUZZ".to_string())
                }
                _ => {
                    return Err(ConnectorError::NotSupported {
                        message: "Wallet type not supported by PayU".to_string(),
                        connector: "PayU",
                    });
                }
            };
            // txn_s2s_flow is None for redirect wallet payments (no S2S JSON response expected)
            Ok((Some(pg), Some(bankcode), None, None))
        }
        PaymentMethodData::Netbanking(NetbankingData { bank_code, .. }) => {
            // Net Banking: pg=NB, bankcode=bank_code from NetbankingData
            // Netbanking is also a redirect flow — no S2S flow type needed
            Ok((Some("NB".to_string()), Some(bank_code.clone()), None, None))
        }
        _ => Err(ConnectorError::NotSupported {
            message: "Payment method not supported by PayU. Only UPI, Wallet, and Netbanking payments are supported"
                .to_string(),
            connector: "PayU",
        }),
    }
}

pub fn is_upi_collect_flow<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    // Check if the payment method is UPI Collect
    matches!(
        request.payment_method_data,
        PaymentMethodData::Upi(UpiData::UpiCollect(_))
    )
}

/// Returns true if the payment is a wallet redirect flow (not UPI, not card).
/// For wallet redirect flows, PayU responds with an HTML page, not JSON.
pub fn is_wallet_redirect_flow<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    matches!(
        request.payment_method_data,
        PaymentMethodData::Wallet(_)
    )
}

/// Returns true if the payment is a netbanking redirect flow.
/// For netbanking redirect flows, PayU also responds with an HTML page.
pub fn is_netbanking_redirect_flow<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    matches!(
        request.payment_method_data,
        PaymentMethodData::Netbanking(_)
    )
}

// Hash generation based on Haskell PayU implementation (makePayuTxnHash)
// PayU expects: sha512(key|txnid|amount|productinfo|firstname|email|udf1|udf2|udf3|udf4|udf5|udf6|udf7|udf8|udf9|udf10|salt)
fn generate_payu_hash(
    request: &PayuPaymentRequest,
    merchant_salt: &Secret<String>,
) -> Result<String, ConnectorError> {
    use sha2::{Digest, Sha512};

    // Build hash fields array exactly as PayU expects based on Haskell implementation
    // Pattern from Haskell: key|txnid|amount|productinfo|firstname|email|udf1|udf2|udf3|udf4|udf5|udf6|udf7|udf8|udf9|udf10|salt
    let hash_fields = vec![
        request.key.clone(),                                // key
        request.txnid.clone(),                              // txnid
        request.amount.get_amount_as_string(),              // amount
        request.productinfo.clone(),                        // productinfo
        request.firstname.peek().clone(),                   // firstname
        request.email.peek().clone(),                       // email
        request.udf1.as_deref().unwrap_or("").to_string(),  // udf1
        request.udf2.as_deref().unwrap_or("").to_string(),  // udf2
        request.udf3.as_deref().unwrap_or("").to_string(),  // udf3
        request.udf4.as_deref().unwrap_or("").to_string(),  // udf4
        request.udf5.as_deref().unwrap_or("").to_string(),  // udf5
        request.udf6.as_deref().unwrap_or("").to_string(),  // udf6
        request.udf7.as_deref().unwrap_or("").to_string(),  // udf7
        request.udf8.as_deref().unwrap_or("").to_string(),  // udf8
        request.udf9.as_deref().unwrap_or("").to_string(),  // udf9
        request.udf10.as_deref().unwrap_or("").to_string(), // udf10
        merchant_salt.peek().to_string(),                   // salt
    ];

    // Join with pipe separator as PayU expects
    let hash_string = hash_fields.join("|");

    // Log hash string for debugging (remove in production)
    #[cfg(debug_assertions)]
    {
        if let Some(fields_without_last) = hash_fields.get(..hash_fields.len().saturating_sub(1)) {
            let masked_hash = format!("{}|***MASKED***", fields_without_last.join("|"));
            tracing::debug!("PayU hash string (salt masked): {}", masked_hash);
            tracing::debug!("PayU expected format from Haskell: key|txnid|amount|productinfo|firstname|email|udf1|udf2|udf3|udf4|udf5|udf6|udf7|udf8|udf9|udf10|salt");
        }
    }

    // Generate SHA-512 hash as PayU expects
    let mut hasher = Sha512::new();
    hasher.update(hash_string.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// Response conversion with Framework Integration
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PayuPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PayuPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;

        // Check if this is an error response first
        if let Some(error_code) = &response.error {
            // Extract transaction ID for error response
            let error_transaction_id = response
                .reference_id
                .clone()
                .or_else(|| response.txn_id.clone())
                .or_else(|| response.token.clone());

            // This is an error response - return error with actual status code
            let error_response = ErrorResponse {
                status_code: item.http_code, // Use actual HTTP response code instead of hardcoded 200
                code: error_code.clone(),
                message: response.message.clone().unwrap_or_default(),
                reason: None,
                attempt_status: Some(AttemptStatus::Failure),
                connector_transaction_id: error_transaction_id,
                network_error_message: None,
                network_advice_code: None,
                network_decline_code: None,
            };

            return Ok(Self {
                response: Err(error_response),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Extract reference ID for transaction tracking (success case)
        let upi_transaction_id = response
            .reference_id
            .or_else(|| response.txn_id.clone())
            .or_else(|| response.token.clone())
            .unwrap_or_else(|| {
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });

        // Convert amount back using AmountConvertor framework if available
        let response_amount = if let Some(_amount_str) = response.amount {
            // For now, we'll use the request amount since convert_back has complex requirements
            // This will be improved in the full implementation
            item.router_data.request.minor_amount
        } else {
            item.router_data.request.minor_amount // Use request amount if response doesn't have it
        };

        // Create integrity object for response validation
        let _integrity_object = Some(AuthoriseIntegrityObject {
            amount: response_amount,
            currency: item.router_data.request.currency,
        });

        // This is a success response - determine type based on response format
        let (status, transaction_id, redirection_data) = match &response.status {
            Some(PayuStatusValue::IntStatus(1)) => {
                // UPI Intent success - PayU returns status=1 for successful UPI intent generation
                let redirection_data = response.intent_uri_data.map(|intent_data| {
                    // PayU returns UPI intent parameters that need to be formatted as UPI URI
                    Box::new(RedirectForm::Uri { uri: intent_data })
                });

                (
                    AttemptStatus::AuthenticationPending,
                    upi_transaction_id.clone(),
                    redirection_data,
                )
            }
            Some(PayuStatusValue::StringStatus(s)) if s == "success" => {
                // PayU returns status="success" with a result object for both:
                // 1. UPI Collect: result.status = "pending" while awaiting customer approval
                // 2. Wallet/netbanking redirect: result.status = "pending" with redirect_url set
                let (status, transaction_id) = response
                    .result
                    .as_ref()
                    .map(|result| {
                        let txn_id = result
                            .mihpayid
                            .clone()
                            .unwrap_or_else(|| upi_transaction_id.clone());
                        match result.status.as_str() {
                            "pending" => (AttemptStatus::AuthenticationPending, txn_id),
                            "success" => (AttemptStatus::Charged, txn_id),
                            _ => {
                                // "failure" or any other terminal status from the result wrapper
                                (AttemptStatus::Failure, txn_id)
                            }
                        }
                    })
                    .unwrap_or((AttemptStatus::AuthenticationPending, upi_transaction_id.clone()));

                // Build redirection data from redirect_url and form fields (netbanking/wallet flows)
                let redirection_data = response.redirect_url.as_ref().and_then(|url| {
                    if url.is_empty() {
                        None
                    } else {
                        let form_fields = response
                            .redirect_form_fields
                            .clone()
                            .unwrap_or_default();
                        Some(Box::new(RedirectForm::Form {
                            endpoint: url.clone(),
                            method: Method::Post,
                            form_fields,
                        }))
                    }
                });

                (status, transaction_id, redirection_data)
            }
            _ => {
                // Unknown success status
                (AttemptStatus::Failure, upi_transaction_id.clone(), None)
            }
        };

        let payment_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id),
            redirection_data,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(payment_response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// PayU Sync Response conversion to RouterData
impl TryFrom<ResponseRouterData<PayuSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PayuSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let error_message = response
            .msg
            .unwrap_or_else(|| "PayU PSync error".to_string());
        // Check PayU status field - 0 means error, 1 means success response structure
        match (response.status, response.transaction_details) {
            (Some(1), Some(transaction_details)) => {
                // PayU returned success status, check transaction_details
                // Try to find the transaction in the response by iterating through all transactions
                // Since PayU returns transaction details as a map with txnid as key
                let txn_detail = transaction_details.values().next();

                let connector_transaction_id = txn_detail.and_then(|detail| detail.txnid.clone());
                match (txn_detail, connector_transaction_id) {
                    (Some(txn_detail), Some(connector_transaction_id)) => {
                        // Found transaction details, map status
                        let attempt_status = map_payu_sync_status(&txn_detail.status, txn_detail);

                        let payment_response_data = PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                connector_transaction_id.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: txn_detail.field1.clone(), // UPI transaction ID
                            connector_response_reference_id: txn_detail.mihpayid.clone(),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        };

                        Ok(Self {
                            response: Ok(payment_response_data),
                            resource_common_data: PaymentFlowData {
                                status: attempt_status,
                                ..item.router_data.resource_common_data
                            },
                            ..item.router_data
                        })
                    }
                    _ => {
                        // Transaction not found in PayU response
                        let error_response = ErrorResponse {
                            status_code: item.http_code,
                            code: "TRANSACTION_NOT_FOUND".to_string(),
                            message: error_message,
                            reason: None,
                            attempt_status: Some(AttemptStatus::Failure),
                            connector_transaction_id: None,
                            network_error_message: None,
                            network_advice_code: None,
                            network_decline_code: None,
                        };

                        Ok(Self {
                            response: Err(error_response),
                            resource_common_data: PaymentFlowData {
                                status: AttemptStatus::Failure,
                                ..item.router_data.resource_common_data
                            },
                            ..item.router_data
                        })
                    }
                }
            }
            _ => {
                // PayU returned error status
                let error_response = ErrorResponse {
                    status_code: item.http_code,
                    code: "PAYU_SYNC_ERROR".to_string(),
                    message: error_message,
                    reason: None,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_error_message: None,
                    network_advice_code: None,
                    network_decline_code: None,
                };

                Ok(Self {
                    response: Err(error_response),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}

// Map PayU transaction status to internal AttemptStatus
// Based on Haskell implementation analysis
fn map_payu_sync_status(payu_status: &str, txn_detail: &PayuTransactionDetail) -> AttemptStatus {
    match payu_status.to_lowercase().as_str() {
        "success" => {
            // For success, check if it's captured or just authorized
            // Based on Haskell: "success" + "captured" -> CHARGED, "success" + "auth" -> AUTHORIZED
            if txn_detail.field3.as_deref() == Some("captured") {
                AttemptStatus::Charged
            } else if txn_detail.field3.as_deref() == Some("auth") {
                AttemptStatus::Authorized
            } else {
                // Default success case - treat as charged for UPI
                AttemptStatus::Charged
            }
        }
        "pending" => {
            // Pending status - typically for UPI Collect waiting for customer approval
            AttemptStatus::AuthenticationPending
        }
        "failure" | "failed" | "cancel" | "cancelled" => {
            // Transaction failed
            AttemptStatus::Failure
        }
        _ => {
            // Unknown status - treat as failure for safety
            AttemptStatus::Failure
        }
    }
}

// ============================================================
// Capture Flow
// ============================================================

// PayU Capture Request structure
// Spec: key | command="capture_transaction" | var1=mihpayid | var2=amount | hash
#[derive(Debug, Serialize)]
pub struct PayuCaptureRequest {
    pub key: String,     // Merchant key
    pub command: String, // "capture_transaction"
    pub var1: String,    // PayU payment ID (mihpayid / connector_transaction_id)
    pub var2: String,    // Amount to capture
    pub hash: String,    // SHA-512 signature: SHA512(key|command|var1|salt)
}

// PayU Capture Response structure
// Based on spec section 4.2 / Flow.hs:1453 — capture returns a simple status
// Note: PayU returns status as integer (0) in error responses and string ("success") in success responses
#[derive(Debug, Deserialize, Serialize)]
pub struct PayuCaptureResponse {
    #[serde(default, deserialize_with = "deserialize_payu_status")]
    pub status: Option<PayuStatusValue>,   // e.g. "success" (string) or 0 (integer for error)
    pub message: Option<String>,           // Status message
    pub msg: Option<String>,               // Alternative message field used in error responses
    pub mihpayid: Option<String>,          // PayU payment ID
    pub error_code: Option<String>,        // Error code if failed
    pub error_description: Option<String>, // Error description if failed
}

// Hash generation for PayU capture request
// Formula: SHA512(key|command|var1|salt)  (standard makePayuHash)
fn generate_payu_capture_hash(
    request: &PayuCaptureRequest,
    merchant_salt: &Secret<String>,
) -> Result<String, ConnectorError> {
    use sha2::{Digest, Sha512};

    // PayU standard hash format: key|command|var1|salt
    let hash_string = format!(
        "{}|{}|{}|{}",
        request.key,
        request.command,
        request.var1,
        merchant_salt.peek()
    );

    let mut hasher = Sha512::new();
    hasher.update(hash_string.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// RouterDataV2 → PayuCaptureRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PayuRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PayuCaptureRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: super::PayuRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth
        let auth = PayuAuthType::try_from(&router_data.connector_config)?;

        // Purpose: API requires original PayU payment ID (mihpayid) for capture
        let connector_transaction_id = router_data
            .request
            .get_connector_transaction_id()
            .change_context(ConnectorError::MissingConnectorTransactionID)?;

        // Convert amount using the amount converter
        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(ConnectorError::AmountConversionFailed)?;

        let command = "capture_transaction".to_string();

        let mut request = Self {
            key: auth.api_key.peek().to_string(),
            command: command.clone(),
            var1: connector_transaction_id,
            var2: amount.get_amount_as_string(),
            hash: String::new(), // Computed below
        };

        // Generate hash: SHA512(key|command|var1|salt)
        request.hash = generate_payu_capture_hash(&request, &auth.api_secret)?;

        Ok(request)
    }
}

// PayuCaptureResponse → RouterDataV2
// Note: PaymentsCaptureData is not generic over T, so no T parameter here
impl TryFrom<ResponseRouterData<PayuCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayuCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Map connector status to internal status
        // Per spec Flow.hs:1453: success → CHARGED, error → CAPTURE_PROCESSING_FAILED
        // PayU returns status as string "success" on success, or integer 0 on error
        let is_success = match &response.status {
            Some(PayuStatusValue::StringStatus(s)) => s.as_str() == "success",
            Some(PayuStatusValue::IntStatus(0)) => false, // 0 = error/failure
            Some(PayuStatusValue::IntStatus(_)) => false, // any other int = treat as non-success
            None => false,
        };

        let attempt_status = if is_success {
            AttemptStatus::Charged
        } else {
            AttemptStatus::CaptureFailed
        };

        // Check for error response: integer status 0 or error_code present
        let has_error = matches!(&response.status, Some(PayuStatusValue::IntStatus(0)))
            || response.error_code.is_some();

        // Get the error message — check both `msg` and `message` fields
        let error_msg = response
            .msg
            .clone()
            .or_else(|| response.error_description.clone())
            .or_else(|| response.message.clone());

        if has_error {
            let error_code = response
                .error_code
                .clone()
                .unwrap_or_else(|| "CAPTURE_PROCESSING_FAILED".to_string());
            let error_response = ErrorResponse {
                status_code: item.http_code,
                code: error_code,
                message: error_msg.unwrap_or_default(),
                reason: None,
                attempt_status: Some(AttemptStatus::CaptureFailed),
                connector_transaction_id: response.mihpayid.clone(),
                network_error_message: None,
                network_advice_code: None,
                network_decline_code: None,
            };

            return Ok(Self {
                response: Err(error_response),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let connector_transaction_id = response
            .mihpayid
            .clone()
            .unwrap_or_else(|| {
                item.router_data
                    .request
                    .get_connector_transaction_id()
                    .unwrap_or_default()
            });

        let payment_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(connector_transaction_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(payment_response_data),
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================
// Void Flow
// Spec section 3.7: PayuVoidRequest
// command = "cancel_refund_transaction", var1 = payuId (mihpayid)
// Hash: SHA512(key|command|var1|salt)
// ============================================================

#[derive(Debug, Serialize)]
pub struct PayuVoidRequest {
    pub key: String,     // Merchant key
    pub command: String, // "cancel_refund_transaction"
    pub var1: String,    // PayU payment ID (mihpayid / connector_transaction_id)
    pub hash: String,    // SHA-512 signature: SHA512(key|command|var1|salt)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayuVoidResponse {
    #[serde(default, deserialize_with = "deserialize_payu_status")]
    pub status: Option<PayuStatusValue>,   // e.g. "success" (string) or 0 (integer for error)
    pub message: Option<String>,           // Status message
    pub msg: Option<String>,               // Alternative message field used in error responses
    pub mihpayid: Option<String>,          // PayU payment ID
    pub error_code: Option<String>,        // Error code if failed
    pub error_description: Option<String>, // Error description if failed
}

fn generate_payu_void_hash(
    request: &PayuVoidRequest,
    merchant_salt: &Secret<String>,
) -> Result<String, ConnectorError> {
    use sha2::{Digest, Sha512};

    // PayU void hash format: key|command|var1|salt
    let hash_string = format!(
        "{}|{}|{}|{}",
        request.key,
        request.command,
        request.var1,
        merchant_salt.peek()
    );

    let mut hasher = Sha512::new();
    hasher.update(hash_string.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// RouterDataV2 → PayuVoidRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PayuRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PayuVoidRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: super::PayuRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth
        let auth = PayuAuthType::try_from(&router_data.connector_config)?;

        // Purpose: API requires original PayU payment ID (mihpayid) for void
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        let command = "cancel_refund_transaction".to_string();

        let mut request = Self {
            key: auth.api_key.peek().to_string(),
            command: command.clone(),
            var1: connector_transaction_id,
            hash: String::new(), // Computed below
        };

        // Generate hash: SHA512(key|command|var1|salt)
        request.hash = generate_payu_void_hash(&request, &auth.api_secret)?;

        Ok(request)
    }
}

// PayuVoidResponse → RouterDataV2
impl TryFrom<ResponseRouterData<PayuVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayuVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Map connector status to internal status
        // Per spec Flow.hs:1478: success → VOIDED, error → VOID_PROCESSING_FAILED
        let attempt_status = match &response.status {
            Some(PayuStatusValue::StringStatus(s)) if s.as_str() == "success" => AttemptStatus::Voided,
            Some(PayuStatusValue::StringStatus(s))
                if matches!(s.as_str(), "failure" | "failed" | "error") =>
            {
                AttemptStatus::VoidFailed
            }
            Some(PayuStatusValue::IntStatus(0)) => AttemptStatus::VoidFailed, // 0 = error
            Some(PayuStatusValue::IntStatus(_)) => AttemptStatus::VoidFailed, // any other int = non-success
            _ => {
                if item.http_code >= 200 && item.http_code < 300 {
                    AttemptStatus::Voided
                } else {
                    AttemptStatus::VoidFailed
                }
            }
        };

        // Check for error response: integer status 0 or error_code present
        let has_void_error = matches!(&response.status, Some(PayuStatusValue::IntStatus(0)))
            || response.error_code.is_some();

        if has_void_error {
            let error_code = response
                .error_code
                .clone()
                .unwrap_or_else(|| "PAYU_VOID_ERROR".to_string());
            let error_message = response
                .error_description
                .clone()
                .or_else(|| response.msg.clone())
                .or_else(|| response.message.clone())
                .unwrap_or_else(|| "PayU void error".to_string());
            let error_response = ErrorResponse {
                status_code: item.http_code,
                code: error_code,
                message: error_message,
                reason: None,
                attempt_status: Some(AttemptStatus::VoidFailed),
                connector_transaction_id: response.mihpayid.clone(),
                network_error_message: None,
                network_advice_code: None,
                network_decline_code: None,
            };

            return Ok(Self {
                response: Err(error_response),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let connector_transaction_id = response
            .mihpayid
            .clone()
            .unwrap_or_else(|| item.router_data.request.connector_transaction_id.clone());

        let payment_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(connector_transaction_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(payment_response_data),
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================
// Refund Flow
// Spec section 3.5: PayuRefundRequest
// command = "cancel_refund_transaction"
// var1 = mihpayid (connector_transaction_id)
// var2 = refund amount
// var3 = txnid (merchant transaction id / refund_id)
// Hash: SHA512(key|command|var1|salt)
// ============================================================

#[derive(Debug, Serialize)]
pub struct PayuRefundRequest {
    pub key: String,
    pub command: String,
    pub var1: String,
    pub var2: String,
    pub var3: String,
    pub hash: String,
}

// PayU Refund Response - Techspec 4.6: SuccessRefundFetch | SplitRefundFetch | FailureRefundResponse
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PayuRefundResponse {
    #[serde(default, deserialize_with = "deserialize_payu_status")]
    pub status: Option<PayuStatusValue>,   // e.g. "success" (string) or 0 (integer for error)
    pub message: Option<String>,           // Status message
    pub msg: Option<String>,               // Alternative message field used in error responses
    pub mihpayid: Option<String>,
    pub refundId: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

fn generate_payu_refund_hash(
    request: &PayuRefundRequest,
    merchant_salt: &Secret<String>,
) -> Result<String, ConnectorError> {
    use sha2::{Digest, Sha512};
    let hash_string = format!(
        "{}|{}|{}|{}",
        request.key, request.command, request.var1, merchant_salt.peek()
    );
    let mut hasher = Sha512::new();
    hasher.update(hash_string.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// RouterDataV2 -> PayuRefundRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PayuRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PayuRefundRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: super::PayuRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PayuAuthType::try_from(&router_data.connector_config)?;
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();
        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.minor_refund_amount, router_data.request.currency)
            .change_context(ConnectorError::AmountConversionFailed)?;
        let txnid = router_data.request.refund_id.clone();
        let command = "cancel_refund_transaction".to_string();
        let mut request = Self {
            key: auth.api_key.peek().to_string(),
            command,
            var1: connector_transaction_id,
            var2: amount.get_amount_as_string(),
            var3: txnid,
            hash: String::new(),
        };
        request.hash = generate_payu_refund_hash(&request, &auth.api_secret)?;
        Ok(request)
    }
}

/// Techspec section 7.11: Refund Status Mapping
fn map_payu_refund_status(status: &PayuStatusValue) -> RefundStatus {
    match status {
        PayuStatusValue::IntStatus(0) => RefundStatus::Failure,
        PayuStatusValue::IntStatus(_) => RefundStatus::Pending,
        PayuStatusValue::StringStatus(s) => match s.to_lowercase().as_str() {
            "success" => RefundStatus::Success,
            "failure" | "failed" => RefundStatus::Failure,
            "od_hit" => RefundStatus::Pending,
            _ => RefundStatus::Pending,
        },
    }
}

// PayuRefundResponse -> RouterDataV2
impl TryFrom<ResponseRouterData<PayuRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PayuRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // FailureRefundResponse: has error_code or integer status 0
        let has_refund_error = matches!(&response.status, Some(PayuStatusValue::IntStatus(0)))
            || response.error_code.is_some();

        if has_refund_error {
            let error_code = response
                .error_code
                .clone()
                .unwrap_or_else(|| "PAYU_REFUND_ERROR".to_string());
            let error_message = response
                .error_description
                .clone()
                .or_else(|| response.msg.clone())
                .or_else(|| response.message.clone())
                .unwrap_or_else(|| "PayU refund error".to_string());
            let error_response = ErrorResponse {
                status_code: item.http_code,
                code: error_code,
                message: error_message,
                reason: None,
                attempt_status: None,
                connector_transaction_id: response.mihpayid.clone(),
                network_error_message: None,
                network_advice_code: None,
                network_decline_code: None,
            };
            return Ok(Self {
                response: Err(error_response),
                resource_common_data: RefundFlowData {
                    status: RefundStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let refund_status = response
            .status
            .as_ref()
            .map(map_payu_refund_status)
            .unwrap_or(RefundStatus::Pending);

        let connector_refund_id = response
            .refundId
            .clone()
            .or_else(|| response.mihpayid.clone())
            .unwrap_or_else(|| item.router_data.request.refund_id.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================
// RSync (Refund Sync) Flow
// Spec section 3.4: PayuVerifyPaymentRequest
// command = "verify_payment", var1 = connector_transaction_id (original txnId)
// Hash: SHA512(key|command|var1|salt)
// The response (PayuSyncResponse) includes transaction_details with refund status
// ============================================================

#[derive(Debug, Serialize)]
pub struct PayuRefundSyncRequest {
    pub key: String,     // Merchant key
    pub command: String, // "verify_payment"
    pub var1: String,    // Transaction ID (connector_transaction_id)
    pub hash: String,    // SHA-512 signature: SHA512(key|command|var1|salt)
}

// PayU Refund Sync Response — same structure as verify_payment response
// The transaction_details map includes refund status info
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PayuRefundSyncResponse {
    #[serde(default, deserialize_with = "deserialize_payu_status")]
    pub status: Option<PayuStatusValue>, // 0 (integer) = error, 1 (integer) = success
    pub msg: Option<String>,             // Status message (used in error responses)
    pub transaction_details: Option<std::collections::HashMap<String, PayuTransactionDetail>>,
    pub result: Option<serde_json::Value>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub message: Option<String>,
}

fn generate_payu_refund_sync_hash(
    request: &PayuRefundSyncRequest,
    merchant_salt: &Secret<String>,
) -> Result<String, ConnectorError> {
    use sha2::{Digest, Sha512};

    // PayU verify hash format: key|command|var1|salt
    let hash_string = format!(
        "{}|{}|{}|{}",
        request.key,
        request.command,
        request.var1,
        merchant_salt.peek()
    );

    let mut hasher = Sha512::new();
    hasher.update(hash_string.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// RouterDataV2 -> PayuRefundSyncRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PayuRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for PayuRefundSyncRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: super::PayuRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth
        let auth = PayuAuthType::try_from(&router_data.connector_config)?;

        // Use connector_transaction_id (original payment txn ID) for verify_payment
        let transaction_id = router_data.request.connector_transaction_id.clone();

        let command = "verify_payment".to_string();

        let mut request = Self {
            key: auth.api_key.peek().to_string(),
            command,
            var1: transaction_id,
            hash: String::new(), // Computed below
        };

        // Generate hash: SHA512(key|command|var1|salt)
        request.hash = generate_payu_refund_sync_hash(&request, &auth.api_secret)?;

        Ok(request)
    }
}

// PayuRefundSyncResponse -> RouterDataV2
// Maps PayU transaction verify response to refund sync status
impl TryFrom<ResponseRouterData<PayuRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayuRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let error_message = response
            .msg
            .clone()
            .or_else(|| response.message.clone())
            .unwrap_or_else(|| "PayU RSync error".to_string());

        // Check PayU status field - IntStatus(0) means error, IntStatus(1) means success
        let is_rsync_success = matches!(
            &response.status,
            Some(PayuStatusValue::IntStatus(n)) if *n != 0
        );
        match (is_rsync_success, response.transaction_details) {
            (true, Some(transaction_details)) => {
                // PayU returned success status with transaction details
                // Find the refund entry in transaction details
                let txn_detail = transaction_details.values().next();

                match txn_detail {
                    Some(txn_detail) => {
                        // Map the refund status from transaction detail status
                        // Techspec 7.11: "success" -> SUCCESS, "failure"/"failed" -> FAILURE,
                        // "od_hit" -> PENDING, other -> PENDING
                        let refund_status = match txn_detail.status.to_lowercase().as_str() {
                            "success" => RefundStatus::Success,
                            "failure" | "failed" => RefundStatus::Failure,
                            _ => RefundStatus::Pending,
                        };

                        let connector_refund_id = txn_detail
                            .mihpayid
                            .clone()
                            .unwrap_or_else(|| item.router_data.request.connector_refund_id.clone());

                        Ok(Self {
                            response: Ok(RefundsResponseData {
                                connector_refund_id,
                                refund_status,
                                status_code: item.http_code,
                            }),
                            resource_common_data: RefundFlowData {
                                status: refund_status,
                                ..item.router_data.resource_common_data
                            },
                            ..item.router_data
                        })
                    }
                    None => {
                        // Transaction details not found — treat as pending
                        let connector_refund_id =
                            item.router_data.request.connector_refund_id.clone();
                        Ok(Self {
                            response: Ok(RefundsResponseData {
                                connector_refund_id,
                                refund_status: RefundStatus::Pending,
                                status_code: item.http_code,
                            }),
                            resource_common_data: RefundFlowData {
                                status: RefundStatus::Pending,
                                ..item.router_data.resource_common_data
                            },
                            ..item.router_data
                        })
                    }
                }
            }
            _ => {
                // PayU returned error status - return error response
                let error_response = ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .error_code
                        .unwrap_or_else(|| "PAYU_RSYNC_ERROR".to_string()),
                    message: error_message,
                    reason: None,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_error_message: None,
                    network_advice_code: None,
                    network_decline_code: None,
                };

                Ok(Self {
                    response: Err(error_response),
                    resource_common_data: RefundFlowData {
                        status: RefundStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}
