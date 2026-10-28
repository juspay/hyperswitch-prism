//! Kgp (Omise) wire-format transformers.
//!
//! Omise speaks `application/x-www-form-urlencoded` on two hosts:
//!   - `vault.omise.co` + public key  → `POST /tokens`
//!   - `api.omise.co`   + secret key  → charges / capture / reverse / refunds
//!
//! Nested card keys are literal brackets on the wire (`card[number]`, …).
//! Amounts are integer minor units.

use std::{collections::HashMap, fmt::Debug};

use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PaymentMethodToken, PSync, RSync, Refund,
        Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        PaymentVoidData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RefundWebhookReference, ResponseId, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::{
        AuthoriseIntegrityObject, CaptureIntegrityObject, PaymentMethodTokenIntegrityObject,
        PaymentSynIntegrityObject, PaymentVoidIntegrityObject, RefundIntegrityObject,
        RefundSyncIntegrityObject,
    },
    router_response_types::RedirectForm,
};
use error_stack::Report;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::KgpRouterData;
use crate::types::ResponseRouterData;

// ============================================================================
// AUTH
// ============================================================================

/// Omise dual-key auth: secret key hits `api.omise.co`, public key hits
/// `vault.omise.co` for tokenization. HTTP Basic with an empty password.
pub struct KgpAuthType {
    pub secret_key: Secret<String>,
    pub public_key: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for KgpAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Kgp {
                api_key,
                public_key,
                ..
            } => Ok(Self {
                secret_key: api_key.to_owned(),
                public_key: public_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure this merchant account's Kgp connector with a secret_key \
                         (api_key) and public_key (ConnectorSpecificConfig::Kgp)."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "The connector_config passed to KgpAuthType::try_from was not \
                         the ConnectorSpecificConfig::Kgp variant."
                            .to_string(),
                    ),
                },
            }
            .into()),
        }
    }
}

// ============================================================================
// ERRORS
// ============================================================================

/// Omise error envelope: `{ "object": "error", "code": "...", "message": "..." }`.
/// In-band payment failures on a 2xx charge also carry `failure_code` /
/// `failure_message` on the charge object itself (handled in status mapping).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpErrorResponse {
    pub object: Option<String>,
    pub code: Option<String>,
    pub message: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
}

// ============================================================================
// STATUS MAPPING
// ============================================================================

/// Omise charge `status` field. Wire values are snake_case.
/// `authorized` / `paid` / `captured` arrive as sibling booleans on the charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KgpChargeStatus {
    Pending,
    Successful,
    Failed,
    Expired,
    Reversed,
    #[serde(other)]
    Unknown,
}

/// Omise refund `status` field: `closed` | `failed` | `pending`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KgpRefundStatus {
    Closed,
    Failed,
    Pending,
    #[serde(other)]
    Unknown,
}

/// Map an Omise charge to `AttemptStatus`.
///
/// `authorize_uri` present + `pending` = 3DS hand-off (`AuthenticationPending`).
/// `successful` + `paid` = captured; `successful` + `authorized && !paid` = auth-only.
pub fn map_charge_status(
    status: KgpChargeStatus,
    authorized: bool,
    paid: bool,
    has_authorize_uri: bool,
) -> AttemptStatus {
    match status {
        KgpChargeStatus::Pending => {
            if has_authorize_uri {
                AttemptStatus::AuthenticationPending
            } else {
                AttemptStatus::Pending
            }
        }
        KgpChargeStatus::Successful => {
            if paid {
                AttemptStatus::Charged
            } else if authorized {
                AttemptStatus::Authorized
            } else {
                AttemptStatus::Pending
            }
        }
        KgpChargeStatus::Failed | KgpChargeStatus::Expired => AttemptStatus::Failure,
        KgpChargeStatus::Reversed => AttemptStatus::Voided,
        KgpChargeStatus::Unknown => AttemptStatus::Pending,
    }
}

