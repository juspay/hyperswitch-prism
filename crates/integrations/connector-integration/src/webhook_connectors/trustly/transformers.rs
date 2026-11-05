use base64::{engine::general_purpose, Engine};
use common_enums::Currency;
use common_utils::{ext_traits::ByteSliceExt, StringMajorUnit};
use domain_types::{
    connector_types::{
        PaymentWebhookReference, PayoutWebhookReference, RefundWebhookReference, RequestDetails,
        WebhookParseResponse, WebhookResourceReference,
    },
    errors::WebhookError,
};
use domain_types::{errors, utils::base64_decode};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use openssl::{
    hash::MessageDigest,
    pkey::PKey,
    rsa::Rsa,
    sign::{Signer, Verifier},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookBody {
    pub method: TrustlyWebhookMethod,
    pub params: TrustlyWebhookParams,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TrustlyWebhookMethod {
    Credit,
    Debit,
    Cancel,
    Account,
    Pending,
    PayoutConfirmation,
    PayoutFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookParams {
    pub signature: String,
    pub uuid: String,
    pub data: TrustlyWebhookData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookData {
    pub amount: Option<StringMajorUnit>,
    pub currency: Option<Currency>,
    pub messageid: Secret<String>,
    pub orderid: String,
    pub enduserid: Option<String>,
    pub accountid: Option<String>,
    pub verified: Option<String>,
    pub notificationid: String,
    pub timestamp: Option<String>,
    pub attributes: Option<TrustlyWebhookAttributes>,
    pub errorcode: Option<String>,
    pub errormessage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookAttributes {
    pub bank: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub name: Option<Secret<String>>,
    pub address: Option<Secret<String>>,
    pub zipcode: Option<Secret<String>>,
    pub personid: Option<Secret<String>>,
    pub descriptor: Option<String>,
    pub lastdigits: Option<String>,
    pub clearinghouse: Option<String>,
}

impl TrustlyWebhookMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Credit => "credit",
            Self::Debit => "debit",
            Self::Cancel => "cancel",
            Self::Account => "account",
            Self::Pending => "pending",
            Self::PayoutConfirmation => "payoutconfirmation",
            Self::PayoutFailed => "payoutfailed",
        }
    }
}

pub const TRUSTLY_PAYOUT_MESSAGE_ID_PREFIX: &str = "payout_";

fn is_payout_message_id(message_id: &str) -> bool {
    message_id.starts_with(TRUSTLY_PAYOUT_MESSAGE_ID_PREFIX)
}

pub fn is_payment_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::Credit
            | TrustlyWebhookMethod::Debit
            | TrustlyWebhookMethod::Cancel
            | TrustlyWebhookMethod::Account
            | TrustlyWebhookMethod::Pending
    ) && !is_payout_message_id(message_id)
}

pub fn is_refund_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::PayoutConfirmation | TrustlyWebhookMethod::PayoutFailed
    ) && !is_payout_message_id(message_id)
}

pub fn is_payout_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::PayoutConfirmation
            | TrustlyWebhookMethod::PayoutFailed
            | TrustlyWebhookMethod::Credit
            | TrustlyWebhookMethod::Cancel
    ) && is_payout_message_id(message_id)
}

pub fn get_webhook_event(
    event: TrustlyWebhookMethod,
    message_id: String,
) -> domain_types::connector_types::EventType {
    match (event, !message_id.as_str().starts_with("payout_")) {
        (TrustlyWebhookMethod::Credit, true) => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        (TrustlyWebhookMethod::Credit, false) => {
            domain_types::connector_types::EventType::PayoutReversed
        }
        (TrustlyWebhookMethod::Debit, _) => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        (TrustlyWebhookMethod::Cancel, true) => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        (TrustlyWebhookMethod::Cancel, false) => {
            domain_types::connector_types::EventType::PayoutCancelled
        }
        (TrustlyWebhookMethod::Account, _) => {
            domain_types::connector_types::EventType::PaymentAssociatedDataUpdate
        }
        (TrustlyWebhookMethod::Pending, _) => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        (TrustlyWebhookMethod::PayoutConfirmation, true) => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        (TrustlyWebhookMethod::PayoutFailed, true) => {
            domain_types::connector_types::EventType::RefundFailure
        }
        (TrustlyWebhookMethod::PayoutConfirmation, false) => {
            domain_types::connector_types::EventType::PayoutSuccess
        }
        (TrustlyWebhookMethod::PayoutFailed, false) => {
            domain_types::connector_types::EventType::PayoutFailure
        }
    }
}

