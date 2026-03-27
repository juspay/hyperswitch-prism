use base64::Engine;
use common_enums;
use common_utils::{
    crypto::{self, GenerateDigest},
    ext_traits::Encode,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{Authorize, PSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, UpiData, UpiSource},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::BrowserInformation,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

pub const NEXT_ACTION_DATA: &str = "nextActionData";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextActionData {
    WaitScreenInstructions,
}

use super::constants;
use crate::{connectors::phonepe::PhonepeRouterData, types::ResponseRouterData};

type Error = error_stack::Report<errors::ConnectorError>;

// ===== AMOUNT CONVERSION =====
// Using macro-generated PhonepeRouterData from crate::connectors::phonepe

// ===== REQUEST STRUCTURES =====

#[derive(Debug, Serialize)]
pub struct PhonepePaymentsRequest {
    request: Secret<String>,
    #[serde(skip)]
    pub checksum: String,
}

#[derive(Debug, Serialize)]
struct PhonepePaymentRequestPayload {
    #[serde(rename = "merchantId")]
    merchant_id: Secret<String>,
    #[serde(rename = "merchantTransactionId")]
    merchant_transaction_id: String,
    #[serde(rename = "merchantUserId", skip_serializing_if = "Option::is_none")]
    merchant_user_id: Option<Secret<String>>,
    amount: MinorUnit,
    #[serde(rename = "callbackUrl")]
    callback_url: String,
    #[serde(rename = "mobileNumber", skip_serializing_if = "Option::is_none")]
    mobile_number: Option<Secret<String>>,
    #[serde(rename = "paymentInstrument")]
    payment_instrument: PhonepePaymentInstrument,
    #[serde(rename = "deviceContext", skip_serializing_if = "Option::is_none")]
    device_context: Option<PhonepeDeviceContext>,
    #[serde(rename = "paymentMode", skip_serializing_if = "Option::is_none")]
    payment_mode: Option<String>,
}

#[derive(Debug, Serialize)]
struct PhonepeDeviceContext {
    #[serde(rename = "deviceOS", skip_serializing_if = "Option::is_none")]
    device_os: Option<String>,
}

#[derive(Debug, Serialize)]
struct PhonepePaymentInstrument {
    #[serde(rename = "type")]
    instrument_type: String,
    #[serde(rename = "targetApp", skip_serializing_if = "Option::is_none")]
    target_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vpa: Option<Secret<String>>,
}

// ===== SYNC REQUEST STRUCTURES =====

#[derive(Debug, Serialize)]
pub struct PhonepeSyncRequest {
    #[serde(skip)]
    pub merchant_transaction_id: String,
    #[serde(skip)]
    pub checksum: String,
}

// ===== RESPONSE STRUCTURES =====

#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepeErrorResponse {
    pub success: bool,
    pub code: String,
    #[serde(default = "default_error_message")]
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepePaymentsResponse {
    pub success: bool,
    pub code: String,
    pub message: String,
    pub data: Option<PhonepeResponseData>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepeSyncResponse {
    pub success: bool,
    pub code: String,
    #[serde(default = "default_sync_error_message")]
    pub message: String,
    #[serde(default)]
    pub data: Option<PhonepeSyncResponseData>,
}

fn default_error_message() -> String {
    "Payment processing failed".to_string()
}

fn default_sync_error_message() -> String {
    "Payment sync failed".to_string()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepeResponseData {
    #[serde(rename = "merchantId")]
    merchant_id: String,
    #[serde(rename = "merchantTransactionId")]
    merchant_transaction_id: String,
    #[serde(rename = "transactionId", skip_serializing_if = "Option::is_none")]
    transaction_id: Option<String>,
    #[serde(rename = "instrumentResponse", skip_serializing_if = "Option::is_none")]
    instrument_response: Option<PhonepeInstrumentResponse>,
    #[serde(rename = "responseCode", skip_serializing_if = "Option::is_none")]
    response_code: Option<String>,
    #[serde(
        rename = "responseCodeDescription",
        skip_serializing_if = "Option::is_none"
    )]
    response_code_description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepeInstrumentResponse {
    #[serde(rename = "type")]
    instrument_type: String,
    #[serde(rename = "intentUrl", skip_serializing_if = "Option::is_none")]
    intent_url: Option<String>,
    #[serde(rename = "qrData", skip_serializing_if = "Option::is_none")]
    qr_data: Option<String>,
    // Fields for UPI CC/CL detection
    #[serde(rename = "accountType", skip_serializing_if = "Option::is_none")]
    account_type: Option<String>,
    #[serde(rename = "cardNetwork", skip_serializing_if = "Option::is_none")]
    card_network: Option<String>,
    #[serde(rename = "upiCreditLine", skip_serializing_if = "Option::is_none")]
    upi_credit_line: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepePaymentInstrumentSync {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub instrument_type: Option<String>,
    #[serde(rename = "cardNetwork", skip_serializing_if = "Option::is_none")]
    pub card_network: Option<String>,
    #[serde(rename = "accountType", skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    #[serde(rename = "upiCreditLine", skip_serializing_if = "Option::is_none")]
    pub upi_credit_line: Option<bool>,
    #[serde(
        rename = "maskedAccountNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub masked_account_number: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepeSyncResponseData {
    #[serde(rename = "merchantId", skip_serializing_if = "Option::is_none")]
    merchant_id: Option<String>,
    #[serde(
        rename = "merchantTransactionId",
        skip_serializing_if = "Option::is_none"
    )]
    merchant_transaction_id: Option<String>,
    #[serde(rename = "transactionId", skip_serializing_if = "Option::is_none")]
    transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(rename = "responseCode", skip_serializing_if = "Option::is_none")]
    response_code: Option<String>,
    #[serde(rename = "paymentInstrument", skip_serializing_if = "Option::is_none")]
    pub payment_instrument: Option<PhonepePaymentInstrumentSync>,
}

