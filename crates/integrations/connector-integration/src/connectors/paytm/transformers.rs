use std::{
    cmp,
    time::{SystemTime, UNIX_EPOCH},
};

use aes::{Aes128, Aes192, Aes256};
use base64::{engine::general_purpose, Engine};
use cbc::{
    cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit},
    Encryptor,
};
use common_enums::AttemptStatus;
use common_utils::{errors::CustomResult, request::Method};
use domain_types::{
    connector_flow::{Authorize, PSync, RepeatPayment, ServerSessionAuthenticationToken},
    connector_types::{
        MandateReferenceId, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RepeatPaymentData, ResponseId,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
    },
    errors::{ConnectorResponseTransformationError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, UpiData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::BrowserInformation,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde_json;
use url::Url;

use crate::{
    connectors::paytm::PaytmRouterData as MacroPaytmRouterData, types::ResponseRouterData,
};
use serde::{Deserialize, Serialize};

pub use super::request::{
    PaytmAmount, PaytmAuthorizeRequest, PaytmEnableMethod, PaytmExtendInfo, PaytmGoodsInfo,
    PaytmInitiateReqBody, PaytmInitiateTxnRequest, PaytmNativeProcessRequestBody,
    PaytmNativeProcessTxnRequest, PaytmProcessBodyTypes, PaytmProcessHeadTypes,
    PaytmProcessTxnRequest, PaytmRepeatPaymentReqBody, PaytmRepeatPaymentRequest,
    PaytmRequestHeader, PaytmShippingInfo, PaytmTransactionStatusReqBody,
    PaytmTransactionStatusRequest, PaytmTxnTokenType, PaytmUserInfo,
};
pub use super::response::{
    PaytmBankForm, PaytmBankFormBody, PaytmBankFormResponse, PaytmCallbackErrorBody,
    PaytmCallbackErrorResponse, PaytmDeepLinkInfo, PaytmErrorBody, PaytmErrorResponse,
    PaytmInitiateTxnResponse, PaytmNativeProcessFailureResp, PaytmNativeProcessRespBodyTypes,
    PaytmNativeProcessSuccessResp, PaytmNativeProcessTxnResponse, PaytmProcessFailureResp,
    PaytmProcessHead, PaytmProcessRespBodyTypes, PaytmProcessSuccessResp, PaytmProcessTxnResponse,
    PaytmRepeatPaymentRespBodyTypes, PaytmRepeatPaymentResponse, PaytmRepeatPaymentSuccessResp,
    PaytmResBodyTypes, PaytmRespBody, PaytmRespHead, PaytmResultInfo, PaytmSessionTokenErrorBody,
    PaytmSessionTokenErrorResponse, PaytmSuccessTransactionBody, PaytmSuccessTransactionResponse,
    PaytmTransactionStatusRespBody, PaytmTransactionStatusRespBodyTypes,
    PaytmTransactionStatusResponse, PaytmTxnInfo,
};

// PayTM API Constants
pub mod constants {
    // PayTM API versions and identifiers
    pub const API_VERSION: &str = "v1";

    // Request types
    pub const REQUEST_TYPE_PAYMENT: &str = "Payment";
    pub const REQUEST_TYPE_NATIVE: &str = "NATIVE";

    // UPI specific constants
    pub const PAYMENT_MODE_UPI: &str = "UPI";
    pub const UPI_CHANNEL_UPIPUSH: &str = "UPIPUSH";
    pub const PAYMENT_FLOW_NONE: &str = "NONE";

    // Default values
    pub const DEFAULT_CALLBACK_URL: &str = "https://default-callback.com";

    // Error messages
    pub const ERROR_INVALID_VPA: &str = "Invalid UPI VPA format";
    pub const ERROR_SALT_GENERATION: &str = "Failed to generate random salt";
    pub const ERROR_AES_128_ENCRYPTION: &str = "AES-128 encryption failed";
    pub const ERROR_AES_192_ENCRYPTION: &str = "AES-192 encryption failed";
    pub const ERROR_AES_256_ENCRYPTION: &str = "AES-256 encryption failed";

    // HTTP constants
    pub const CONTENT_TYPE_JSON: &str = "application/json";
    pub const CONTENT_TYPE_HEADER: &str = "Content-Type";

    // Channel IDs
    pub const CHANNEL_ID_WAP: &str = "WAP";
    pub const CHANNEL_ID_WEB: &str = "WEB";

    // AES encryption constants (from PayTM Haskell implementation)
    pub const PAYTM_IV: &[u8; 16] = b"@@@@&&&&####$$$$";
    pub const SALT_LENGTH: usize = 3;
    pub const AES_BUFFER_PADDING: usize = 16;
    pub const AES_128_KEY_LENGTH: usize = 16;
    pub const AES_192_KEY_LENGTH: usize = 24;
    pub const AES_256_KEY_LENGTH: usize = 32;
}

pub const NEXT_ACTION_DATA: &str = "nextActionData";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextActionData {
    WaitScreenInstructions,
}

#[derive(Debug, Clone)]
pub struct PaytmAuthType {
    pub merchant_id: Secret<String>,       // From api_key
    pub merchant_key: Secret<String>,      // From key1
    pub website: Secret<String>,           // From api_secret
    pub client_id: Option<Secret<String>>, // Unique key for each merchant
}

impl TryFrom<&ConnectorSpecificConfig> for PaytmAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paytm {
                merchant_id,
                merchant_key,
                website,
                client_id,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                merchant_key: merchant_key.to_owned(),
                website: website.to_owned(),
                client_id: client_id.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum UpiFlowType {
    Intent,
    Collect,
}

// ================================
// Session Token Flow
// ================================

// PaytmInitiateTxnRequest TryFrom ServerSessionAuthenticationToken RouterData
// Using the macro-generated PaytmRouterData type from the paytm module
impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + Serialize,
    >
    TryFrom<
        MacroPaytmRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for PaytmInitiateTxnRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MacroPaytmRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PaytmAuthType::try_from(&item.router_data.connector_config)?;

        // Extract data directly from router_data
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let paytm_amount = PaytmAmount {
            value: amount,
            currency: item.router_data.request.currency,
        };
        let user_info = PaytmUserInfo {
            cust_id: item
                .router_data
                .resource_common_data
                .get_customer_id()
                .unwrap_or_default(),
            mobile: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            first_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_first_name(),
            last_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_last_name(),
        };
        let return_url = item.router_data.resource_common_data.get_return_url();

        let order_details = item.router_data.resource_common_data.order_details.clone();
        let goods = match order_details.as_ref().and_then(|details| details.first()) {
            Some(details) => {
                // Convert order detail amount using amount converter
                let order_amount = item
                    .connector
                    .amount_converter
                    .convert(details.amount, item.router_data.request.currency)
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: Default::default(),
                    })?;

                Some(PaytmGoodsInfo {
                    merchant_goods_id: details.product_id.clone(),
                    merchant_shipping_id: None,
                    snapshot_url: details.product_img_link.clone(),
                    description: details
                        .description
                        .clone()
                        .unwrap_or_else(|| details.product_name.clone()),
                    category: details.category.clone(),
                    quantity: details.quantity.into(),
                    unit: details.unit_of_measure.clone(),
                    price: PaytmAmount {
                        value: order_amount,
                        currency: item.router_data.request.currency,
                    },
                    extend_info: None,
                })
            }
            None => None,
        };

        let shipping_info = PaytmShippingInfo {
            merchant_shipping_id: None,
            tracking_no: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            carrier: None,
            charge_amount: Some(paytm_amount.clone()),
            country_name: item
                .router_data
                .resource_common_data
                .get_optional_shipping_country(),
            state_name: item
                .router_data
                .resource_common_data
                .get_optional_shipping_state(),
            city_name: item
                .router_data
                .resource_common_data
                .get_optional_shipping_city(),
            address1: item
                .router_data
                .resource_common_data
                .get_optional_shipping_line1(),
            address2: item
                .router_data
                .resource_common_data
                .get_optional_shipping_line2(),
            first_name: item
                .router_data
                .resource_common_data
                .get_optional_shipping_first_name(),
            last_name: item
                .router_data
                .resource_common_data
                .get_optional_shipping_last_name(),
            mobile_no: item
                .router_data
                .resource_common_data
                .get_optional_shipping_phone_number(),
            zip_code: item
                .router_data
                .resource_common_data
                .get_optional_shipping_zip(),
            email: item
                .router_data
                .resource_common_data
                .get_optional_shipping_email(),
        };

        let body = PaytmInitiateReqBody {
            request_type: constants::REQUEST_TYPE_PAYMENT.to_string(),
            mid: auth.merchant_id.clone(),
            order_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            website_name: Secret::new(auth.website.peek().to_string()),
            txn_amount: paytm_amount,
            user_info,
            enable_payment_mode: vec![PaytmEnableMethod {
                mode: constants::PAYMENT_MODE_UPI.to_string(),
                channels: Some(vec![
                    constants::UPI_CHANNEL_UPIPUSH.to_string(), // UPI_INTENT
                    constants::PAYMENT_MODE_UPI.to_string(),    // UPI_COLLECT
                ]),
            }],
            callback_url: return_url.unwrap_or_else(|| constants::DEFAULT_CALLBACK_URL.to_string()),
            goods,
            shipping_info: Some(vec![shipping_info]),
            extend_info: None, // from metadata
        };

        // Create header with actual signature
        let channel_id =
            get_channel_id_from_browser_info(item.router_data.request.browser_info.as_ref());
        let head = create_paytm_header(&body, &auth, channel_id.as_deref())?;

        Ok(Self { head, body })
    }
}

