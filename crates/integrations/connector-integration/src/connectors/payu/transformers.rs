use common_enums::{self, AttemptStatus, Currency};
use common_utils::{pii::IpAddress, Email};
use domain_types::{
    connector_flow::{Authorize, PSync, RepeatPayment},
    connector_types::{
        MandateReferenceId, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RepeatPaymentData, ResponseId,
    },
    errors::{ConnectorResponseTransformationError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, UpiData},
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
    type Error = error_stack::Report<IntegrationError>;

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
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
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
    pub txn_s2s_flow: String, // S2S flow type ("2" for UPI)
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
    #[serde(alias = "txnId")]
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
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayuResult {
    pub status: String,   // UPI Collect Status
    pub mihpayid: String, // ID
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
    type Error = error_stack::Report<IntegrationError>;

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
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Extract authentication
        let auth = PayuAuthType::try_from(&router_data.connector_config)?;

        // Determine payment flow based on payment method
        let (pg, bankcode, vpa, s2s_flow) = determine_upi_flow(&router_data.request)?;

        // Generate UDF fields based on Haskell implementation
        let udf_fields = generate_udf_fields(
            &router_data.resource_common_data.payment_id,
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
            firstname: router_data
                .resource_common_data
                .get_billing_first_name()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "billing.first_name",
                    context: Default::default(),
                })?,
            lastname: router_data
                .resource_common_data
                .get_optional_billing_last_name(),
            email: router_data
                .resource_common_data
                .get_billing_email()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "billing.email",
                    context: Default::default(),
                })?,
            phone: router_data
                .resource_common_data
                .get_billing_phone_number()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "billing.phone_number",
                    context: Default::default(),
                })?,

            // URLs - use router return URL if available
            surl: router_data.request.get_router_return_url().map_err(|_| {
                IntegrationError::MissingRequiredField {
                    field_name: "router_return_url",
                    context: Default::default(),
                }
            })?,
            furl: router_data.request.get_router_return_url().map_err(|_| {
                IntegrationError::MissingRequiredField {
                    field_name: "router_return_url",
                    context: Default::default(),
                }
            })?,

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
                    report!(IntegrationError::MissingRequiredField {
                        field_name: "IP address",
                        context: Default::default()
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
    type Error = error_stack::Report<IntegrationError>;

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
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "connector_transaction_id",
                context: Default::default(),
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
) -> Result<String, IntegrationError> {
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
        // udf1: From metadata "udf1" or default to transaction ID
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
) -> Result<Option<String>, IntegrationError> {
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
        _ => Ok(None),
    }
}