// ===== REQUEST BUILDING =====

// TryFrom implementation for macro-generated PhonepeRouterData wrapper (owned)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PhonepeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PhonepePaymentsRequest
{
    type Error = Error;

    fn try_from(
        wrapper: PhonepeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = PhonepeAuthType::try_from(&router_data.connector_config)?;

        // Use amount converter to get proper amount in minor units
        let amount_in_minor_units = wrapper
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        // Get customer mobile number from billing address
        let mobile_number = router_data
            .resource_common_data
            .get_optional_billing_phone_number()
            .map(|phone| Secret::new(phone.peek().to_string()));

        // Create payment instrument based on payment method data
        let payment_instrument = match &router_data.request.payment_method_data {
            PaymentMethodData::Upi(upi_data) => match upi_data {
                UpiData::UpiIntent(intent_data) => {
                    let target_app =
                        get_target_app_for_phonepe(intent_data, &router_data.request.browser_info);
                    PhonepePaymentInstrument {
                        instrument_type: constants::UPI_INTENT.to_string(),
                        target_app,
                        vpa: None,
                    }
                }
                UpiData::UpiQr(_) => PhonepePaymentInstrument {
                    instrument_type: constants::UPI_QR.to_string(),
                    target_app: None,
                    vpa: None,
                },
                UpiData::UpiCollect(collect_data) => PhonepePaymentInstrument {
                    instrument_type: constants::UPI_COLLECT.to_string(),
                    target_app: None,
                    vpa: collect_data
                        .vpa_id
                        .as_ref()
                        .map(|vpa| Secret::new(vpa.peek().to_string())),
                },
            },
            _ => {
                return Err(errors::ConnectorError::NotSupported {
                    message: "Payment method not supported".to_string(),
                    connector: "Phonepe",
                }
                .into())
            }
        };

        // For UPI Intent, add device context with proper OS detection
        let device_context = match &router_data.request.payment_method_data {
            PaymentMethodData::Upi(UpiData::UpiIntent(_)) => {
                let device_os = match router_data
                    .request
                    .browser_info
                    .as_ref()
                    .and_then(|info| info.os_type.clone())
                    .unwrap_or_else(|| constants::DEFAULT_DEVICE_OS.to_string())
                    .to_uppercase()
                    .as_str()
                {
                    "IOS" | "IPHONE" | "IPAD" | "MACOS" | "DARWIN" => "IOS".to_string(),
                    "ANDROID" => "ANDROID".to_string(),
                    _ => "ANDROID".to_string(), // Default to ANDROID for unknown OS
                };

                Some(PhonepeDeviceContext {
                    device_os: Some(device_os),
                })
            }
            _ => None,
        };

        // Calculate payment_mode from upi_source
        let payment_mode = router_data
            .request
            .payment_method_data
            .get_upi_source()
            .map(|source| source.to_payment_mode());

        // Build payload
        let payload = PhonepePaymentRequestPayload {
            merchant_id: auth.merchant_id.clone(),
            merchant_transaction_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            merchant_user_id: router_data
                .resource_common_data
                .customer_id
                .clone()
                .map(|id| Secret::new(id.get_string_repr().to_string())),
            amount: amount_in_minor_units,
            callback_url: router_data.request.get_webhook_url()?,
            mobile_number,
            payment_instrument,
            device_context,
            payment_mode,
        };

        // Convert to JSON and encode
        let json_payload = Encode::encode_to_string_of_json(&payload)
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        // Base64 encode the payload
        let base64_payload = base64::engine::general_purpose::STANDARD.encode(&json_payload);

        // Generate checksum - use merchant-based endpoint if merchant is IRCTC
        let api_endpoint = if is_irctc_merchant(auth.merchant_id.peek()) {
            constants::API_IRCTC_PAY_ENDPOINT
        } else {
            constants::API_PAY_ENDPOINT
        };
        let api_path = format!("/{}", api_endpoint);
        let checksum =
            generate_phonepe_checksum(&base64_payload, &api_path, &auth.salt_key, &auth.key_index)?;

        Ok(Self {
            request: Secret::new(base64_payload),
            checksum,
        })
    }
}

