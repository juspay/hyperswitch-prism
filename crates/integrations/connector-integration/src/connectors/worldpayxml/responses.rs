use common_utils::StringMinorUnit;
use hyperswitch_masking::Secret;
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlAuthorizeResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: String,
    pub reply: WorldpayxmlReply,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlReply {
    #[serde(rename = "orderStatus")]
    pub order_status: Option<WorldpayxmlOrderStatus>,
    pub error: Option<WorldpayxmlError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlOrderStatus {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    pub payment: Option<WorldpayxmlPayment>,
    /// Present when the order asked Worldpay to create a payment token.
    pub token: Option<WorldpayxmlToken>,
    pub error: Option<WorldpayxmlError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlToken {
    #[serde(rename = "authenticatedShopperID")]
    pub authenticated_shopper_id: Option<String>,
    pub token_details: WorldpayxmlTokenDetails,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlTokenDetails {
    #[serde(rename = "@tokenEvent")]
    pub token_event: Option<String>,
    #[serde(rename = "paymentTokenID")]
    pub payment_token_id: Secret<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlPayment {
    pub payment_method: Option<String>,
    pub amount: Option<WorldpayxmlAmountResponse>,
    pub last_event: WorldpayxmlLastEvent,
    #[serde(rename = "AuthorisationId")]
    pub authorisation_id: Option<WorldpayxmlAuthorisationId>,
    #[serde(rename = "ISO8583ReturnCode")]
    pub iso8583_return_code: Option<WorldpayxmlISO8583ReturnCode>,
    // Additional fields from XML response (optional, not used in processing)
    #[serde(rename = "CVCResultCode")]
    pub cvc_result_code: Option<WorldpayxmlResultCode>,
    #[serde(rename = "AVSResultCode")]
    pub avs_result_code: Option<WorldpayxmlResultCode>,
    #[serde(rename = "AAVAddressResultCode")]
    pub aav_address_result_code: Option<WorldpayxmlResultCode>,
    #[serde(rename = "AAVPostcodeResultCode")]
    pub aav_postcode_result_code: Option<WorldpayxmlResultCode>,
    #[serde(rename = "AAVCardholderNameResultCode")]
    pub aav_cardholder_name_result_code: Option<WorldpayxmlResultCode>,
    #[serde(rename = "AAVTelephoneResultCode")]
    pub aav_telephone_result_code: Option<WorldpayxmlResultCode>,
    #[serde(rename = "AAVEmailResultCode")]
    pub aav_email_result_code: Option<WorldpayxmlResultCode>,
    pub card_holder_name: Option<String>,
    pub issuer_country_code: Option<String>,
    pub issuer_name: Option<String>,
    pub payment_method_detail: Option<WorldpayxmlPaymentMethodDetail>,
    pub balance: Option<Vec<WorldpayxmlBalance>>,
    pub scheme_response: Option<WorldpayxmlSchemeResponse>,
    pub card_p_a_r: Option<String>,
}

/// `lastEvent`/`PaymentStatus` values Worldpay reports for an order. Unmodelled values
/// deserialize to [`WorldpayxmlLastEvent::Unknown`] rather than failing the response.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorldpayxmlLastEvent {
    Authorised,
    Refused,
    Cancelled,
    Captured,
    Settled,
    SentForAuthorisation,
    SentForRefund,
    SentForFastRefund,
    Refunded,
    RefundRequested,
    RefundFailed,
    RefundedByMerchant,
    Expired,
    Error,
    QueryRequired,
    CancelReceived,
    RefundReceived,
    PushRequested,
    PushPending,
    PushApproved,
    PushRefused,
    SettledByMerchant,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlResultCode {
    #[serde(rename = "@description")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlBalance {
    #[serde(rename = "@accountType")]
    pub account_type: Option<String>,
    pub amount: Option<WorldpayxmlAmountResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlSchemeResponse {
    pub transaction_identifier: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlPaymentMethodDetail {
    pub card: Option<WorldpayxmlCardResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlCardResponse {
    #[serde(rename = "@number")]
    pub number: Option<String>,
    #[serde(rename = "@type")]
    pub card_type: Option<String>,
    pub expiry_date: Option<WorldpayxmlExpiryDate>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlExpiryDate {
    pub date: Option<WorldpayxmlDate>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlDate {
    #[serde(rename = "@month")]
    pub month: String,
    #[serde(rename = "@year")]
    pub year: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlAmountResponse {
    #[serde(rename = "@value")]
    pub value: String,
    #[serde(rename = "@currencyCode")]
    pub currency_code: String,
    #[serde(rename = "@exponent")]
    pub exponent: String,
    #[serde(rename = "@debitCreditIndicator")]
    pub debit_credit_indicator: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlAuthorisationId {
    #[serde(rename = "@id")]
    pub id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlISO8583ReturnCode {
    #[serde(rename = "@code")]
    pub code: String,
    #[serde(rename = "@description")]
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlError {
    #[serde(rename = "@code")]
    pub code: String,
    #[serde(rename = "$text")]
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlCaptureResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: String,
    pub reply: WorldpayxmlCaptureReply,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlCaptureReply {
    pub ok: Option<WorldpayxmlCaptureOk>,
    pub error: Option<WorldpayxmlError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlCaptureOk {
    pub capture_received: WorldpayxmlCaptureReceived,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlCaptureReceived {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    pub amount: Option<WorldpayxmlAmountResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlVoidResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: String,
    pub reply: WorldpayxmlVoidReply,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlVoidReply {
    pub ok: Option<WorldpayxmlVoidOk>,
    pub error: Option<WorldpayxmlError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlVoidOk {
    pub cancel_received: WorldpayxmlCancelReceived,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlCancelReceived {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlRefundResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: String,
    pub reply: WorldpayxmlRefundReply,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlRefundReply {
    pub ok: Option<WorldpayxmlRefundOk>,
    pub error: Option<WorldpayxmlError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlRefundOk {
    pub refund_received: WorldpayxmlRefundReceived,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlRefundReceived {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    pub amount: Option<WorldpayxmlAmountResponse>,
}

// VoidPC (cancelOrRefund) response - reuses cancelReceived element
// The cancelOrRefund operation can return either cancelOrRefundReceived or cancelReceived
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlVoidPCResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: String,
    pub reply: WorldpayxmlVoidPCReply,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlVoidPCReply {
    pub ok: Option<WorldpayxmlVoidPCOk>,
    pub error: Option<WorldpayxmlError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlVoidPCOk {
    // cancelOrRefund can return orderCode as attribute on <ok> itself,
    // or inside a <cancelOrRefundReceived> or <cancelReceived> child element
    #[serde(rename = "@orderCode")]
    pub order_code: Option<String>,
    #[serde(rename = "cancelOrRefundReceived")]
    pub cancel_or_refund_received: Option<WorldpayxmlCancelOrRefundReceived>,
    #[serde(rename = "cancelReceived")]
    pub cancel_received: Option<WorldpayxmlCancelReceived>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlCancelOrRefundReceived {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
}

/// Carrier for a PSync/RSync body: the XML `<paymentService>` order-inquiry envelope, or the
/// order-notification body.
///
/// Deliberately does not derive `Deserialize` as an `untagged` enum: `untagged` forces
/// `deserialize_any`, whose buffer loses XML sequence and text semantics, so the `Payment`
/// variant never matched and the notification variant absorbed every payload.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum WorldpayxmlTransactionResponse {
    Payment(Box<WorldpayxmlAuthorizeResponse>),
    Webhook(WorldpayxmlWebhookResponse),
}

/// Concrete shape covering both bodies a sync call can return, so the variant can be picked
/// without buffering. The notification half is PascalCase; the `<paymentService>` half is not,
/// so those three keep an explicit rename.
#[derive(Debug, Deserialize)]
#[serde(rename = "paymentService", rename_all = "PascalCase")]
struct WorldpayxmlSyncResponseBody {
    #[serde(rename = "@version")]
    version: Option<String>,
    #[serde(rename = "@merchantCode")]
    merchant_code: Option<String>,
    #[serde(rename = "reply")]
    reply: Option<WorldpayxmlReply>,
    // Order-notification body.
    payment_amount: Option<StringMinorUnit>,
    payment_id: Option<String>,
    order_code: Option<String>,
    payment_status: Option<WorldpayxmlLastEvent>,
    return_code: Option<String>,
    return_message: Option<String>,
}

impl<'de> Deserialize<'de> for WorldpayxmlTransactionResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let body = WorldpayxmlSyncResponseBody::deserialize(deserializer)?;

        if let Some(reply) = body.reply {
            return Ok(Self::Payment(Box::new(WorldpayxmlAuthorizeResponse {
                version: body.version.unwrap_or_default(),
                merchant_code: body.merchant_code.unwrap_or_default(),
                reply,
            })));
        }

        match (body.order_code, body.payment_status) {
            (Some(order_code), Some(payment_status)) => {
                Ok(Self::Webhook(WorldpayxmlWebhookResponse {
                    payment_amount: body.payment_amount,
                    payment_id: body.payment_id,
                    order_code,
                    payment_status,
                    return_code: body.return_code,
                    return_message: body.return_message,
                }))
            }
            _ => Err(de::Error::custom(
                "worldpayxml: sync response body carried neither a paymentService reply nor an order notification",
            )),
        }
    }
}

/// Order-notification body Worldpay can deliver for an order.
///
/// `order_code` and `payment_status` are mandatory: making them optional is what let this shape
/// match every payload and mask genuine parse failures.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldpayxmlWebhookResponse {
    pub payment_amount: Option<StringMinorUnit>,
    pub payment_id: Option<String>,
    pub order_code: String,
    pub payment_status: WorldpayxmlLastEvent,
    pub return_code: Option<String>,
    pub return_message: Option<String>,
}

// Type alias for RSync - reuses PSync response structure
pub type WorldpayxmlRsyncResponse = WorldpayxmlTransactionResponse;

// SetupMandate and RepeatPayment return the same `<reply><orderStatus>` envelope as Authorize.
// They are aliased (rather than reused directly) because the connector macros key their
// per-flow bridge types off the request/response type name.
pub type WorldpayxmlSetupMandateResponse = WorldpayxmlAuthorizeResponse;
pub type WorldpayxmlRepeatPaymentResponse = WorldpayxmlAuthorizeResponse;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WorldpayxmlErrorResponse {
    // Error response structures will be implemented in flow implementation phase
    Standard(WorldpayxmlStandardError),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldpayxmlStandardError {
    pub code: Option<String>,
    pub message: Option<String>,
}

// ===== PAYOUT RESPONSES =====

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlPayoutTransferResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: String,
    pub reply: WorldpayxmlReply,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlPayoutGetResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: String,
    pub reply: WorldpayxmlReply,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlPayoutVoidResponse {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: String,
    pub reply: WorldpayxmlPayoutVoidReply,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlPayoutVoidReply {
    pub ok: Option<WorldpayxmlPayoutCancelOk>,
    pub error: Option<WorldpayxmlError>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlPayoutCancelOk {
    pub cancel_received: WorldpayxmlCancelReceived,
}