// PayU flow determination based on Haskell getTxnS2SType implementation
#[allow(clippy::type_complexity)]
fn determine_upi_flow<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<(Option<String>, Option<String>, Option<String>, String), IntegrationError> {
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
                            constants::UPI_S2S_FLOW.to_string(), // UPI Collect typically uses S2S flow "2"
                        ))
                    } else {
                        // Missing VPA for UPI Collect - this should be an error
                        Err(IntegrationError::MissingRequiredField {
                            field_name: "vpa_id",
                            context: Default::default(),
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
                        constants::UPI_S2S_FLOW.to_string(),
                    ))
                }
            }
        }
        _ => Err(IntegrationError::NotSupported {
            message: "Payment method not supported by PayU. Only UPI payments are supported"
                .to_string(),
            connector: "PayU",
            context: Default::default(),
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

// Hash generation based on Haskell PayU implementation (makePayuTxnHash)
// PayU expects: sha512(key|txnid|amount|productinfo|firstname|email|udf1|udf2|udf3|udf4|udf5|udf6|udf7|udf8|udf9|udf10|salt)
fn generate_payu_hash(
    request: &PayuPaymentRequest,
    merchant_salt: &Secret<String>,
) -> Result<String, IntegrationError> {
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
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

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
            .unwrap_or_else(|| item.router_data.resource_common_data.payment_id.clone());

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
                // UPI Collect success - PayU returns status="success" with result object
                let (status, transaction_id) = response
                    .result
                    .map(|result| {
                        if result.status == "pending" {
                            (
                                AttemptStatus::AuthenticationPending,
                                result.mihpayid.clone(),
                            )
                        } else {
                            (AttemptStatus::Failure, result.mihpayid.clone())
                        }
                    })
                    .unwrap_or((AttemptStatus::Failure, "".to_owned()));
                (status, transaction_id, None)
            }
            _ => {
                // Unknown success status
                (AttemptStatus::Failure, upi_transaction_id.clone(), None)
            }
        };

        let payment_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
            redirection_data,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(transaction_id),
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
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

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

// === RepeatPayment (MIT) Request and Response Types ===

// PayU MIT/Recurring Request - uses the same /_payment endpoint
// For merchant-initiated transactions, PayU India requires the user_token
// (card token from a previous transaction) to charge without 2FA
#[derive(Debug, Serialize)]
pub struct PayuRepeatPaymentRequest {
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

    // MIT-specific fields
    pub user_token: String, // Card/mandate token from previous transaction (connector_mandate_id)

    // S2S fields
    pub txn_s2s_flow: String,                 // S2S flow type
    pub s2s_client_ip: Secret<String, IpAddress>, // Client IP
    pub s2s_device_info: String,              // Device info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,          // API version

    // Security
    pub hash: String, // SHA-512 signature

    // User defined fields
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
}

// PayU MIT Response - same structure as regular payment response
#[derive(Debug, Deserialize, Serialize)]
pub struct PayuRepeatPaymentResponse {
    // Success response fields
    #[serde(deserialize_with = "deserialize_payu_status")]
    pub status: Option<PayuStatusValue>,
    pub token: Option<String>,
    #[serde(alias = "referenceId")]
    pub reference_id: Option<String>,
    #[serde(alias = "returnUrl")]
    pub return_url: Option<String>,
    #[serde(alias = "merchantName")]
    pub merchant_name: Option<String>,
    pub amount: Option<String>,
    #[serde(alias = "txnId")]
    pub txn_id: Option<String>,

    // Result field for collect-style flows
    pub result: Option<PayuResult>,

    // Error response fields
    pub error: Option<String>,
    pub message: Option<String>,
}

// Hash generation for RepeatPayment request
// Same format as regular payment: sha512(key|txnid|amount|productinfo|firstname|email|udf1..udf10|salt)
fn generate_payu_repeat_payment_hash(
    request: &PayuRepeatPaymentRequest,
    merchant_salt: &Secret<String>,
) -> Result<String, IntegrationError> {
    use sha2::{Digest, Sha512};

    let hash_fields = vec![
        request.key.clone(),
        request.txnid.clone(),
        request.amount.get_amount_as_string(),
        request.productinfo.clone(),
        request.firstname.peek().clone(),
        request.email.peek().clone(),
        request.udf1.as_deref().unwrap_or("").to_string(),
        request.udf2.as_deref().unwrap_or("").to_string(),
        request.udf3.as_deref().unwrap_or("").to_string(),
        request.udf4.as_deref().unwrap_or("").to_string(),
        request.udf5.as_deref().unwrap_or("").to_string(),
        request.udf6.as_deref().unwrap_or("").to_string(),
        request.udf7.as_deref().unwrap_or("").to_string(),
        request.udf8.as_deref().unwrap_or("").to_string(),
        request.udf9.as_deref().unwrap_or("").to_string(),
        request.udf10.as_deref().unwrap_or("").to_string(),
        merchant_salt.peek().to_string(),
    ];

    let hash_string = hash_fields.join("|");
    let mut hasher = Sha512::new();
    hasher.update(hash_string.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

// Helper to extract connector mandate ID from MandateReferenceId
fn extract_connector_mandate_id(
    mandate_reference: &MandateReferenceId,
) -> Result<String, IntegrationError> {
    match mandate_reference {
        MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
            .get_connector_mandate_id()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            }),
        MandateReferenceId::NetworkMandateId(network_transaction_id) => {
            Ok(network_transaction_id.clone())
        }
        MandateReferenceId::NetworkTokenWithNTI(_) => Err(IntegrationError::NotSupported {
            message: "Network token with NTI not supported for PayU MIT".to_string(),
            connector: "PayU",
            context: Default::default(),
        }),
    }
}

// RepeatPayment Request conversion from RouterData
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PayuRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayuRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::PayuRouterData<
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

        // Use AmountConvertor framework for proper amount handling
        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Extract authentication
        let auth = PayuAuthType::try_from(&router_data.connector_config)?;

        // Extract mandate token (user_token) from mandate reference
        let user_token =
            extract_connector_mandate_id(&router_data.request.mandate_reference)?;

        // Extract metadata for UDF fields
        let metadata = router_data.request.metadata.as_ref();
        let get_metadata_field = |field: &str| -> Option<String> {
            metadata
                .and_then(|m| m.peek().get(field))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };

        // Build request
        let mut request = Self {
            key: auth.api_key.peek().to_string(),
            txnid: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            currency: router_data.request.currency,
            productinfo: constants::PRODUCT_INFO.to_string(),

            // Customer info
            firstname: router_data
                .resource_common_data
                .get_billing_first_name()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "billing.first_name",
                    context: Default::default(),
                })?,
            lastname: router_data
                .resource_common_data
                .get_optional_billing_last_name(),
            email: router_data
                .resource_common_data
                .get_billing_email()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "billing.email",
                    context: Default::default(),
                })?,
            phone: router_data
                .resource_common_data
                .get_billing_phone_number()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "billing.phone_number",
                    context: Default::default(),
                })?,

            // URLs
            surl: router_data
                .request
                .router_return_url
                .clone()
                .unwrap_or_else(|| "https://example.com/success".to_string()),
            furl: router_data
                .request
                .router_return_url
                .clone()
                .unwrap_or_else(|| "https://example.com/failure".to_string()),

            // MIT-specific: stored card token
            user_token,

            // S2S fields
            txn_s2s_flow: constants::UPI_S2S_FLOW.to_string(),
            s2s_client_ip: router_data
                .request
                .get_ip_address_as_optional()
                .ok_or_else(|| {
                    report!(IntegrationError::MissingRequiredField {
                        field_name: "IP address",
                        context: Default::default()
                    })
                })?,
            s2s_device_info: constants::DEVICE_INFO.to_string(),
            api_version: Some(constants::API_VERSION.to_string()),

            // Hash will be calculated below
            hash: String::new(),

            // UDF fields from metadata
            udf1: get_metadata_field("udf1")
                .or(Some(router_data.resource_common_data.payment_id.clone())),
            udf2: get_metadata_field("udf2").or(Some(
                router_data
                    .resource_common_data
                    .merchant_id
                    .get_string_repr()
                    .to_string(),
            )),
            udf3: get_metadata_field("udf3"),
            udf4: get_metadata_field("udf4"),
            udf5: get_metadata_field("udf5"),
            udf6: get_metadata_field("udf6"),
            udf7: get_metadata_field("udf7"),
            udf8: get_metadata_field("udf8"),
            udf9: get_metadata_field("udf9"),
            udf10: get_metadata_field("udf10"),
        };

        // Generate hash signature
        request.hash = generate_payu_repeat_payment_hash(&request, &auth.api_secret)?;

        Ok(request)
    }
}