// ServerSessionAuthenticationToken response transformation
impl TryFrom<ResponseRouterData<PaytmInitiateTxnResponse, Self>>
    for RouterDataV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<PaytmInitiateTxnResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let mut router_data = item.router_data;

        // Handle both success and failure cases from the enum body
        router_data.response = match &response.body {
            PaytmResBodyTypes::SuccessBody(success_body) => {
                // Check for idempotent/duplicate case (0002) which should be treated as error
                if success_body.result_info.result_code == "0002" {
                    Err(domain_types::router_data::ErrorResponse {
                        code: success_body.result_info.result_code.clone(),
                        message: success_body.result_info.result_msg.clone(),
                        reason: Some(success_body.result_info.result_msg.clone()),
                        status_code: item.http_code,
                        attempt_status: None, // Duplicate Request.
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    })
                } else {
                    Ok(ServerSessionAuthenticationTokenResponseData {
                        session_token: success_body.txn_token.clone().expose(),
                    })
                }
            }
            PaytmResBodyTypes::FailureBody(failure_body) => {
                Err(domain_types::router_data::ErrorResponse {
                    code: failure_body.result_info.result_code.clone(),
                    message: failure_body.result_info.result_msg.clone(),
                    reason: Some(failure_body.result_info.result_msg.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
        };

        Ok(router_data)
    }
}

// ================================
// Authorization Flow
// ================================

// PaytmAuthorizeRequest TryFrom Authorize RouterData
impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + Serialize,
    >
    TryFrom<
        MacroPaytmRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaytmAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MacroPaytmRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PaytmAuthType::try_from(&item.router_data.connector_config)?;

        let payment_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let session_token = item.router_data.resource_common_data.get_session_token()?;
        let payment_method_data = &item.router_data.request.payment_method_data;

        // Determine the UPI flow type based on payment method data
        let upi_flow = determine_upi_flow(payment_method_data)?;

        match upi_flow {
            UpiFlowType::Intent => {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| IntegrationError::InvalidDataFormat {
                        field_name: "timestamp",
                        context: Default::default(),
                    })?
                    .as_secs()
                    .to_string();

                let channel_id = get_channel_id_from_browser_info(
                    item.router_data.request.browser_info.as_ref(),
                );
                let head = PaytmProcessHeadTypes {
                    version: constants::API_VERSION.to_string(),
                    request_timestamp: timestamp,
                    channel_id,
                    txn_token: Secret::new(session_token),
                };

                let body = PaytmProcessBodyTypes {
                    mid: auth.merchant_id.clone(),
                    order_id: payment_id,
                    request_type: constants::REQUEST_TYPE_NATIVE.to_string(),
                    payment_mode: format!("{}_{}", constants::PAYMENT_MODE_UPI, "INTENT"),
                    payment_flow: Some(constants::PAYMENT_FLOW_NONE.to_string()),
                    txn_note: item.router_data.resource_common_data.description.clone(),
                    extend_info: None,
                };

                let intent_request = PaytmProcessTxnRequest { head, body };
                Ok(Self::Intent(intent_request))
            }
            UpiFlowType::Collect => {
                let vpa = match extract_upi_vpa(payment_method_data)? {
                    Some(vpa) => vpa,
                    None => {
                        return Err(IntegrationError::MissingRequiredField {
                            field_name: "vpa_id",
                            context: Default::default(),
                        }
                        .into())
                    }
                };

                let head = PaytmTxnTokenType {
                    txn_token: Secret::new(session_token.clone()),
                };

                let channel_id = get_channel_id_from_browser_info(
                    item.router_data.request.browser_info.as_ref(),
                );
                let body = PaytmNativeProcessRequestBody {
                    request_type: constants::REQUEST_TYPE_NATIVE.to_string(),
                    mid: auth.merchant_id.clone(),
                    order_id: payment_id,
                    payment_mode: constants::PAYMENT_MODE_UPI.to_string(),
                    payer_account: Some(vpa),
                    channel_code: Some("".to_string()), //BankCode (only in NET_BANKING)
                    channel_id: channel_id.unwrap_or_else(|| constants::CHANNEL_ID_WEB.to_string()),
                    txn_token: Secret::new(session_token),
                    auth_mode: None, //authentication mode if any
                };

                let collect_request = PaytmNativeProcessTxnRequest { head, body };
                Ok(Self::Collect(collect_request))
            }
        }
    }
}

