use base64::Engine;
use common_enums::enums;
use common_utils::{
    ext_traits::XmlExt,
    types::{MinorUnit, StringMajorUnit},
};
use domain_types::connector_types;
use error_stack::{Report, ResultExt};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

#[derive(Debug, Deserialize)]
pub struct BraintreeWebhookResponse {
    pub bt_signature: String,
    pub bt_payload: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Notification {
    pub kind: String, // xml parse only string to fields
    pub timestamp: String,
    pub dispute: Option<BraintreeDisputeData>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BraintreeDisputeData {
    pub amount_disputed: MinorUnit,
    pub amount_won: Option<String>,
    pub case_number: Option<String>,
    pub chargeback_protection_level: Option<String>,
    pub currency_iso_code: enums::Currency,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub created_at: Option<PrimitiveDateTime>,
    pub evidence: Option<DisputeEvidence>,
    pub id: String,
    pub kind: String, // xml parse only string to fields
    pub status: String,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub updated_at: Option<PrimitiveDateTime>,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub reply_by_date: Option<PrimitiveDateTime>,
    pub transaction: DisputeTransaction,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DisputeTransaction {
    pub amount: StringMajorUnit,
    pub id: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct DisputeEvidence {
    pub comment: String,
    pub id: Secret<String>,
    pub created_at: Option<PrimitiveDateTime>,
    pub url: url::Url,
}

// Maps the Braintree notification `kind` to the prism webhook event type.
// Ports HS `get_status` (hyperswitch braintree/transformers.rs `get_status`) 1:1.
pub(crate) fn get_status(status: &str) -> connector_types::EventType {
    match status {
        "dispute_opened" => connector_types::EventType::DisputeOpened,
        "dispute_lost" => connector_types::EventType::DisputeLost,
        "dispute_won" => connector_types::EventType::DisputeWon,
        "dispute_accepted" | "dispute_auto_accepted" => connector_types::EventType::DisputeAccepted,
        "dispute_expired" => connector_types::EventType::DisputeExpired,
        "dispute_disputed" => connector_types::EventType::DisputeChallenged,
        _ => connector_types::EventType::IncomingWebhookEventUnspecified,
    }
}

// Maps the Braintree notification `kind` to the prism dispute status.
// Mirrors how the HS router derives `DisputeStatus` from the webhook event.
pub(crate) fn get_dispute_status(status: &str) -> enums::DisputeStatus {
    match status {
        "dispute_opened" => enums::DisputeStatus::DisputeOpened,
        "dispute_lost" => enums::DisputeStatus::DisputeLost,
        "dispute_won" => enums::DisputeStatus::DisputeWon,
        "dispute_accepted" | "dispute_auto_accepted" => enums::DisputeStatus::DisputeAccepted,
        "dispute_expired" => enums::DisputeStatus::DisputeExpired,
        "dispute_disputed" => enums::DisputeStatus::DisputeChallenged,
        _ => enums::DisputeStatus::DisputeOpened,
    }
}

// Maps the Braintree dispute `kind` to the prism dispute stage.
// Ports HS `get_dispute_stage` 1:1.
pub(crate) fn get_dispute_stage(
    code: &str,
) -> Result<enums::DisputeStage, Report<domain_types::errors::WebhookError>> {
    match code {
        "CHARGEBACK" => Ok(enums::DisputeStage::Dispute),
        "PRE_ARBITRATION" => Ok(enums::DisputeStage::PreArbitration),
        "RETRIEVAL" => Ok(enums::DisputeStage::PreDispute),
        _ => Err(error_stack::report!(
            domain_types::errors::WebhookError::WebhookBodyDecodingFailed
        )),
    }
}

// Decodes the form-urlencoded webhook envelope (`bt_signature` + `bt_payload`).
pub(crate) fn get_webhook_object_from_body(
    body: &[u8],
) -> Result<BraintreeWebhookResponse, Report<domain_types::errors::WebhookError>> {
    serde_urlencoded::from_bytes::<BraintreeWebhookResponse>(body)
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable(
            "failed to url-decode the Braintree webhook body (bt_signature/bt_payload)",
        )
}

// Base64-decodes the (newline-stripped) `bt_payload` and parses the XML `Notification`.
pub(crate) fn decode_webhook_payload(
    payload: &[u8],
) -> Result<Notification, Report<domain_types::errors::WebhookError>> {
    let decoded_response = crate::connectors::braintree::BASE64_ENGINE
        .decode(payload)
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("failed to base64-decode the Braintree bt_payload")?;

    let xml_response = String::from_utf8(decoded_response)
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("Braintree bt_payload is not valid UTF-8")?;

    xml_response
        .parse_xml::<Notification>()
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("failed to parse the Braintree notification XML")
}

// `bt_signature` is `pubkey1|sig1&pubkey2|sig2&...`; pick the signature whose
// public key matches the merchant's Braintree public key.
pub(crate) fn get_matching_webhook_signature(
    signature_pairs: &[(&str, &str)],
    secret: &str,
) -> Option<String> {
    signature_pairs
        .iter()
        .find(|(public_key, _)| *public_key == secret)
        .map(|(_, signature)| signature.to_string())
}

// Full request -> `Notification` decode: urlencoded envelope, then base64 + XML on the
// newline-stripped `bt_payload`. Mirrors the two-step decode the trait methods performed inline.
pub(crate) fn decode_from_request(
    request: &connector_types::RequestDetails,
) -> Result<Notification, Report<domain_types::errors::WebhookError>> {
    let notif = get_webhook_object_from_body(&request.body)?;
    decode_webhook_payload(notif.bt_payload.replace('\n', "").as_bytes())
}

// Builds the typed webhook resource reference for a dispute notification.
pub(crate) fn get_webhook_reference(
    notification: &Notification,
) -> Result<
    Option<connector_types::WebhookResourceReference>,
    Report<domain_types::errors::WebhookError>,
> {
    match &notification.dispute {
        // HS emits `PaymentId(ConnectorTransactionId(transaction.id))`. The shadow normaliser
        // maps a prism Dispute reference via `connector_dispute_id.or(connector_transaction_id)`,
        // preferring connector_dispute_id, so it MUST be `None` here to match HS byte-for-byte.
        Some(dispute_data) => Ok(Some(connector_types::WebhookResourceReference::Dispute(
            connector_types::DisputeWebhookReference {
                connector_dispute_id: None,
                connector_transaction_id: Some(dispute_data.transaction.id.clone()),
            },
        ))),
        None => Err(error_stack::report!(
            domain_types::errors::WebhookError::WebhookReferenceIdNotFound
        )),
    }
}

// Builds the dispute webhook response, including the webhook amount conversion.
pub(crate) fn build_webhook_dispute_response(
    notification: &Notification,
    raw_body: &[u8],
) -> Result<
    connector_types::DisputeWebhookDetailsResponse,
    Report<domain_types::errors::WebhookError>,
> {
    match &notification.dispute {
        Some(dispute_data) => Ok(connector_types::DisputeWebhookDetailsResponse {
            amount: domain_types::utils::convert_amount_for_webhook(
                &common_utils::types::StringMinorUnitForConnector,
                &domain_types::utils::convert_back_amount_to_money_for_webhook(
                    &common_utils::types::MinorUnitForConnector,
                    dispute_data.amount_disputed,
                    dispute_data.currency_iso_code,
                )?,
            )?,
            currency: dispute_data.currency_iso_code,
            dispute_id: dispute_data.id.clone(),
            status: get_dispute_status(notification.kind.as_str()),
            stage: get_dispute_stage(dispute_data.kind.as_str())?,
            connector_response_reference_id: None,
            dispute_message: dispute_data.reason.clone(),
            connector_reason_code: dispute_data.reason_code.clone(),
            additional_details: None,
            raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
            status_code: 200,
            response_headers: None,
        }),
        None => Err(error_stack::report!(
            domain_types::errors::WebhookError::WebhookResourceObjectNotFound
        )),
    }
}