pub fn map_refund_status(status: KgpRefundStatus) -> RefundStatus {
    match status {
        KgpRefundStatus::Closed => RefundStatus::Success,
        KgpRefundStatus::Failed => RefundStatus::Failure,
        KgpRefundStatus::Pending => RefundStatus::Pending,
        KgpRefundStatus::Unknown => RefundStatus::Pending,
    }
}

/// Build the `response: Err(ErrorResponse)` payload for an in-band charge failure.
fn charge_failure_error(response: &KgpCharge, http_code: u16) -> ErrorResponse {
    ErrorResponse {
        code: response
            .failure_code
            .clone()
            .or_else(|| response.failure_message.clone())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response
            .failure_message
            .clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: response.failure_message.clone(),
        status_code: http_code,
        attempt_status: None,
        connector_transaction_id: Some(response.id.clone()),
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

fn refund_failure_error(response: &KgpRefund, http_code: u16) -> ErrorResponse {
    ErrorResponse {
        code: response
            .failure_code
            .clone()
            .or_else(|| response.failure_message.clone())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response
            .failure_message
            .clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: response.failure_message.clone(),
        status_code: http_code,
        attempt_status: None,
        connector_transaction_id: response.charge.clone(),
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

fn authorize_uri_redirect(uri: &str) -> Option<Box<RedirectForm>> {
    Some(Box::new(RedirectForm::Form {
        endpoint: uri.to_string(),
        method: common_utils::Method::Get,
        form_fields: HashMap::new(),
    }))
}

/// Resolve the Omise card token / card id for CreateConnectorCustomer.
///
/// `CustomerServiceCreateRequest` has no card field, so gRPC callers put the
/// unused `tokn_…` (or `card_…`) in `connector_feature_data`.
fn extract_customer_card_token(
    request: &ConnectorCustomerData,
    resource_common_data: &PaymentFlowData,
) -> Option<String> {
    if let Some(id) = request.preprocessing_id.as_ref().filter(|s| !s.is_empty()) {
        return Some(id.clone());
    }

    let feature = resource_common_data.connector_feature_data.as_ref()?;
    let raw = feature.peek();

    if let Some(map) = raw.as_object() {
        for key in ["card", "token", "source", "preprocessing_id", "card_token"] {
            if let Some(value) = map.get(key).and_then(|v| v.as_str()) {
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }

    if let Some(s) = raw.as_str() {
        let trimmed = s.trim().trim_matches('"');
        if trimmed.starts_with("tokn_") || trimmed.starts_with("card_") {
            return Some(trimmed.to_string());
        }
    }
    None
}

// ============================================================================
// SHARED WIRE TYPES
// ============================================================================

/// Card block inside a token / charge response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpCard {
    pub id: Option<String>,
    pub object: Option<String>,
    pub brand: Option<String>,
    pub last_digits: Option<String>,
    pub name: Option<String>,
    pub expiration_month: Option<u8>,
    pub expiration_year: Option<u16>,
    pub security_code: Option<String>,
    pub created: Option<String>,
}

/// Charge object — shared by Authorize / PSync / Capture / Void responses.
///
/// `currency` stays a raw string: Omise returns lowercase ISO codes ("usd")
/// while `common_enums::Currency` deserializes UPPERCASE — integrity objects
/// take the request-side currency instead.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpCharge {
    pub id: String,
    pub object: Option<String>,
    pub status: KgpChargeStatus,
    pub amount: Option<i64>,
    pub currency: Option<String>,
    pub authorized: Option<bool>,
    pub paid: Option<bool>,
    pub captured: Option<bool>,
    pub captured_amount: Option<i64>,
    pub authorize_uri: Option<String>,
    pub return_uri: Option<String>,
    pub card: Option<KgpCard>,
    /// Omise `customer` id (`cust_…`) when the charge targeted a saved card.
    pub customer: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub created: Option<String>,
}

// Flow-specific response aliases: the `create_all_prerequisites!` /
// `macro_connector_implementation!` machinery generates a distinct
// `<Response>Templating` marker per response type name, so every flow must
// name a distinct type even when the wire shape is shared (same approach as
// xendit's `RefundResponse` / `RefundResponse as RefundSyncResponse`).
pub type KgpAuthorizeResponse = KgpCharge;
pub type KgpSyncResponse = KgpCharge;
pub type KgpCaptureResponse = KgpCharge;
pub type KgpVoidResponse = KgpCharge;


/// Refund object — shared by Refund / RSync responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpRefund {
    pub id: String,
    pub object: Option<String>,
    pub status: KgpRefundStatus,
    pub amount: Option<i64>,
    pub currency: Option<String>,
    pub charge: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub created: Option<String>,
}

pub type KgpRefundResponse = KgpRefund;
pub type KgpRefundSyncResponse = KgpRefund;

/// Token object — PaymentMethodToken response (`POST {vault}/tokens`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpToken {
    pub id: String,
    pub object: Option<String>,
    pub used: Option<bool>,
    pub card: Option<KgpCard>,
    pub charge: Option<String>,
    pub created: Option<String>,
}

// ============================================================================
// REQUEST TYPES (form-urlencoded)
// ============================================================================

/// `POST {vault}/tokens` — raw card → `tokn_…`.
/// Field names are Omise's literal bracket syntax.
#[derive(Debug, Clone, Serialize)]
pub struct KgpTokenRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(
        rename = "card[name]",
        skip_serializing_if = "Option::is_none"
    )]
    pub card_name: Option<Secret<String>>,
    #[serde(rename = "card[number]")]
    pub card_number: RawCardNumber<T>,
    #[serde(rename = "card[expiration_month]")]
    pub expiration_month: Secret<String>,
    #[serde(rename = "card[expiration_year]")]
    pub expiration_year: Secret<String>,
    #[serde(rename = "card[security_code]")]
    pub security_code: Secret<String>,
}