// Authorize response transformation
impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + Serialize,
    > TryFrom<ResponseRouterData<PaytmProcessTxnResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<PaytmProcessTxnResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let mut router_data = item.router_data;

        // Handle both success and failure cases from the enum body
        let (redirection_data, connector_ref_id, connector_txn_id) = match &response.body {
            PaytmProcessRespBodyTypes::SuccessBody(success_body) => {
                // Extract redirection URL if present
                let redirection_data = if let Some(deep_link_info) = &success_body.deep_link_info {
                    if !deep_link_info.deep_link.is_empty() {
                        // Check if it's a UPI deep link (starts with upi://) or regular URL
                        if deep_link_info.deep_link.starts_with("upi://") {
                            // For UPI deep links, use them as-is
                            Some(Box::new(RedirectForm::Uri {
                                uri: deep_link_info.deep_link.clone(),
                            }))
                        } else {
                            // For regular URLs, parse and convert
                            let url = Url::parse(&deep_link_info.deep_link).change_context(
                                crate::utils::response_handling_fail_for_connector(
                                    item.http_code,
                                    "paytm",
                                ),
                            )?;
                            Some(Box::new(RedirectForm::from((url, Method::Get))))
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Extract transaction IDs from deep_link_info or use fallback
                let (connector_ref_id, connector_txn_id) =
                    if let Some(deep_link_info) = &success_body.deep_link_info {
                        let connector_txn_id =
                            ResponseId::ConnectorTransactionId(deep_link_info.trans_id.clone());
                        let connector_ref_id = Some(deep_link_info.order_id.clone());
                        (connector_ref_id, connector_txn_id)
                    } else {
                        // Fallback when deep_link_info is not present
                        let connector_ref_id = Some(
                            router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                        );
                        (connector_ref_id, ResponseId::NoResponseId)
                    };

                (redirection_data, connector_ref_id, connector_txn_id)
            }
            PaytmProcessRespBodyTypes::FailureBody(_failure_body) => {
                let connector_ref_id = Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                );
                (None, connector_ref_id, ResponseId::NoResponseId)
            }
        };
        // Get result code for status mapping
        let result_code = match &response.body {
            PaytmProcessRespBodyTypes::SuccessBody(success_body) => {
                &success_body.result_info.result_code
            }
            PaytmProcessRespBodyTypes::FailureBody(failure_body) => {
                &failure_body.result_info.result_code
            }
        };

        // Map status using the result code
        let attempt_status = map_paytm_authorize_status_to_attempt_status(result_code);
        router_data.resource_common_data.set_status(attempt_status);

        let connector_metadata = get_wait_screen_metadata();

        router_data.response = if is_failure_status(attempt_status) {
            Err(domain_types::router_data::ErrorResponse {
                code: result_code.clone(),
                message: match &response.body {
                    PaytmProcessRespBodyTypes::SuccessBody(body) => {
                        body.result_info.result_msg.clone()
                    }
                    PaytmProcessRespBodyTypes::FailureBody(body) => {
                        body.result_info.result_msg.clone()
                    }
                },
                reason: match &response.body {
                    PaytmProcessRespBodyTypes::SuccessBody(body) => {
                        Some(body.result_info.result_msg.clone())
                    }
                    PaytmProcessRespBodyTypes::FailureBody(body) => {
                        Some(body.result_info.result_msg.clone())
                    }
                },
                status_code: item.http_code,
                attempt_status: Some(attempt_status),
                connector_transaction_id: connector_ref_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: connector_txn_id,
                redirection_data,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: connector_ref_id,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(router_data)
    }
}

// ================================
// Payment Sync Flow
// ================================

// PaytmTransactionStatusRequest TryFrom PSync RouterData
impl<
        T: domain_types::payment_method_data::PaymentMethodDataTypes
            + std::fmt::Debug
            + Sync
            + Send
            + 'static
            + Serialize,
    >
    TryFrom<
        MacroPaytmRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for PaytmTransactionStatusRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MacroPaytmRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PaytmAuthType::try_from(&item.router_data.connector_config)?;

        // Extract data directly from router_data
        let order_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let body = PaytmTransactionStatusReqBody {
            mid: auth.merchant_id.clone(),
            order_id,
            txn_type: None, // Can be enhanced later to support specific transaction types
        };

        // Create header with actual signature
        let head = create_paytm_header(&body, &auth, None)?;

        Ok(Self { head, body })
    }
}

// PSync response transformation
impl TryFrom<ResponseRouterData<PaytmTransactionStatusResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<PaytmTransactionStatusResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let mut router_data = item.router_data;

        // Handle both success and failure cases from the enum body
        let (connector_ref_id, connector_txn_id) = match &response.body {
            PaytmTransactionStatusRespBodyTypes::SuccessBody(success_body) => {
                let connector_ref_id = Some(success_body.order_id.clone());
                let connector_txn_id =
                    ResponseId::ConnectorTransactionId(success_body.txn_id.clone());
                (connector_ref_id, connector_txn_id)
            }
            PaytmTransactionStatusRespBodyTypes::FailureBody(_failure_body) => {
                let connector_ref_id = Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                );
                (connector_ref_id, ResponseId::NoResponseId)
            }
        };

        // Get result code for status mapping
        let result_code = match &response.body {
            PaytmTransactionStatusRespBodyTypes::SuccessBody(success_body) => {
                &success_body.result_info.result_code
            }
            PaytmTransactionStatusRespBodyTypes::FailureBody(failure_body) => {
                &failure_body.result_info.result_code
            }
        };

        // Map status and set response accordingly
        let attempt_status = map_paytm_sync_status_to_attempt_status(result_code);

        // Update the status using the new setter function
        router_data.resource_common_data.set_status(attempt_status);

        router_data.response = if is_failure_status(attempt_status) {
            Err(domain_types::router_data::ErrorResponse {
                code: result_code.clone(),
                message: match &response.body {
                    PaytmTransactionStatusRespBodyTypes::SuccessBody(body) => {
                        body.result_info.result_msg.clone()
                    }
                    PaytmTransactionStatusRespBodyTypes::FailureBody(body) => {
                        body.result_info.result_msg.clone()
                    }
                },
                reason: Some(match &response.body {
                    PaytmTransactionStatusRespBodyTypes::SuccessBody(body) => {
                        body.result_info.result_status.clone()
                    }
                    PaytmTransactionStatusRespBodyTypes::FailureBody(body) => {
                        body.result_info.result_status.clone()
                    }
                }),
                status_code: item.http_code,
                attempt_status: Some(attempt_status),
                connector_transaction_id: connector_ref_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            let connector_metadata = get_wait_screen_metadata();
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: connector_txn_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: connector_ref_id,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(router_data)
    }
}

pub fn determine_upi_flow<T: domain_types::payment_method_data::PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> CustomResult<UpiFlowType, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::Upi(upi_data) => {
            match upi_data {
                UpiData::UpiCollect(collect_data) => {
                    // If VPA is provided, it's a collect flow
                    if collect_data.vpa_id.is_some() {
                        Ok(UpiFlowType::Collect)
                    } else {
                        Err(IntegrationError::MissingRequiredField {
                            field_name: "vpa_id",
                            context: Default::default(),
                        }
                        .into())
                    }
                }
                UpiData::UpiIntent(_) | UpiData::UpiQr(_) => Ok(UpiFlowType::Intent),
            }
        }
        _ => Err(IntegrationError::NotSupported {
            message: "Only UPI payment methods are supported".to_string(),
            connector: "Paytm",
            context: Default::default(),
        }
        .into()),
    }
}

