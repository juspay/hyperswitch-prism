use std::collections::HashMap;

use common_enums::Currency;
use common_utils::types::StringMajorUnit;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

// Response structures for CreateSessionToken flow

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmInitiateTxnResponse {
    pub head: PaytmRespHead,
    pub body: PaytmResBodyTypes,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaytmResBodyTypes {
    SuccessBody(PaytmRespBody),
    FailureBody(PaytmErrorBody),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmRespBody {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
    #[serde(rename = "txnToken")]
    pub txn_token: Secret<String>, // This will be stored as session_token
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmResultInfo {
    #[serde(rename = "resultStatus")]
    pub result_status: String,
    #[serde(rename = "resultCode")]
    pub result_code: String, // "0000" for success, "0002" for duplicate
    #[serde(rename = "resultMsg")]
    pub result_msg: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmRespHead {
    #[serde(rename = "responseTimestamp")]
    pub response_timestamp: Option<String>,
    pub version: String,
    #[serde(rename = "clientId")]
    pub client_id: Option<Secret<String>>,
    pub signature: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmErrorBody {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
}

// Error response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmErrorResponse {
    #[serde(rename = "errorCode")]
    pub error_code: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "errorDescription")]
    pub error_description: Option<String>,
    #[serde(rename = "transactionId")]
    pub transaction_id: Option<String>,
}

// Transaction info structure used in multiple response types
// Supports both lowercase (txnId) and uppercase (TXNID) field name variants
#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmTxnInfo {
    #[serde(rename = "txnId", alias = "TXNID")]
    pub txn_id: Option<String>,
    #[serde(rename = "orderId", alias = "ORDERID")]
    pub order_id: Option<String>,
    #[serde(rename = "bankTxnId", alias = "BANKTXNID")]
    pub bank_txn_id: Option<String>,
    #[serde(alias = "STATUS")]
    pub status: Option<String>,
    #[serde(rename = "respCode", alias = "RESPCODE")]
    pub resp_code: Option<String>,
    #[serde(rename = "respMsg", alias = "RESPMSG")]
    pub resp_msg: Option<String>,
    // Additional callback-specific fields
    #[serde(alias = "CHECKSUMHASH")]
    pub checksum_hash: Option<String>,
    #[serde(alias = "CURRENCY")]
    pub currency: Option<Currency>,
    #[serde(alias = "GATEWAYNAME")]
    pub gateway_name: Option<String>,
    #[serde(alias = "MID")]
    pub mid: Option<String>,
    #[serde(alias = "PAYMENTMODE")]
    pub payment_mode: Option<String>,
    #[serde(alias = "TXNAMOUNT")]
    pub txn_amount: Option<StringMajorUnit>,
    #[serde(alias = "TXNDATE")]
    pub txn_date: Option<String>,
}

// Alternative error response structure for callback URL format
#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmCallbackErrorResponse {
    pub head: PaytmRespHead,
    pub body: PaytmCallbackErrorBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmCallbackErrorBody {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
    #[serde(rename = "txnInfo")]
    pub txn_info: PaytmTxnInfo,
}

// Authorize flow response structures

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmProcessTxnResponse {
    pub head: PaytmProcessHead,
    pub body: PaytmProcessRespBodyTypes,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmProcessHead {
    pub version: Option<String>,
    #[serde(rename = "responseTimestamp")]
    pub response_timestamp: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaytmProcessRespBodyTypes {
    SuccessBody(Box<PaytmProcessSuccessResp>),
    FailureBody(PaytmProcessFailureResp),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmProcessSuccessResp {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
    #[serde(rename = "deepLinkInfo", skip_serializing_if = "Option::is_none")]
    pub deep_link_info: Option<PaytmDeepLinkInfo>,
    #[serde(rename = "bankForm", skip_serializing_if = "Option::is_none")]
    pub bank_form: Option<serde_json::Value>,
    #[serde(rename = "upiDirectForm", skip_serializing_if = "Option::is_none")]
    pub upi_direct_form: Option<serde_json::Value>,
    #[serde(rename = "displayField", skip_serializing_if = "Option::is_none")]
    pub display_field: Option<serde_json::Value>,
    #[serde(rename = "riskContent", skip_serializing_if = "Option::is_none")]
    pub risk_content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmDeepLinkInfo {
    #[serde(rename = "deepLink")]
    pub deep_link: String, // UPI intent URL
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "cashierRequestId")]
    pub cashier_request_id: String,
    #[serde(rename = "transId")]
    pub trans_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmProcessFailureResp {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
}

// UPI Collect Native Process Response
#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmNativeProcessTxnResponse {
    pub head: PaytmProcessHead,
    pub body: PaytmNativeProcessRespBodyTypes,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaytmNativeProcessRespBodyTypes {
    SuccessBody(PaytmNativeProcessSuccessResp),
    FailureBody(PaytmNativeProcessFailureResp),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmNativeProcessSuccessResp {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
    #[serde(rename = "transId")]
    pub trans_id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmNativeProcessFailureResp {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
}

// PSync (Payment Sync) flow response structures

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmTransactionStatusResponse {
    pub head: PaytmRespHead,
    pub body: PaytmTransactionStatusRespBodyTypes,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaytmTransactionStatusRespBodyTypes {
    SuccessBody(Box<PaytmTransactionStatusRespBody>),
    FailureBody(PaytmErrorBody),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaytmTransactionStatusRespBody {
    pub result_info: PaytmResultInfo,
    pub txn_id: String,
    pub bank_txn_id: Option<String>,
    pub order_id: String,
    pub txn_amount: Option<StringMajorUnit>,
    pub txn_type: Option<String>,
    pub gateway_name: Option<String>,
    pub mid: Option<String>,
    pub payment_mode: Option<String>,
    pub refund_amt: Option<String>,
    pub txn_date: Option<String>,
}

// Additional response structures needed for compilation

// Session token error response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmSessionTokenErrorResponse {
    pub head: PaytmRespHead,
    pub body: PaytmSessionTokenErrorBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmSessionTokenErrorBody {
    #[serde(rename = "extraParamsMap")]
    pub extra_params_map: Option<serde_json::Value>,
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
}

// Success transaction response structure (handles both callback and standard formats)
#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmSuccessTransactionResponse {
    pub head: PaytmRespHead,
    pub body: PaytmSuccessTransactionBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmSuccessTransactionBody {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
    #[serde(rename = "txnInfo")]
    pub txn_info: PaytmTxnInfo,
    #[serde(rename = "callBackUrl")]
    pub callback_url: Option<String>,
}

// Bank form response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmBankFormResponse {
    pub head: PaytmRespHead,
    pub body: PaytmBankFormBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmBankFormBody {
    #[serde(rename = "resultInfo")]
    pub result_info: PaytmResultInfo,
    #[serde(rename = "bankForm")]
    pub bank_form: PaytmBankForm,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmBankForm {
    #[serde(rename = "redirectForm")]
    pub redirect_form: PaytmRedirectForm,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaytmRedirectForm {
    #[serde(rename = "actionUrl")]
    pub action_url: String,
    pub method: String,
    pub content: HashMap<String, String>,
}
