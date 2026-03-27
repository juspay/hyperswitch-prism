use common_enums::Currency;
use common_utils::{types::StringMajorUnit, CustomerId, Email};
use hyperswitch_masking::Secret;
use serde::Serialize;

// Request structures for CreateSessionToken flow (Paytm initiate)

#[derive(Debug, Serialize)]
pub struct PaytmInitiateTxnRequest {
    pub head: PaytmRequestHeader,
    pub body: PaytmInitiateReqBody,
}

#[derive(Debug, Serialize)]
pub struct PaytmRequestHeader {
    #[serde(rename = "clientId")]
    pub client_id: Option<Secret<String>>,
    pub version: String,
    #[serde(rename = "requestTimestamp")]
    pub request_timestamp: String,
    #[serde(rename = "channelId", skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    pub signature: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct PaytmInitiateReqBody {
    #[serde(rename = "requestType")]
    pub request_type: String, // "Payment"
    pub mid: Secret<String>, // Merchant ID
    #[serde(rename = "orderId")]
    pub order_id: String, // Merchant Reference ID
    #[serde(rename = "websiteName")]
    pub website_name: Secret<String>, // From api_secret
    #[serde(rename = "txnAmount")]
    pub txn_amount: PaytmAmount,
    #[serde(rename = "userInfo")]
    pub user_info: PaytmUserInfo,
    #[serde(rename = "enablePaymentMode")]
    pub enable_payment_mode: Vec<PaytmEnableMethod>,
    #[serde(rename = "callbackUrl")]
    pub callback_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goods: Option<PaytmGoodsInfo>,
    #[serde(rename = "shippingInfo", skip_serializing_if = "Option::is_none")]
    pub shipping_info: Option<Vec<PaytmShippingInfo>>,
    #[serde(rename = "extendInfo", skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<PaytmExtendInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaytmAmount {
    pub value: StringMajorUnit, // Decimal amount (e.g., "10.50")
    pub currency: Currency,     // INR
}

#[derive(Debug, Serialize)]
pub struct PaytmUserInfo {
    #[serde(rename = "custId")]
    pub cust_id: CustomerId,
    pub mobile: Option<Secret<String>>,
    pub email: Option<Email>,
    #[serde(rename = "firstName")]
    pub first_name: Option<Secret<String>>,
    #[serde(rename = "lastName")]
    pub last_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct PaytmGoodsInfo {
    #[serde(rename = "merchantGoodsId", skip_serializing_if = "Option::is_none")]
    pub merchant_goods_id: Option<String>, // Unique id for the goods item
    #[serde(rename = "merchantShippingId", skip_serializing_if = "Option::is_none")]
    pub merchant_shipping_id: Option<String>, // Merchant shipping id
    #[serde(rename = "snapshotUrl", skip_serializing_if = "Option::is_none")]
    pub snapshot_url: Option<String>, // Product Image URL
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>, // Category of Product
    pub quantity: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>, // Unit of quantity (KG/Litre)
    pub price: PaytmAmount,
    #[serde(rename = "extendInfo", skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<PaytmExtendInfo>, // Extended info of goods
}

#[derive(Debug, Serialize)]
pub struct PaytmShippingInfo {
    #[serde(rename = "merchantShippingId", skip_serializing_if = "Option::is_none")]
    pub merchant_shipping_id: Option<String>,
    #[serde(rename = "trackingNo", skip_serializing_if = "Option::is_none")]
    pub tracking_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier: Option<String>,
    #[serde(rename = "chargeAmount", skip_serializing_if = "Option::is_none")]
    pub charge_amount: Option<PaytmAmount>, // reusing PaytmAmount struct
    #[serde(rename = "countryName", skip_serializing_if = "Option::is_none")]
    pub country_name: Option<common_enums::CountryAlpha2>,
    #[serde(rename = "stateName", skip_serializing_if = "Option::is_none")]
    pub state_name: Option<Secret<String>>,
    #[serde(rename = "cityName", skip_serializing_if = "Option::is_none")]
    pub city_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<Secret<String>>,
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(rename = "mobileNo", skip_serializing_if = "Option::is_none")]
    pub mobile_no: Option<Secret<String>>,
    #[serde(rename = "zipCode", skip_serializing_if = "Option::is_none")]
    pub zip_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
}