/// `POST {api}/charges` — create charge from token.
#[derive(Debug, Clone, Serialize)]
pub struct KgpPaymentsRequest {
    /// Integer minor units.
    pub amount: i64,
    pub currency: Currency,
    /// Omise card token (`tokn_…`) or card id (`card_…`).
    pub card: Secret<String>,
    /// Required with `card=card_…`, must be absent with `card=tokn_…`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Secret<String>>,
    /// Required for 3DS — browser land-back after OTP.
    pub return_uri: String,
    /// `true` = auto-capture, `false` = auth-only (pre_auth).
    pub capture: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "metadata[txn_id]", skip_serializing_if = "Option::is_none")]
    pub metadata_txn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// `pre_auth` | `final_auth`. Omitted for standard auto-capture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_type: Option<String>,
    // webhooks are deliberately NOT sent: Omise rejects non-HTTPS /
    // localhost dynamic webhook endpoints with a 400.
}

/// `POST {api}/charges/{id}/capture` — optional partial via `capture_amount`.
#[derive(Debug, Clone, Serialize)]
pub struct KgpCaptureRequest {
    /// Partial capture amount in minor units. Omitted = capture full.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_amount: Option<i64>,
}

/// `POST {api}/charges/{id}/reverse` — uncaptured auth only.
#[derive(Debug, Clone, Serialize)]
pub struct KgpVoidRequest {}

/// `POST {api}/charges/{id}/refunds`.
#[derive(Debug, Clone, Serialize)]
pub struct KgpRefundRequest {
    /// Minor units. Omitted = full remaining refundable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i64>,
    #[serde(rename = "metadata[refund_id]", skip_serializing_if = "Option::is_none")]
    pub metadata_refund_id: Option<String>,
    /// Set `true` to also void an uncaptured charge in one call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub void: Option<bool>,
}