// RepeatPayment Response conversion
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PayuRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<PayuRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        // Check if this is an error response
        if let Some(error_code) = &response.error {
            let error_transaction_id = response
                .reference_id
                .clone()
                .or_else(|| response.txn_id.clone())
                .or_else(|| response.token.clone());

            let error_response = ErrorResponse {
                status_code: item.http_code,
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

        // Extract transaction ID
        let transaction_id = response
            .reference_id
            .or_else(|| response.txn_id.clone())
            .or_else(|| response.token.clone())
            .unwrap_or_else(|| item.router_data.resource_common_data.payment_id.clone());

        // Determine status based on response
        let (status, connector_txn_id) = match &response.status {
            Some(PayuStatusValue::IntStatus(1)) => {
                // Success - MIT payment processed
                (AttemptStatus::Charged, transaction_id.clone())
            }
            Some(PayuStatusValue::StringStatus(s)) if s == "success" => {
                // Success response with result object
                let (status, txn_id) = response
                    .result
                    .map(|result| {
                        let st = match result.status.to_lowercase().as_str() {
                            "success" | "captured" => AttemptStatus::Charged,
                            "pending" => AttemptStatus::Pending,
                            "auth" => AttemptStatus::Authorized,
                            _ => AttemptStatus::Failure,
                        };
                        (st, result.mihpayid)
                    })
                    .unwrap_or((AttemptStatus::Charged, transaction_id.clone()));
                (status, txn_id)
            }
            _ => (AttemptStatus::Failure, transaction_id.clone()),
        };

        let payment_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(connector_txn_id),
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