pub fn parse_event(
    request: &RequestDetails,
) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>> {
    let body: TrustlyWebhookBody = request
        .body
        .parse_struct("TrustlyWebhookBody")
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;
    let message_id = body.params.data.messageid.expose();
    let event_type = get_webhook_event(body.method.clone(), message_id.clone());
    let order_id = body.params.data.orderid;
    let reference = if is_payment_webhook_event(&body.method, &message_id) {
        Some(WebhookResourceReference::Payment(PaymentWebhookReference {
            connector_transaction_id: Some(order_id),
            merchant_transaction_id: None,
            connector_preprocessing_id: None,
        }))
    } else if is_refund_webhook_event(&body.method, &message_id) {
        Some(WebhookResourceReference::Refund(RefundWebhookReference {
            connector_refund_id: Some(order_id.clone()),
            merchant_refund_id: None,
            connector_transaction_id: Some(order_id),
            merchant_transaction_id: None,
        }))
    } else if is_payout_webhook_event(&body.method, &message_id) {
        Some(WebhookResourceReference::Payout(PayoutWebhookReference {
            connector_payout_id: Some(order_id),
            merchant_payout_id: None,
        }))
    } else {
        None
    };
    Ok(WebhookParseResponse {
        event_type,
        reference,
    })
}

fn trustly_serialize<T: Serialize>(data: &T) -> String {
    let value = serde_json::to_value(data).unwrap_or_default();
    serialize_value(&value)
}

enum Algorithm {
    SHA256,
    SHA384,
    SHA512,
    SHA1,
}

impl Algorithm {
    fn message_digest(&self) -> MessageDigest {
        match self {
            Self::SHA256 => MessageDigest::sha256(),
            Self::SHA384 => MessageDigest::sha384(),
            Self::SHA512 => MessageDigest::sha512(),
            Self::SHA1 => MessageDigest::sha1(),
        }
    }

    fn prefix(&self) -> &'static str {
        "alg=RS256;"
    }
}

fn serialize_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: BTreeMap<_, _> = map.iter().collect();
            sorted
                .iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| format!("{}{}", k, serialize_value(v)))
                .collect()
        }
        serde_json::Value::Array(arr) => arr.iter().map(serialize_value).collect(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
    }
}

pub fn generate_trustly_signature<T: Serialize>(
    method: &str,
    uuid: &str,
    data: &T,
    private_key: &str,
) -> Result<String, errors::IntegrationError> {
    let algorithm = Algorithm::SHA256;
    let pem = base64_decode(private_key.to_string()).map_err(|_| {
        errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    })?;
    let rsa = Rsa::private_key_from_pem(&pem).map_err(|_| {
        errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    })?;
    let private_key =
        PKey::from_rsa(rsa).map_err(|_| errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;

    let plaintext = format!("{}{}{}", method, uuid, trustly_serialize(data));

    let mut signer = Signer::new(algorithm.message_digest(), &private_key).map_err(|_| {
        errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    })?;
    signer.update(plaintext.as_bytes()).map_err(|_| {
        errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    })?;
    let signature =
        signer
            .sign_to_vec()
            .map_err(|_| errors::IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

    Ok(format!(
        "{}{}",
        algorithm.prefix(),
        general_purpose::STANDARD.encode(&signature)
    ))
}

pub fn verify_webhook_signature(
    webhook_body: TrustlyWebhookBody,
    public_key: Vec<u8>,
) -> error_stack::Result<bool, WebhookError> {
    let method = webhook_body.method;
    let uuid = webhook_body.params.uuid;
    let data = &webhook_body.params.data;
    let signature = &webhook_body.params.signature;

    let pem_bytes = general_purpose::STANDARD
        .decode(&public_key)
        .change_context(WebhookError::WebhookSourceVerificationFailed)?;

    let rsa = Rsa::public_key_from_pem(&pem_bytes)
        .change_context(WebhookError::WebhookSourceVerificationFailed)?;
    let public_key =
        PKey::from_rsa(rsa).change_context(WebhookError::WebhookSourceVerificationFailed)?;

    let (algorithm, signature_b64) = if signature.len() >= 10
        && signature.starts_with("alg=RS")
        && matches!(signature.as_bytes().get(9), Some(b';'))
    {
        let prefix = &signature[..10];

        let algorithm = match prefix {
            "alg=RS256;" => Algorithm::SHA256,
            "alg=RS384;" => Algorithm::SHA384,
            "alg=RS512;" => Algorithm::SHA512,
            _ => Algorithm::SHA1,
        };

        (algorithm, &signature[10..])
    } else {
        (Algorithm::SHA1, signature.as_str())
    };

    let plaintext = format!("{}{}{}", method.as_str(), uuid, trustly_serialize(data));

    let signature_bytes = general_purpose::STANDARD
        .decode(signature_b64)
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;

    let mut verifier = Verifier::new(algorithm.message_digest(), &public_key)
        .change_context(WebhookError::WebhookSourceVerificationFailed)?;
    verifier
        .update(plaintext.as_bytes())
        .change_context(WebhookError::WebhookSourceVerificationFailed)?;
    verifier
        .verify(&signature_bytes)
        .change_context(WebhookError::WebhookSourceVerificationFailed)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookResponse {
    pub result: TrustlyWebhookResponseResult,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookResponseResult {
    pub signature: Secret<String>,
    pub uuid: String,
    pub method: String,
    pub data: TrustlyWebhookResponseResultData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookResponseResultData {
    pub status: String,
}