/// `POST {api}/customers` — create a customer and optionally attach a card.
///
/// When `card` is an unused `tokn_…` (or an existing `card_…`), Omise stores it
/// as the customer's default card — the saved-card path from euler-api-gateway.
#[derive(Debug, Clone, Serialize)]
pub struct KgpCustomerRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "metadata[customer_id]", skip_serializing_if = "Option::is_none")]
    pub metadata_customer_id: Option<String>,
    #[serde(rename = "metadata[name]", skip_serializing_if = "Option::is_none")]
    pub metadata_name: Option<String>,
}

/// Customer object — CreateConnectorCustomer response.
///
/// `default_card` is the `card_…` attached at create time.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpCustomer {
    pub id: String,
    pub object: Option<String>,
    pub email: Option<String>,
    pub description: Option<String>,
    pub default_card: Option<String>,
    pub cards: Option<KgpCardList>,
    pub created: Option<String>,
}

pub type KgpCustomerResponse = KgpCustomer;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpCardList {
    pub object: Option<String>,
    pub data: Option<Vec<KgpCard>>,
}

// ============================================================================
// PAYMENT METHOD TOKEN  (POST {vault}/tokens, public key)
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KgpRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for KgpTokenRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: KgpRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card) => Ok(Self {
                card_name: card.card_holder_name.clone(),
                card_number: card.card_number.clone(),
                expiration_month: card.card_exp_month.clone(),
                expiration_year: card.get_expiry_year_4_digit(),
                security_code: card.card_cvc.clone(),
            }),
            _ => Err(IntegrationError::NotSupported {
                message: "Kgp (Omise) tokenization only supports cards".to_string(),
                connector: "kgp",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send PaymentMethodData::Card for PaymentMethodToken.".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Omise Tokens API accepts only raw card details.".to_string(),
                    ),
                },
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            KgpToken,
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
        >,
    > for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            KgpToken,
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let response_integrity_object = Some(PaymentMethodTokenIntegrityObject {
            amount: router_data.request.amount.amount,
            currency: router_data.request.currency,
        });

        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: response.id.clone(),
                connector_payment_method_id: response.card.as_ref().and_then(|c| c.id.clone()),
                status_code: http_code,
            }),
            request: PaymentMethodTokenizationData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            ..router_data
        })
    }
}