// TryFrom implementation for borrowed PhonepeRouterData wrapper (for header generation)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &PhonepeRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PhonepePaymentsRequest
{
    type Error = Error;

    fn try_from(
        item: &PhonepeRouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = PhonepeAuthType::try_from(&router_data.connector_config)?;

        // Use amount converter to get proper amount in minor units
        let amount_in_minor_units = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        // Get customer mobile number from billing address
        let mobile_number = router_data
            .resource_common_data
            .get_optional_billing_phone_number()
            .map(|phone| Secret::new(phone.peek().to_string()));

        // Create payment instrument based on payment method data
        let payment_instrument = match &router_data.request.payment_method_data {
            PaymentMethodData::Upi(upi_data) => match upi_data {
                UpiData::UpiIntent(intent_data) => {
                    let target_app =
                        get_target_app_for_phonepe(intent_data, &router_data.request.browser_info);
                    PhonepePaymentInstrument {
                        instrument_type: constants::UPI_INTENT.to_string(),
                        target_app,
                        vpa: None,
                    }
                }
                UpiData::UpiQr(_) => PhonepePaymentInstrument {
                    instrument_type: constants::UPI_QR.to_string(),
                    target_app: None,
                    vpa: None,
                },
                UpiData::UpiCollect(collect_data) => PhonepePaymentInstrument {
                    instrument_type: constants::UPI_COLLECT.to_string(),
                    target_app: None,
                    vpa: collect_data
                        .vpa_id
                        .as_ref()
                        .map(|vpa| Secret::new(vpa.peek().to_string())),
                },
            },
            _ => {
                return Err(errors::ConnectorError::NotSupported {
                    message: "Payment method not supported".to_string(),
                    connector: "Phonepe",
                }
                .into())
            }
        };

        // For UPI Intent, add device context with proper OS detection
        let device_context = match &router_data.request.payment_method_data {
            PaymentMethodData::Upi(UpiData::UpiIntent(_)) => {
                let device_os = match router_data
                    .request
                    .browser_info
                    .as_ref()
                    .and_then(|info| info.os_type.clone())
                    .unwrap_or_else(|| constants::DEFAULT_DEVICE_OS.to_string())
                    .to_uppercase()
                    .as_str()
                {
                    "IOS" | "IPHONE" | "IPAD" | "MACOS" | "DARWIN" => "IOS".to_string(),
                    "ANDROID" => "ANDROID".to_string(),
                    _ => "ANDROID".to_string(), // Default to ANDROID for unknown OS
                };

                Some(PhonepeDeviceContext {
                    device_os: Some(device_os),
                })
            }
            _ => None,
        };

        // Calculate payment_mode from upi_source
        let payment_mode = router_data
            .request
            .payment_method_data
            .get_upi_source()
            .map(|source| source.to_payment_mode());

        // Build payload
        let payload = PhonepePaymentRequestPayload {
            merchant_id: auth.merchant_id.clone(),
            merchant_transaction_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            merchant_user_id: router_data
                .resource_common_data
                .customer_id
                .clone()
                .map(|id| Secret::new(id.get_string_repr().to_string())),
            amount: amount_in_minor_units,
            callback_url: router_data.request.get_webhook_url()?,
            mobile_number,
            payment_instrument,
            device_context,
            payment_mode,
        };

        // Convert to JSON and encode
        let json_payload = Encode::encode_to_string_of_json(&payload)
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        // Base64 encode the payload
        let base64_payload = base64::engine::general_purpose::STANDARD.encode(&json_payload);

        // Generate checksum - use merchant-based endpoint if merchant is IRCTC
        let api_endpoint = if is_irctc_merchant(auth.merchant_id.peek()) {
            constants::API_IRCTC_PAY_ENDPOINT
        } else {
            constants::API_PAY_ENDPOINT
        };
        let api_path = format!("/{}", api_endpoint);
        let checksum =
            generate_phonepe_checksum(&base64_payload, &api_path, &auth.salt_key, &auth.key_index)?;

        Ok(Self {
            request: Secret::new(base64_payload),
            checksum,
        })
    }
}