// Helper function for UPI VPA extraction
pub fn extract_upi_vpa<T: domain_types::payment_method_data::PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> CustomResult<Option<String>, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::Upi(UpiData::UpiCollect(collect_data)) => {
            if let Some(vpa_id) = &collect_data.vpa_id {
                let vpa = vpa_id.peek().to_string();
                if vpa.contains('@') && vpa.len() > 3 {
                    Ok(Some(vpa))
                } else {
                    Err(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    }
                    .into())
                }
            } else {
                Err(IntegrationError::MissingRequiredField {
                    field_name: "vpa_id",
                    context: Default::default(),
                }
                .into())
            }
        }
        _ => Ok(None),
    }
}

// Paytm signature generation algorithm implementation
// Following exact PayTM v2 algorithm from Haskell codebase
pub fn generate_paytm_signature(
    payload: &str,
    merchant_key: &str,
) -> CustomResult<String, IntegrationError> {
    // Step 1: Generate random salt bytes using ring (same logic, different implementation)
    let rng = SystemRandom::new();
    let mut salt_bytes = [0u8; constants::SALT_LENGTH];
    rng.fill(&mut salt_bytes)
        .map_err(|_| IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;

    // Step 2: Convert salt to Base64 (same logic)
    let salt_b64 = general_purpose::STANDARD.encode(salt_bytes);

    // Step 3: Create hash input: payload + "|" + base64_salt (same logic)
    let hash_input = format!("{payload}|{salt_b64}");

    // Step 4: SHA-256 hash using ring (same logic, different implementation)
    let hash_digest = digest::digest(&digest::SHA256, hash_input.as_bytes());
    let sha256_hash = hex::encode(hash_digest.as_ref());

    // Step 5: Create checksum: sha256_hash + base64_salt (same logic)
    let checksum = format!("{sha256_hash}{salt_b64}");

    // Step 6: AES encrypt checksum with merchant key (same logic)
    let signature = aes_encrypt(&checksum, merchant_key)?;

    Ok(signature)
}

// AES-CBC encryption implementation for PayTM v2
// This follows the exact PayTMv1 encrypt function used by PayTMv2:
// - Fixed IV: "@@@@&&&&####$$$$" (16 bytes) - exact value from Haskell code
// - Key length determines AES variant: 16→AES-128, 24→AES-192, other→AES-256
// - Mode: CBC with PKCS7 padding (16-byte blocks)
// - Output: Base64 encoded encrypted data
fn aes_encrypt(data: &str, key: &str) -> CustomResult<String, IntegrationError> {
    // PayTM uses fixed IV as specified in PayTMv1 implementation
    let iv = get_paytm_iv();
    let key_bytes = key.as_bytes();
    let data_bytes = data.as_bytes();

    // Determine AES variant based on key length (following PayTMv1 Haskell implementation)
    match key_bytes.len() {
        constants::AES_128_KEY_LENGTH => {
            // AES-128-CBC with PKCS7 padding
            type Aes128CbcEnc = Encryptor<Aes128>;
            let mut key_array = [0u8; constants::AES_128_KEY_LENGTH];
            key_array.copy_from_slice(key_bytes);

            let encryptor = Aes128CbcEnc::new(&key_array.into(), &iv.into());

            // Encrypt with proper buffer management
            let mut buffer = Vec::with_capacity(data_bytes.len() + constants::AES_BUFFER_PADDING);
            buffer.extend_from_slice(data_bytes);
            buffer.resize(buffer.len() + constants::AES_BUFFER_PADDING, 0);

            let encrypted_len = encryptor
                .encrypt_padded_mut::<Pkcs7>(&mut buffer, data_bytes.len())
                .map_err(|_| IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?
                .len();

            buffer.truncate(encrypted_len);
            Ok(general_purpose::STANDARD.encode(&buffer))
        }
        constants::AES_192_KEY_LENGTH => {
            // AES-192-CBC with PKCS7 padding
            type Aes192CbcEnc = Encryptor<Aes192>;
            let mut key_array = [0u8; constants::AES_192_KEY_LENGTH];
            key_array.copy_from_slice(key_bytes);

            let encryptor = Aes192CbcEnc::new(&key_array.into(), &iv.into());

            let mut buffer = Vec::with_capacity(data_bytes.len() + constants::AES_BUFFER_PADDING);
            buffer.extend_from_slice(data_bytes);
            buffer.resize(buffer.len() + constants::AES_BUFFER_PADDING, 0);

            let encrypted_len = encryptor
                .encrypt_padded_mut::<Pkcs7>(&mut buffer, data_bytes.len())
                .map_err(|_| IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?
                .len();

            buffer.truncate(encrypted_len);
            Ok(general_purpose::STANDARD.encode(&buffer))
        }
        _ => {
            // Default to AES-256-CBC with PKCS7 padding (for any other key length)
            type Aes256CbcEnc = Encryptor<Aes256>;

            // For AES-256, we need exactly 32 bytes, so pad or truncate the key
            let mut aes256_key = [0u8; constants::AES_256_KEY_LENGTH];
            let copy_len = cmp::min(key_bytes.len(), constants::AES_256_KEY_LENGTH);
            if let (Some(dest), Some(src)) =
                (aes256_key.get_mut(..copy_len), key_bytes.get(..copy_len))
            {
                dest.copy_from_slice(src);
            }

            let encryptor = Aes256CbcEnc::new(&aes256_key.into(), &iv.into());

            let mut buffer = Vec::with_capacity(data_bytes.len() + constants::AES_BUFFER_PADDING);
            buffer.extend_from_slice(data_bytes);
            buffer.resize(buffer.len() + constants::AES_BUFFER_PADDING, 0);

            let encrypted_len = encryptor
                .encrypt_padded_mut::<Pkcs7>(&mut buffer, data_bytes.len())
                .map_err(|_| IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?
                .len();

            buffer.truncate(encrypted_len);
            Ok(general_purpose::STANDARD.encode(&buffer))
        }
    }
}

// Fixed IV for Paytm AES encryption (from PayTM v2 Haskell implementation)
// IV value: "@@@@&&&&####$$$$" (16 characters) - exact value from Haskell codebase
fn get_paytm_iv() -> [u8; 16] {
    // This is the exact IV used by PayTM v2 as found in the Haskell codebase
    *constants::PAYTM_IV
}

// Helper function to determine channel ID based on OS type
fn get_channel_id_from_browser_info(browser_info: Option<&BrowserInformation>) -> Option<String> {
    match browser_info {
        Some(info) => match &info.os_type {
            Some(os_type) => {
                let os_lower = os_type.to_lowercase();
                if os_lower.contains("android") || os_lower.contains("ios") {
                    Some(constants::CHANNEL_ID_WAP.to_string())
                } else {
                    Some(constants::CHANNEL_ID_WEB.to_string())
                }
            }
            None => None,
        },
        None => None,
    }
}

pub fn create_paytm_header(
    request_body: &impl Serialize,
    auth: &PaytmAuthType,
    channel_id: Option<&str>,
) -> CustomResult<PaytmRequestHeader, IntegrationError> {
    let _payload = serde_json::to_string(request_body).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?;
    let signature = generate_paytm_signature(&_payload, auth.merchant_key.peek())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| IntegrationError::InvalidDataFormat {
            field_name: "timestamp",
            context: Default::default(),
        })?
        .as_secs()
        .to_string();

    Ok(PaytmRequestHeader {
        client_id: auth.client_id.clone(),
        version: constants::API_VERSION.to_string(),
        request_timestamp: timestamp,
        channel_id: channel_id.map(|id| id.to_string()),
        signature: signature.into(),
    })
}

pub fn map_paytm_authorize_status_to_attempt_status(status_code: &str) -> AttemptStatus {
    match status_code {
        // Success case - 0000: Success
        "0000" => AttemptStatus::Authorized,

        // 931: Incorrect Passcode
        // 1006: Your Session has expired.
        // 2004: Invalid User Token
        "931" | "1006" | "2004" => AttemptStatus::AuthenticationFailed,

        // RC-00018: Payment failed as merchant has crossed his daily/monthly/weekly acceptance limit
        // 312: This card is not supported. Please use another card.
        // 315: Invalid Year
        "RC-00018" | "312" | "315" => AttemptStatus::AuthorizationFailed,

        // 0001: FAILED
        // 309: Invalid Order ID
        // 1001: Request parameters are not valid
        // 1007: Missing mandatory element
        // 501: System Error
        // 510: Merchant Transaction Failure
        // 372: Retry count breached
        // 1005: Duplicate request handling
        "0001" | "309" | "1001" | "1007" | "501" | "510" | "372" | "1005" => AttemptStatus::Failure, // Invalid request parameters

        // Unknown status codes
        _ => AttemptStatus::Pending,
    }
}

pub fn map_paytm_sync_status_to_attempt_status(result_code: &str) -> AttemptStatus {
    match result_code {
        // Success case - 01: TXN_SUCCESS
        "01" => AttemptStatus::Charged,

        // 400: Transaction status not confirmed yet
        // 402: Payment not complete, confirming with bank
        "400" | "402" => AttemptStatus::Pending,

        // 335: Mid is invalid
        // 843: Your transaction has been declined by the bank. Remitting account is blocked or frozen.
        "335" | "843" => AttemptStatus::AuthorizationFailed,

        // 820: Mobile number linked to bank account has changed
        // 235: Wallet balance insufficient
        // 295: Invalid UPI ID
        // 334: Invalid Order ID
        // 267: Your payment has been declined due to Mandate gap
        // 331: No Record Found
        // 227: Payment declined by bank
        // 401: Payment declined by bank
        // 501: Server Down
        // 810: Transaction Failed
        "235" | "295" | "334" | "267" | "331" | "820" | "227" | "401" | "501" | "810" => {
            AttemptStatus::Failure
        }

        // Default to Pending for unknown codes to be safe
        _ => AttemptStatus::Pending,
    }
}

fn is_failure_status(status: AttemptStatus) -> bool {
    matches!(
        status,
        AttemptStatus::Failure
            | AttemptStatus::AuthenticationFailed
            | AttemptStatus::AuthorizationFailed
    )
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

// ================================
// RepeatPayment (MIT) Flow
// ================================

/// Extract mandate ID (subscription ID) from RepeatPaymentData mandate reference.
/// Paytm uses the connector_mandate_id as the subscriptionId for renewal.
fn extract_paytm_mandate_id(
    mandate_reference: &MandateReferenceId,
) -> Result<String, error_stack::Report<IntegrationError>> {
    match mandate_reference {
        MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
            .get_connector_mandate_id()
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })
            }),
        MandateReferenceId::NetworkMandateId(_) => {
            Err(error_stack::report!(
                IntegrationError::NotImplemented(
                    "Network mandate ID not supported for repeat payments in paytm".to_string(),
                    Default::default(),
                )
            ))
        }
        MandateReferenceId::NetworkTokenWithNTI(_) => {
            Err(error_stack::report!(
                IntegrationError::NotImplemented(
                    "Network token with NTI not supported for repeat payments in paytm".to_string(),
                    Default::default(),
                )
            ))
        }
    }
}