// ============================================================================
// AUTHORIZE  (POST {api}/charges)
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KgpRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for KgpPaymentsRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: KgpRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        // 3DS requires return_uri. Fail loud with MissingRequiredField rather
        // than sending a charge Omise will 400.
        let return_uri = request.get_router_return_url()?;

        // Card token from PaymentMethodToken flow. Raw cards must be tokenized
        // first (Omise Tokens API / vault.omise.co).
        let card_token = match request.payment_method_data.clone() {
            PaymentMethodData::PaymentMethodToken(token) => token.token,
            PaymentMethodData::Card(_) => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_token",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Run PaymentMethodToken (POST vault.omise.co/tokens) first and \
                             pass PaymentMethodData::PaymentMethodToken with the tokn_ id."
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.omise.co/tokens-api".to_string()),
                        additional_context: Some(
                            "Omise Charges API only accepts a card token or card id, \
                             never raw PAN."
                                .to_string(),
                        ),
                    },
                }
                .into())
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Kgp (Omise) charges require a card token".to_string(),
                    connector: "kgp",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Tokenize the card via PaymentMethodToken first.".to_string(),
                        ),
                        doc_url: Some("https://docs.omise.co/charges-api".to_string()),
                        additional_context: Some(
                            "Only card token / card id is accepted on POST /charges."
                                .to_string(),
                        ),
                    },
                }
                .into())
            }
        };

        // Omise: `tokn_…` must NOT be paired with `customer`; `card_…` REQUIRES it.
        let customer = if card_token.peek().starts_with("card_") {
            let customer_id = item
                .router_data
                .resource_common_data
                .connector_customer
                .clone()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "connector_customer",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "A saved card id (card_…) must be charged against its Omise \
                             customer (cust_…). Run CreateConnectorCustomer first or pass \
                             connector_customer on the payment."
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.omise.co/charges-api".to_string()),
                        additional_context: Some(
                            "Omise rejects POST /charges with card=card_… unless \
                             customer=cust_… is also present."
                                .to_string(),
                        ),
                    },
                })?;
            Some(Secret::new(customer_id))
        } else {
            None
        };

        let capture = request.is_auto_capture();

        let authorization_type = if capture {
            None
        } else {
            Some("pre_auth".to_string())
        };

        let reference = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            amount: request.amount.amount.get_amount_as_i64(),
            currency: request.currency,
            card: card_token,
            customer,
            return_uri,
            capture,
            description: Some(reference.clone()),
            metadata_txn_id: Some(reference),
            ip: None,
            authorization_type,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            KgpAuthorizeResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            KgpAuthorizeResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let has_authorize_uri = response.authorize_uri.is_some();
        let status = map_charge_status(
            response.status,
            response.authorized.unwrap_or(false),
            response.paid.unwrap_or(false),
            has_authorize_uri,
        );

        let payment_response = if status == AttemptStatus::Failure {
            Err(charge_failure_error(&response, http_code))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: response
                    .authorize_uri
                    .as_deref()
                    .and_then(authorize_uri_redirect),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.description.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        };

        // Authorise integrity tracks the charge amount, not captured_amount
        // (auth-only leaves captured_amount at 0 while amount is the auth).
        let response_integrity_object = Some(AuthoriseIntegrityObject {
            amount: MinorUnit::new(
                response
                    .amount
                    .unwrap_or_else(|| router_data.request.amount.amount.get_amount_as_i64()),
            ),
            currency: router_data.request.currency,
        });

        Ok(Self {
            response: payment_response,
            request: PaymentsAuthorizeData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ============================================================================
// PSYNC  (GET {api}/charges/{id})
// ============================================================================

impl TryFrom<
    ResponseRouterData<
        KgpSyncResponse,
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    >,
> for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData> {
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            KgpSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let has_authorize_uri = response.authorize_uri.is_some();
        let status = map_charge_status(
            response.status,
            response.authorized.unwrap_or(false),
            response.paid.unwrap_or(false),
            has_authorize_uri,
        );

        let payment_response = if status == AttemptStatus::Failure {
            Err(charge_failure_error(&response, http_code))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: response
                    .authorize_uri
                    .as_deref()
                    .and_then(authorize_uri_redirect),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.description.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        };

        let response_integrity_object = Some(PaymentSynIntegrityObject {
            amount: MinorUnit::new(
                response
                    .amount
                    .unwrap_or_else(|| router_data.request.amount.amount.get_amount_as_i64()),
            ),
            currency: router_data.request.currency,
        });

        Ok(Self {
            response: payment_response,
            request: PaymentsSyncData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ============================================================================
// CAPTURE  (POST {api}/charges/{id}/capture)
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KgpRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for KgpCaptureRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: KgpRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        // Partial capture: send capture_amount when it is less than the full auth.
        // Omise treats a missing capture_amount as "capture everything remaining".
        Ok(Self {
            capture_amount: Some(request.amount_to_capture.amount.get_amount_as_i64()),
        })
    }
}

impl TryFrom<
    ResponseRouterData<
        KgpCaptureResponse,
        RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    >,
> for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData> {
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            KgpCaptureResponse,
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let status = map_charge_status(
            response.status,
            response.authorized.unwrap_or(false),
            response.paid.unwrap_or(false),
            false,
        );

        let payment_response = if status == AttemptStatus::Failure {
            Err(charge_failure_error(&response, http_code))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.description.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        };

        // Capture integrity is the amount actually captured (partial or full).
        let response_integrity_object = Some(CaptureIntegrityObject {
            amount_to_capture: MinorUnit::new(
                response
                    .captured_amount
                    .or(response.amount)
                    .unwrap_or_else(|| {
                        router_data.request.amount_to_capture.amount.get_amount_as_i64()
                    }),
            ),
            currency: router_data.request.currency,
        });

        Ok(Self {
            response: payment_response,
            request: PaymentsCaptureData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ============================================================================
// VOID  (POST {api}/charges/{id}/reverse) — uncaptured only
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KgpRouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>,
    > for KgpVoidRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        _item: KgpRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // reverse takes no body — form-urlencoded empty object.
        Ok(Self {})
    }
}

impl TryFrom<
    ResponseRouterData<
        KgpVoidResponse,
        RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    >,
> for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData> {
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            KgpVoidResponse,
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // A failed reverse is NOT a void: do not mark the charge Voided.
        let status = match response.status {
            KgpChargeStatus::Reversed => AttemptStatus::Voided,
            // reverse did not apply — charge is still captured / successful
            KgpChargeStatus::Successful => AttemptStatus::Charged,
            KgpChargeStatus::Pending | KgpChargeStatus::Unknown => AttemptStatus::Pending,
            KgpChargeStatus::Failed | KgpChargeStatus::Expired => AttemptStatus::Failure,
        };

        let payment_response = if matches!(
            response.status,
            KgpChargeStatus::Failed | KgpChargeStatus::Expired
        ) {
            Err(charge_failure_error(&response, http_code))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.description.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        };

        let response_integrity_object = Some(PaymentVoidIntegrityObject {
            connector_transaction_id: response.id.clone(),
        });

        Ok(Self {
            response: payment_response,
            request: PaymentVoidData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ============================================================================
// REFUND  (POST {api}/charges/{id}/refunds)
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KgpRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for KgpRefundRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: KgpRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        Ok(Self {
            amount: Some(request.refund_amount.amount.get_amount_as_i64()),
            metadata_refund_id: Some(request.refund_id.clone()),
            void: None,
        })
    }
}

impl TryFrom<
    ResponseRouterData<
        KgpRefundResponse,
        RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    >,
> for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData> {
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            KgpRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let refund_status = map_refund_status(response.status);

        let refund_response = if refund_status == RefundStatus::Failure {
            Err(refund_failure_error(&response, http_code))
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status,
                status_code: http_code,
                acquirer_reference_number: None,
            })
        };

        let response_integrity_object = Some(RefundIntegrityObject {
            refund_amount: MinorUnit::new(response.amount.unwrap_or_else(|| {
                router_data
                    .request
                    .refund_amount
                    .amount
                    .get_amount_as_i64()
            })),
            currency: router_data.request.currency,
        });

        Ok(Self {
            response: refund_response,
            request: RefundsData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            ..router_data
        })
    }
}

// ============================================================================
// RSYNC  (GET {api}/charges/{charge_id}/refunds/{refund_id})
// ============================================================================

impl TryFrom<
    ResponseRouterData<
        KgpRefundSyncResponse,
        RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    >,
> for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData> {
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            KgpRefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let refund_status = map_refund_status(response.status);

        let refund_response = if refund_status == RefundStatus::Failure {
            Err(refund_failure_error(&response, http_code))
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status,
                status_code: http_code,
                acquirer_reference_number: None,
            })
        };

        let response_integrity_object = Some(RefundSyncIntegrityObject {
            connector_transaction_id: response.charge.clone().unwrap_or_default(),
            connector_refund_id: response.id.clone(),
        });

        Ok(Self {
            response: refund_response,
            request: RefundSyncData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            ..router_data
        })
    }
}