#[derive(Debug, Serialize)]
pub struct PaytmExtendInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udf3: Option<String>,
    #[serde(rename = "mercUnqRef", skip_serializing_if = "Option::is_none")]
    pub merc_unq_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
    #[serde(rename = "subwalletAmount", skip_serializing_if = "Option::is_none")]
    pub subwallet_amount: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaytmEnableMethod {
    pub mode: String,                  // "UPI"
    pub channels: Option<Vec<String>>, // ["UPI", "UPIPUSH"] for Collect and Intent
}

// Authorize flow request structures

// Enum to handle both UPI Intent and UPI Collect request types
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaytmAuthorizeRequest {
    Intent(PaytmProcessTxnRequest),
    Collect(PaytmNativeProcessTxnRequest),
}

#[derive(Debug, Serialize)]
pub struct PaytmProcessTxnRequest {
    pub head: PaytmProcessHeadTypes,
    pub body: PaytmProcessBodyTypes,
}

#[derive(Debug, Serialize)]
pub struct PaytmProcessHeadTypes {
    pub version: String, // "v2"
    #[serde(rename = "requestTimestamp")]
    pub request_timestamp: String,
    #[serde(rename = "channelId")]
    pub channel_id: Option<String>,
    #[serde(rename = "txnToken")]
    pub txn_token: Secret<String>, // From CreateSessionToken
}

#[derive(Debug, Serialize)]
pub struct PaytmProcessBodyTypes {
    pub mid: Secret<String>,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "requestType")]
    pub request_type: String, // "Payment"
    #[serde(rename = "paymentMode")]
    pub payment_mode: String, // "UPI"
    #[serde(rename = "paymentFlow")]
    pub payment_flow: Option<String>, // "NONE"
    #[serde(rename = "txnNote", skip_serializing_if = "Option::is_none")]
    pub txn_note: Option<String>, //Transaction note providing a short description
    #[serde(rename = "extendInfo", skip_serializing_if = "Option::is_none")]
    pub extend_info: Option<PaytmExtendInfo>,
}

// UPI Collect Native Process Request
#[derive(Debug, Serialize)]
pub struct PaytmNativeProcessTxnRequest {
    pub head: PaytmTxnTokenType,
    pub body: PaytmNativeProcessRequestBody,
}

#[derive(Debug, Serialize)]
pub struct PaytmTxnTokenType {
    #[serde(rename = "txnToken")]
    pub txn_token: Secret<String>, // From CreateSessionToken
}

#[derive(Debug, Serialize)]
pub struct PaytmNativeProcessRequestBody {
    #[serde(rename = "requestType")]
    pub request_type: String, // "NATIVE"
    pub mid: Secret<String>,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "paymentMode")]
    pub payment_mode: String, // "UPI"
    #[serde(rename = "payerAccount")]
    pub payer_account: Option<String>, // UPI VPA for collect
    #[serde(rename = "channelCode")]
    pub channel_code: Option<String>, // Gateway code
    #[serde(rename = "channelId")]
    pub channel_id: String, // "WEB"
    #[serde(rename = "txnToken")]
    pub txn_token: Secret<String>, // From CreateSessionToken
    #[serde(rename = "authMode")]
    pub auth_mode: Option<String>, // "DEBIT_PIN"
}

// PSync (Payment Sync) flow request structures

#[derive(Debug, Serialize)]
pub struct PaytmTransactionStatusRequest {
    pub head: PaytmRequestHeader, //only signature data PaytmV2SyncRequestHeader = PaytmV2SyncRequestHeader {signature :: Text}
    pub body: PaytmTransactionStatusReqBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaytmTransactionStatusReqBody {
    pub mid: Secret<String>, // Merchant ID
    pub order_id: String,    // Order ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_type: Option<String>, // PREAUTH, CAPTURE, RELEASE, WITHDRAW
}