// ===== RESPONSE HANDLING =====

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PhonepePaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Error;

    fn try_from(
        item: ResponseRouterData<PhonepePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        if response.success {
            if let Some(data) = &response.data {
                if let Some(instrument_response) = &data.instrument_response {
                    // Handle different UPI flow responses
                    let (redirect_form, connector_metadata) =
                        match instrument_response.instrument_type.as_str() {
                            instrument_type if instrument_type == constants::UPI_INTENT => {
                                let redirect_form = instrument_response
                                    .intent_url
                                    .as_ref()
                                    .map(|url| RedirectForm::Uri { uri: url.clone() });
                                (redirect_form, None)
                            }
                            instrument_type if instrument_type == constants::UPI_QR => {
                                let redirect_form = instrument_response
                                    .intent_url
                                    .as_ref()
                                    .map(|url| RedirectForm::Uri { uri: url.clone() });

                                let connector_metadata =
                                    instrument_response.qr_data.as_ref().map(|qr| {
                                        serde_json::json!({
                                            "qr_data": qr
                                        })
                                    });
                                (redirect_form, connector_metadata)
                            }
                            _ => (None, None),
                        };

                    Ok(Self {
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: match &data.transaction_id {
                                Some(txn_id) => ResponseId::ConnectorTransactionId(txn_id.clone()),
                                None => ResponseId::NoResponseId,
                            },
                            redirection_data: redirect_form.map(Box::new),
                            mandate_reference: None,
                            connector_metadata,
                            network_txn_id: None,
                            connector_response_reference_id: Some(
                                data.merchant_transaction_id.clone(),
                            ),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        }),
                        resource_common_data: PaymentFlowData {
                            status: common_enums::AttemptStatus::AuthenticationPending,
                            ..item.router_data.resource_common_data
                        },
                        ..item.router_data
                    })
                } else {
                    // Success but no instrument response
                    Ok(Self {
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: match &data.transaction_id {
                                Some(txn_id) => ResponseId::ConnectorTransactionId(txn_id.clone()),
                                None => ResponseId::NoResponseId,
                            },
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: get_wait_screen_metadata(),
                            network_txn_id: None,
                            connector_response_reference_id: Some(
                                data.merchant_transaction_id.clone(),
                            ),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        }),
                        resource_common_data: PaymentFlowData {
                            ..item.router_data.resource_common_data
                        },
                        ..item.router_data
                    })
                }
            } else {
                Err(errors::ConnectorError::ResponseDeserializationFailed.into())
            }
        } else {
            // Error response - PhonePe returned success: false
            let error_message = response.message.clone();
            let error_code = response.code.clone();

            tracing::warn!(
                "PhonePe payment failed - Code: {}, Message: {}, Status: {}",
                error_code,
                error_message,
                item.http_code
            );

            // Get merchant transaction ID from data if available for better tracking
            let connector_transaction_id = response
                .data
                .as_ref()
                .map(|data| data.merchant_transaction_id.clone());

            // Map specific PhonePe error codes to attempt status if needed
            let attempt_status = match error_code.as_str() {
                "INVALID_TRANSACTION_ID"
                | "TRANSACTION_NOT_FOUND"
                | "INVALID_REQUEST"
                | "PAYMENT_DECLINED" => Some(common_enums::AttemptStatus::Failure),
                "INTERNAL_SERVER_ERROR" | "PAYMENT_PENDING" => {
                    Some(common_enums::AttemptStatus::Pending)
                }
                _ => Some(common_enums::AttemptStatus::Pending),
            };

            tracing::warn!(
                "PhonePe payment failed - Code: {}, Message: {}, Status: {}",
                error_code,
                error_message,
                item.http_code
            );

            Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status,
                    connector_transaction_id,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ===== AUTHENTICATION =====

#[derive(Debug)]
pub struct PhonepeAuthType {
    pub merchant_id: Secret<String>,
    pub salt_key: Secret<String>,
    pub key_index: String,
}

impl TryFrom<&ConnectorSpecificConfig> for PhonepeAuthType {
    type Error = Error;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Phonepe {
                merchant_id,
                salt_key,
                salt_index,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.clone(),
                salt_key: salt_key.clone(),
                key_index: salt_index.peek().clone(),
            }),
            _ => Err(errors::ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

// ===== HELPER FUNCTIONS =====

// Check if merchant ID corresponds to IRCTC (merchant-based endpoints)
// This should be called with the merchant_id from X-MERCHANT-ID auth header
pub fn is_irctc_merchant(merchant_id: &str) -> bool {
    merchant_id.contains(constants::IRCTC_IDENTIFIER)
}

fn generate_phonepe_checksum(
    base64_payload: &str,
    api_path: &str,
    salt_key: &Secret<String>,
    key_index: &str,
) -> Result<String, Error> {
    // PhonePe checksum algorithm: SHA256(base64Payload + apiPath + saltKey) + "###" + keyIndex
    let checksum_input = format!("{}{}{}", base64_payload, api_path, salt_key.peek());

    let sha256 = crypto::Sha256;
    let hash_bytes = sha256
        .generate_digest(checksum_input.as_bytes())
        .change_context(errors::ConnectorError::RequestEncodingFailed)?;
    let hash = hash_bytes.iter().fold(String::new(), |mut acc, byte| {
        use std::fmt::Write;
        let _ = write!(&mut acc, "{byte:02x}");
        acc
    });

    // Format: hash###keyIndex
    Ok(format!(
        "{}{}{}",
        hash,
        constants::CHECKSUM_SEPARATOR,
        key_index
    ))
}

// ===== SYNC REQUEST BUILDING =====

// TryFrom implementation for owned PhonepeRouterData wrapper (sync)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PhonepeRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for PhonepeSyncRequest
{
    type Error = Error;

    fn try_from(
        wrapper: PhonepeRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = PhonepeAuthType::try_from(&router_data.connector_config)?;

        let merchant_transaction_id = router_data.resource_common_data.get_reference_id()?;

        // Generate checksum for status API - use IRCTC endpoint if merchant is IRCTC
        let api_endpoint = if is_irctc_merchant(auth.merchant_id.peek()) {
            constants::API_IRCTC_STATUS_ENDPOINT
        } else {
            constants::API_STATUS_ENDPOINT
        };
        let api_path = format!(
            "/{}/{}/{}",
            api_endpoint,
            auth.merchant_id.peek(),
            merchant_transaction_id
        );
        let checksum = generate_phonepe_sync_checksum(&api_path, &auth.salt_key, &auth.key_index)?;

        Ok(Self {
            merchant_transaction_id: merchant_transaction_id.clone(),
            checksum,
        })
    }
}

// TryFrom implementation for borrowed PhonepeRouterData wrapper (sync header generation)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &PhonepeRouterData<
            &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for PhonepeSyncRequest
{
    type Error = Error;

    fn try_from(
        item: &PhonepeRouterData<
            &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = PhonepeAuthType::try_from(&router_data.connector_config)?;

        let merchant_transaction_id = router_data.resource_common_data.get_reference_id()?;

        // Generate checksum for status API - use IRCTC endpoint if merchant is IRCTC
        let api_endpoint = if is_irctc_merchant(auth.merchant_id.peek()) {
            constants::API_IRCTC_STATUS_ENDPOINT
        } else {
            constants::API_STATUS_ENDPOINT
        };
        let api_path = format!(
            "/{}/{}/{}",
            api_endpoint,
            auth.merchant_id.peek(),
            merchant_transaction_id
        );
        let checksum = generate_phonepe_sync_checksum(&api_path, &auth.salt_key, &auth.key_index)?;

        Ok(Self {
            merchant_transaction_id: merchant_transaction_id.clone(),
            checksum,
        })
    }
}

// ===== SYNC RESPONSE HANDLING =====

impl TryFrom<ResponseRouterData<PhonepeSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Error;

    fn try_from(item: ResponseRouterData<PhonepeSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        if response.success {
            if let Some(data) = &response.data {
                // Check if we have required fields for a successful transaction
                if let (Some(merchant_transaction_id), Some(transaction_id)) =
                    (&data.merchant_transaction_id, &data.transaction_id)
                {
                    // Only extract UPI mode and BIN for UPI payment methods
                    let (upi_mode, bin) = match &item.router_data.request.payment_method_type {
                        Some(
                            common_enums::PaymentMethodType::UpiCollect
                            | common_enums::PaymentMethodType::UpiIntent
                            | common_enums::PaymentMethodType::UpiQr,
                        ) => {
                            let upi_mode = extract_upi_mode_from_sync_data(data);
                            let bin = data.payment_instrument.as_ref().and_then(|payment_inst| {
                                extract_bin_from_masked_account_number(
                                    payment_inst.masked_account_number.as_deref(),
                                )
                            });
                            (upi_mode, bin)
                        }
                        _ => (None, None),
                    };

                    // Map PhonePe response codes to payment statuses based on documentation
                    let status = match response.code.as_str() {
                        "PAYMENT_SUCCESS" => common_enums::AttemptStatus::Charged,
                        "PAYMENT_PENDING" | "TIMED_OUT" | "INTERNAL_SERVER_ERROR" => {
                            common_enums::AttemptStatus::Pending
                        }
                        "PAYMENT_ERROR"
                        | "PAYMENT_DECLINED"
                        | "BAD_REQUEST"
                        | "AUTHORIZATION_FAILED"
                        | "TRANSACTION_NOT_FOUND" => common_enums::AttemptStatus::Failure,
                        _ => common_enums::AttemptStatus::Pending, // Default to pending for unknown codes
                    };

                    Ok(Self {
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: get_sync_metadata(bin),
                            network_txn_id: None,
                            connector_response_reference_id: Some(merchant_transaction_id.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        }),
                        resource_common_data: PaymentFlowData {
                            status,
                            connector_response: get_connector_response_with_upi_mode(upi_mode),
                            ..item.router_data.resource_common_data
                        },
                        ..item.router_data
                    })
                } else {
                    // Data object exists but missing required fields - treat as error
                    Ok(Self {
                        response: Err(domain_types::router_data::ErrorResponse {
                            code: response.code.clone(),
                            message: response.message.clone(),
                            reason: None,
                            status_code: item.http_code,
                            attempt_status: Some(common_enums::AttemptStatus::Failure),
                            connector_transaction_id: data.transaction_id.clone(),
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                        }),
                        ..item.router_data
                    })
                }
            } else {
                Err(errors::ConnectorError::ResponseDeserializationFailed.into())
            }
        } else {
            // Error response from sync API - handle specific PhonePe error codes
            let error_message = response.message.clone();
            let error_code = response.code.clone();

            // Map PhonePe error codes to attempt status
            let attempt_status = get_phonepe_error_status(&error_code);

            Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message,
                    reason: None,
                    status_code: item.http_code,
                    attempt_status,
                    connector_transaction_id: response
                        .data
                        .as_ref()
                        .and_then(|data| data.transaction_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

fn generate_phonepe_sync_checksum(
    api_path: &str,
    salt_key: &Secret<String>,
    key_index: &str,
) -> Result<String, Error> {
    // PhonePe sync checksum algorithm: SHA256(apiPath + saltKey) + "###" + keyIndex
    let checksum_input = format!("{}{}", api_path, salt_key.peek());

    let sha256 = crypto::Sha256;
    let hash_bytes = sha256
        .generate_digest(checksum_input.as_bytes())
        .change_context(errors::ConnectorError::RequestEncodingFailed)?;
    let hash = hash_bytes.iter().fold(String::new(), |mut acc, byte| {
        use std::fmt::Write;
        let _ = write!(&mut acc, "{byte:02x}");
        acc
    });

    // Format: hash###keyIndex
    Ok(format!(
        "{}{}{}",
        hash,
        constants::CHECKSUM_SEPARATOR,
        key_index
    ))
}

pub fn get_phonepe_error_status(error_code: &str) -> Option<common_enums::AttemptStatus> {
    match error_code {
        "TRANSACTION_NOT_FOUND" => Some(common_enums::AttemptStatus::Failure),
        "401" => Some(common_enums::AttemptStatus::AuthenticationFailed),
        "400" | "BAD_REQUEST" => Some(common_enums::AttemptStatus::Failure),
        "PAYMENT_ERROR" | "PAYMENT_DECLINED" | "TIMED_OUT" => {
            Some(common_enums::AttemptStatus::Failure)
        }
        "AUTHORIZATION_FAILED" => Some(common_enums::AttemptStatus::AuthenticationFailed),
        _ => None,
    }
}

pub fn get_wait_screen_metadata() -> Option<serde_json::Value> {
    serde_json::to_value(serde_json::json!({
        NEXT_ACTION_DATA: NextActionData::WaitScreenInstructions
    }))
    .map_err(|e| {
        tracing::error!("Failed to serialize wait screen metadata: {}", e);
        e
    })
    .ok()
}

// ===== TARGET APP MAPPING FOR PHONEPE UPI INTENT =====

/// Gets the target app for PhonePe UPI Intent based on OS and payment source
fn get_target_app_for_phonepe(
    intent_data: &domain_types::payment_method_data::UpiIntentData,
    browser_info: &Option<BrowserInformation>,
) -> Option<String> {
    match get_mobile_os(browser_info).as_str() {
        "ANDROID" => intent_data.app_name.clone(),
        _ => map_ios_payment_source_to_target_app(intent_data.app_name.as_deref()),
    }
}

/// Detects the device OS from browser_info
fn get_mobile_os(browser_info: &Option<BrowserInformation>) -> String {
    browser_info
        .as_ref()
        .and_then(|info| info.os_type.as_ref())
        .map(|os| match os.to_uppercase().as_str() {
            "IOS" | "IPHONE" | "IPAD" | "MACOS" | "DARWIN" => "IOS".to_string(),
            "ANDROID" => "ANDROID".to_string(),
            _ => "ANDROID".to_string(),
        })
        .unwrap_or("ANDROID".to_string())
}

/// Maps iOS payment source to PhonePe's expected target app names
pub fn map_ios_payment_source_to_target_app(payment_source: Option<&str>) -> Option<String> {
    payment_source.and_then(|source| {
        let source_lower = source.to_lowercase();
        match source_lower.as_str() {
            s if s.contains("tez") => Some("GPAY".to_string()),
            s if s.contains("phonepe") => Some("PHONEPE".to_string()),
            s if s.contains("paytm") => Some("PAYTM".to_string()),
            _ => None,
        }
    })
}

/// Extract Android version from user agent string
pub fn get_android_version_from_ua(user_agent: &str) -> String {
    user_agent
        .split_whitespace()
        .skip_while(|&part| part != "Android")
        .nth(1)
        .and_then(|version| version.strip_suffix(';'))
        .unwrap_or("")
        .to_string()
}

pub fn get_source_channel(user_agent: Option<&String>) -> String {
    match user_agent.map(|s| s.to_lowercase()) {
        Some(ua) if ua.contains("android") => "ANDROID".to_string(),
        Some(ua) if ua.contains("iphone") || ua.contains("darwin") => "IOS".to_string(),
        _ => "WEB".to_string(),
    }
}

/// Creates metadata for sync response with BIN from masked account number
fn get_sync_metadata(bin: Option<String>) -> Option<serde_json::Value> {
    bin.and_then(|b| {
        serde_json::to_value(serde_json::json!({
            "card_bin_number": b
        }))
        .map_err(|e| {
            tracing::error!("Failed to serialize sync metadata: {}", e);
            e
        })
        .ok()
    })
}

fn determine_upi_mode(
    payment_instrument: &PhonepePaymentInstrumentSync,
    response_code: Option<&String>,
) -> Option<UpiSource> {
    match (
        payment_instrument.upi_credit_line,
        payment_instrument.account_type.as_deref(),
        payment_instrument.card_network.as_deref(),
        response_code.map(|s| s.as_str()),
    ) {
        (Some(true), _, _, _) => Some(UpiSource::UpiCl),
        (_, Some(constants::ACCOUNT_TYPE_CREDIT), Some(constants::CARD_NETWORK_RUPAY), _) => {
            Some(UpiSource::UpiCc)
        }
        (_, Some(constants::ACCOUNT_TYPE_SAVINGS), _, _) => Some(UpiSource::UpiAccount),
        (_, _, _, Some(constants::RESPONSE_CODE_CREDIT_ACCOUNT_NOT_ALLOWED)) => {
            Some(UpiSource::UpiCc)
        }
        (_, _, _, Some(constants::RESPONSE_CODE_PAY0071)) => Some(UpiSource::UpiCl),
        _ => None,
    }
}

/// Extracts UPI mode from sync response payment instrument
fn extract_upi_mode_from_sync_data(sync_data: &PhonepeSyncResponseData) -> Option<UpiSource> {
    // Try to determine from payment_instrument
    sync_data
        .payment_instrument
        .as_ref()
        .and_then(|payment_instrument| {
            determine_upi_mode(payment_instrument, sync_data.response_code.as_ref())
        })
        // Fallback: determine from response_code alone
        .or_else(|| {
            sync_data
                .response_code
                .as_deref()
                .and_then(|code| match code {
                    constants::RESPONSE_CODE_CREDIT_ACCOUNT_NOT_ALLOWED => Some(UpiSource::UpiCc),
                    constants::RESPONSE_CODE_PAY0071 => Some(UpiSource::UpiCl),
                    _ => None,
                })
        })
}

/// Creates ConnectorResponseData with UPI mode
fn get_connector_response_with_upi_mode(
    upi_mode: Option<UpiSource>,
) -> Option<domain_types::router_data::ConnectorResponseData> {
    upi_mode.map(|mode| {
        domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Upi {
                upi_mode: Some(mode),
            },
        )
    })
}

/// Extracts the BIN (first 6 numeric characters) from a masked account number.
fn extract_bin_from_masked_account_number(masked_account_number: Option<&str>) -> Option<String> {
    masked_account_number.and_then(|account_number| {
        account_number
            .get(..6)
            .filter(|bin| bin.chars().all(|c| c.is_ascii_digit()))
            .map(str::to_string)
    })
}