// ============================================================================
// CREATE CONNECTOR CUSTOMER  (POST {api}/customers)
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KgpRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for KgpCustomerRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: KgpRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        // Saved-card attach: an unused `tokn_…` (or existing `card_…`) is stored
        // as the customer's default card. Carriers, in order:
        //   1. `preprocessing_id` (internal callers / stripe's `source` pattern)
        //   2. `connector_feature_data` JSON key `card` / `token` / `source` /
        //      `preprocessing_id` (gRPC CustomerServiceCreateRequest — the proto
        //      has no card field, so this is the only public carrier)
        //   3. `connector_feature_data` as a raw `tokn_…` / `card_…` string
        let card = extract_customer_card_token(request, &item.router_data.resource_common_data)
            .map(Secret::new);

        // Omise `email` is a plain form field. `ConnectorCustomerData.email` is
        // `Option<Secret<Email>>`; unwrap Secret → Email → Secret<String> → String.
        let email = request.email.as_ref().map(|secret_email| {
            let email = secret_email.peek().clone();
            let inner: Secret<String, common_utils::pii::EmailStrategy> = email.expose();
            inner.expose()
        });

        Ok(Self {
            card,
            description: request.description.clone(),
            email,
            metadata_customer_id: request
                .customer_id
                .as_ref()
                .map(|id| id.peek().clone()),
            metadata_name: request.name.as_ref().map(|name| name.peek().clone()),
        })
    }
}