// PaytmRepeatPaymentRequest TryFrom RepeatPayment RouterData
impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
    >
    TryFrom<
        MacroPaytmRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaytmRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MacroPaytmRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PaytmAuthType::try_from(&item.router_data.connector_config)?;

        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let subscription_id =
            extract_paytm_mandate_id(&item.router_data.request.mandate_reference)?;

        let body = PaytmRepeatPaymentReqBody {
            mid: auth.merchant_id.clone(),
            order_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            txn_amount: PaytmAmount {
                value: amount,
                currency: item.router_data.request.currency,
            },
            subscription_id,
            extend_info: None,
        };

        let head = create_paytm_header(&body, &auth, None)?;

        Ok(Self { head, body })
    }
}

// RepeatPayment response transformation
impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
    > TryFrom<ResponseRouterData<PaytmRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<PaytmRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let mut router_data = item.router_data;

        let (result_code, result_msg) = match &response.body {
            PaytmRepeatPaymentRespBodyTypes::SuccessBody(body) => {
                (&body.result_info.result_code, &body.result_info.result_msg)
            }
            PaytmRepeatPaymentRespBodyTypes::FailureBody(body) => {
                (&body.result_info.result_code, &body.result_info.result_msg)
            }
        };

        // Map the Paytm renew subscription result code to attempt status
        let attempt_status = map_paytm_repeat_payment_status(result_code);
        router_data.resource_common_data.set_status(attempt_status);

        let (connector_txn_id, connector_ref_id) = match &response.body {
            PaytmRepeatPaymentRespBodyTypes::SuccessBody(body) => {
                let txn_id = body
                    .txn_id
                    .as_ref()
                    .map(|id| ResponseId::ConnectorTransactionId(id.clone()))
                    .unwrap_or(ResponseId::NoResponseId);
                let ref_id = Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                );
                (txn_id, ref_id)
            }
            PaytmRepeatPaymentRespBodyTypes::FailureBody(_) => {
                let ref_id = Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                );
                (ResponseId::NoResponseId, ref_id)
            }
        };

        router_data.response = if is_failure_status(attempt_status) {
            Err(domain_types::router_data::ErrorResponse {
                code: result_code.clone(),
                message: result_msg.clone(),
                reason: Some(result_msg.clone()),
                status_code: item.http_code,
                attempt_status: Some(attempt_status),
                connector_transaction_id: connector_ref_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: connector_txn_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: connector_ref_id,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(router_data)
    }
}

/// Map Paytm Renew Subscription result codes to AttemptStatus.
/// Result code 900 means "Subscription Txn accepted" (pending actual deduction).
fn map_paytm_repeat_payment_status(result_code: &str) -> AttemptStatus {
    match result_code {
        // 900: Subscription Txn accepted (async deduction)
        "900" => AttemptStatus::Pending,

        // Failure codes from Renew Subscription API
        "156" | "158" | "165" | "196" => AttemptStatus::AuthorizationFailed,
        "202" => AttemptStatus::Failure,
        "227" => AttemptStatus::Failure,

        // Default to Pending for unknown codes
        _ => AttemptStatus::Pending,
    }
}