impl TryFrom<ResponseRouterData<KgpCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<KgpCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // Keep the default card id on the request so the caller can store it.
        let request = if response.default_card.is_some() || response.cards.is_some() {
            ConnectorCustomerData {
                preprocessing_id: response
                    .default_card
                    .clone()
                    .or_else(|| {
                        response
                            .cards
                            .as_ref()
                            .and_then(|list| list.data.as_ref())
                            .and_then(|cards| cards.first())
                            .and_then(|card| card.id.clone())
                    }),
                ..router_data.request
            }
        } else {
            router_data.request
        };

        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: response.id.clone(),
                status_code: http_code,
            }),
            request,
            ..router_data
        })
    }
}
// ============================================================================
// WEBHOOKS
// ============================================================================

/// Parsed `Omise-Signature` header: `t=<unix>,s=<hex hmac>`.
#[derive(Debug, Clone)]
pub struct KgpWebhookSignature {
    pub timestamp: String,
    pub signature: String,
}

pub fn parse_kgp_signature(header_value: &str) -> Option<KgpWebhookSignature> {
    let mut timestamp = None;
    let mut signature = None;
    for part in header_value.split(',') {
        let mut kv = part.trim().splitn(2, '=');
        let key = kv.next()?;
        let value = kv.next()?;
        match key {
            "t" => timestamp = Some(value.to_string()),
            "s" => signature = Some(value.to_string()),
            _ => {}
        }
    }
    Some(KgpWebhookSignature {
        timestamp: timestamp?,
        signature: signature?,
    })
}

/// Minimal webhook envelope Omise POSTs to the merchant endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpWebhookEvent {
    pub object: Option<String>,
    pub key: Option<String>,
    pub id: Option<String>,
    pub created: Option<String>,
    pub data: Option<KgpWebhookData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KgpWebhookData {
    pub id: Option<String>,
    pub object: Option<String>,
    pub status: Option<String>,
    pub amount: Option<i64>,
    pub currency: Option<String>,
    pub authorized: Option<bool>,
    pub paid: Option<bool>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub charge: Option<String>,
}

/// Build the typed webhook reference Omise's payload maps onto.
pub fn kgp_webhook_reference(event: &KgpWebhookEvent) -> Option<WebhookResourceReference> {
    let key = event.key.clone().unwrap_or_default();
    let data_id = event.data.as_ref().and_then(|d| d.id.clone());
    let charge_id = event.data.as_ref().and_then(|d| d.charge.clone());

    if key.starts_with("refund") {
        Some(WebhookResourceReference::Refund(RefundWebhookReference {
            connector_refund_id: data_id,
            connector_transaction_id: charge_id,
            merchant_transaction_id: None,
            merchant_refund_id: None,
        }))
    } else {
        Some(WebhookResourceReference::Payment(PaymentWebhookReference {
            connector_transaction_id: data_id,
            merchant_transaction_id: None,
        }))
    }
}
