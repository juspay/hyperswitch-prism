use std::str::FromStr;

use common_enums::{AttemptStatus, Currency};
use common_utils::{
    crypto::{self, GenerateDigest},
    pii::Email,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RawConnectorStatus, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
    },
    errors::{self, ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::{PaymentSynIntegrityObject, RefundIntegrityObject},
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::reddot::{ReddotAmountConvertor, ReddotRouterData},
    types::ResponseRouterData,
    utils::get_unimplemented_payment_method_error_message,
};

/// SOP (merchant-collected card data) redirect mode — the only mode this
/// integration wires.
const RDP_API_MODE_SOP: &str = "redirection_sop";
/// `S` = Sale (authorize + automatic capture). `A` (pre-auth / manual capture)
/// is intentionally not implemented until a Capture flow exists.
const RDP_PAYMENT_TYPE_SALE: &str = "S";
const RDP_ORDER_ID_GENERATED_LEN: usize = 16;

/// Key inside the `connector_feature_data` JSON blob under which the
/// persisted order_id lives between authorize and refund.
pub(crate) const RDP_ORDER_ID_BLOB_KEY: &str = "reddot_order_id";

/// Mint a random order_id: 16 lowercase hex chars (= 8 random bytes);
/// RDP's enforced ceiling is 20 (it rejects 22 with `-1014`), so 16 sits
/// comfortably underneath.
fn gen_reddot_order_id() -> String {
    let oid = hex::encode(rand::random::<[u8; 8]>());
    // 8 bytes always hex-encode to 16 chars; keep the assertion so the
    // constant stays load-bearing at the generator, not only in tests.
    debug_assert_eq!(oid.len(), RDP_ORDER_ID_GENERATED_LEN);
    oid
}

/// Read the persisted order_id back on the refund path from the
/// `connector_feature_data` blob (the merged `gateway_auth_req_params`
/// contents that euler hands back verbatim).
fn reddot_order_id_from_blob(
    connector_feature_data: &Option<common_utils::pii::SecretSerdeValue>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    connector_feature_data
        .as_ref()
        .and_then(|cfd| cfd.peek().get(RDP_ORDER_ID_BLOB_KEY))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "connector_feature_data.reddot_order_id",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "reddot_order_id missing from second_factor.gateway_auth_req_params."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Re-authorize the payment or refund via RDP's console using the connector_transaction_id"
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.reddotpayment.com/merchant/#capture-refund-void"
                            .to_string(),
                    ),
                },
            }
            .into()
        })
}

/// Red Dot Payment (RDP) credentials.
///
/// AUTH: BodyKey — `mid` is the RDP merchant id, `secret` is the
/// RDP secret key used to build request signatures. RDP sends no auth headers;
/// credentials travel inside the JSON request body only.
#[derive(Debug, Clone)]
pub struct ReddotAuthType {
    pub mid: Secret<String>,
    pub secret: Secret<String>,
    /// MGA account detail — string-typed as stored in `account_details`:
    /// when `"true"` (CyberSource-acquired MID), bill_to_* fields are
    /// mandatory and must be sent on payment-api requests.
    pub acquirer_cybersource: String,
}

impl ReddotAuthType {
    /// MGA flag is a string in `account_details` ("true"/"false").
    fn is_cybersource_acquired(&self) -> bool {
        self.acquirer_cybersource.eq_ignore_ascii_case("true")
    }
}

impl TryFrom<&ConnectorSpecificConfig> for ReddotAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Reddot {
                mid,
                secret,
                acquirer_cybersource,
                ..
            } => Ok(Self {
                mid: mid.to_owned(),
                secret: secret.to_owned(),
                acquirer_cybersource: acquirer_cybersource.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "reddot connector config must contain `mid` and `secret` under `Reddot` in x-connector-config"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Fix the merchant gateway account credentials JSON."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://developers.reddotpayment.com/merchant/#capture-refund-void"
                                .to_string(),
                        ),
                    },
                }
            )),
        }
    }
}

/// RDP error responses are unsigned JSON bodies:
/// `{"response_status": "error", "response_code": "<string>", "response_msg": "<string>"}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReddotErrorResponse {
    pub response_status: String,
    pub response_code: String,
    pub response_msg: String,
}

impl ReddotErrorResponse {
    /// Synthesize an error from a non-JSON body — RDP's edge WAF (Imperva
    /// / Incapsula) answers IP-blocked traffic with `403 + full HTML page`,
    /// so a strict JSON parse must not blow up. Mirrors the datatrans
    /// fallback: status code becomes the `code`, body text (truncated,
    /// decoded lossily) becomes the `message`.
    pub fn from_non_json_body(status_code: u16, body: &[u8]) -> Self {
        let text = String::from_utf8_lossy(body);
        Self {
            response_status: "ERROR".to_string(),
            response_code: status_code.to_string(),
            response_msg: text.trim().chars().take(200).collect(),
        }
    }
}

/// RDP Redirect API (SOP mode) authorize request body — the exact minimal
/// verified-working field contract for `POST /service/payment-api`.
#[derive(Debug, Serialize)]
pub struct ReddotAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub mid: Secret<String>,
    pub api_mode: String,
    pub payment_type: String,
    pub order_id: String,
    pub amount: StringMajorUnit,
    pub ccy: String,
    pub card_no: RawCardNumber<T>,
    /// Card expiry as `MMYYYY` — zero-padded month followed by 4-digit year
    /// (RDP rejects any other format with `-1906`).
    pub exp_date: Secret<String>,
    pub cvv2: Secret<String>,
    pub payer_name: Secret<String>,
    pub payer_email: Email,
    pub redirect_url: String,
    pub notify_url: String,
    /// Free-text reference RDP echoes verbatim in query_redirection and the
    /// notification webhook — carries the FULL Juspay txnId (1092-char-safe)
    /// so euler's tracker integrity check / recon can key off it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_reference: Option<String>,
    pub signature: Secret<String>,
    // bill_to_* — sent only when the MGA account detail `acquirer_cybersource`
    // is true (mandatory for CyberSource-acquired MIDs, harmless otherwise).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_forename: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_surname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_address_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_address_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_address_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_address_postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to_phone: Option<Secret<String>>,
}

/// Payment signature input, in the exact concatenation order mandated by RDP.
struct PaymentSignatureInput<'a> {
    mid: &'a str,
    order_id: &'a str,
    payment_type: &'a str,
    amount: &'a str,
    ccy: &'a str,
    card_no: &'a str,
    exp_date: &'a str,
    cvv2: &'a str,
    secret_key: &'a str,
}

/// RDP payment signature for `/service/payment-api`: SHA-512 hex over
/// `mid + order_id + payment_type + amount + ccy + (card_no first6+last4)
/// + exp_date (MMYYYY) + (cvv2 last digit) + secret_key`.
///
/// `amount`, `exp_date` and `card_no`/`cvv2` contributions MUST use the exact
/// values sent in the outgoing JSON body.
fn compute_payment_signature(input: PaymentSignatureInput<'_>) -> Result<String, IntegrationError> {
    use sha2::{Digest, Sha512};

    let card_details_ctx = |field: &str| errors::IntegrationErrorContext {
        additional_context: Some(format!("RDP payment signature needs slices of {field}")),
        suggested_action: Some(format!(
            "Ensure {field} is a valid value ({})",
            if field.contains("card_number") {
                "card number with at least 10 digits"
            } else {
                "non-empty CVC"
            }
        )),
        doc_url: Some("https://developers.reddotpayment.com/redirect/#payment-api".to_string()),
    };

    let card_first6 = input
        .card_no
        .get(..6)
        .ok_or(IntegrationError::InvalidDataFormat {
            field_name: "payment_method_data.card.card_number",
            context: card_details_ctx("payment_method_data.card.card_number"),
        })?;
    let card_last4 = input
        .card_no
        .get(input.card_no.len().saturating_sub(4)..)
        .ok_or(IntegrationError::InvalidDataFormat {
            field_name: "payment_method_data.card.card_number",
            context: card_details_ctx("payment_method_data.card.card_number"),
        })?;
    let cvv_last_digit = input.cvv2.get(input.cvv2.len().saturating_sub(1)..).ok_or(
        IntegrationError::InvalidDataFormat {
            field_name: "payment_method_data.card.card_cvc",
            context: card_details_ctx("payment_method_data.card.card_cvc"),
        },
    )?;

    let concatenated = [
        input.mid,
        input.order_id,
        input.payment_type,
        input.amount,
        input.ccy,
        card_first6,
        card_last4,
        input.exp_date,
        cvv_last_digit,
        input.secret_key,
    ]
    .concat();

    let mut hasher = Sha512::new();
    hasher.update(concatenated.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// RDP generic signature (used by `query_redirection`/PSync requests and to
/// sign/verify responses and notifications): drop `signature`, ASCII-sort the
/// remaining keys alphabetically, concatenate their values in that order,
/// append `secret_key`, SHA-512 hex.
///
/// This is a DIFFERENT algorithm from [`compute_payment_signature`] — do not
/// conflate them. The input must NOT contain a `signature` entry.
///
/// `pub` so the future webhook/response signature verification (implemented on
/// the `Reddot` connector struct in the parent module) can reuse it —
/// [`compute_payment_signature`] stays private as it is only needed to build
/// the Authorize request body.
pub fn compute_generic_signature(mut params: Vec<(&str, &str)>, secret_key: &str) -> String {
    use sha2::{Digest, Sha512};

    params.sort_by_key(|(key_a, _)| *key_a);
    let mut concatenated: String = params.into_iter().map(|(_, value)| value).collect();
    concatenated.push_str(secret_key);

    let mut hasher = Sha512::new();
    hasher.update(concatenated.as_bytes());
    hex::encode(hasher.finalize())
}

/// bill_to_* block of [`ReddotAuthorizeRequest`], sourced from the shared
/// billing getters (unified `payment.billing` + `payment_method_data.billing`).
#[derive(Default)]
struct ReddotBillTo {
    forename: Option<Secret<String>>,
    surname: Option<Secret<String>>,
    address_line1: Option<Secret<String>>,
    address_city: Option<Secret<String>>,
    address_country: Option<String>,
    address_postal_code: Option<Secret<String>>,
    phone: Option<Secret<String>>,
}

/// CyberSource-acquired MIDs are billed by RDP with a mandatory billing
/// block, so a missing billing address is a hard error; individual
/// subfields stay optional and are simply omitted when absent.
fn get_bill_to(
    resource_data: &PaymentFlowData,
) -> Result<ReddotBillTo, error_stack::Report<IntegrationError>> {
    resource_data.get_billing_address()?;
    Ok(ReddotBillTo {
        forename: resource_data.get_optional_billing_first_name(),
        surname: resource_data.get_optional_billing_last_name(),
        address_line1: resource_data.get_optional_billing_line1(),
        address_city: resource_data.get_optional_billing_city(),
        address_country: resource_data
            .get_optional_billing_country()
            .map(|country| country.to_string()),
        address_postal_code: resource_data.get_optional_billing_zip(),
        phone: resource_data.get_optional_billing_phone_number(),
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ReddotRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ReddotAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ReddotRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // This integration only wires payment_type = "S" (sale / auto-capture).
        if !router_data.request.is_auto_capture() {
            Err(IntegrationError::CaptureMethodNotSupported {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "reddot supports only automatic capture (payment_type = \"S\"); manual capture (payment_type = \"A\") is not implemented yet"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Initiate the payment in automatic capture mode".to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.reddotpayment.com/redirect/#payment-api".to_string(),
                    ),
                },
            })?;
        }

        let auth = ReddotAuthType::try_from(&router_data.connector_config)?;
        let amount = ReddotAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;
        let redirect_url = router_data.request.get_router_return_url()?;
        let notify_url = router_data.request.get_webhook_url()?;
        let payer_email = router_data.request.get_email()?;
        let ccy = router_data.request.currency.to_string();
        // Juspay txnUuids (19-21 chars) overflow RDP's 20-char `order_id`
        // ceiling, so the connector mints its own random 16-char hex id here
        // (euler never learns it). It returns to euler inside the Authorize
        // response `connector_metadata` (→ second_factor blob) and the refund
        // request hands it back via `connector_feature_data`.
        // `merchant_reference` carries the FULL Juspay txnId — RDP echoes it
        // in query_redirection + webhook payloads, feeding euler's txn-id
        // integrity check + recon.
        let order_id = gen_reddot_order_id();
        let merchant_reference = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Mandatory `payer_name` fallback chain (mirrors the email chain):
                //   1. card_holder_name as collected (skip Juspay's sentinel
                //      "name" that euler's TempCard backfills when missing)
                //   2. Customer record's name (euler composes first+last into
                //      UCS CustomerDetails.name)
                //   3. literal "name" — RDP requires the field non-empty
                let payer_name = card_data
                    .card_holder_name
                    .clone()
                    .map(|name| name.peek().clone())
                    .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("name"))
                    .or_else(|| {
                        router_data
                            .request
                            .customer_name
                            .clone()
                            .map(|customer_name| customer_name.trim().to_string())
                            .filter(|customer_name| !customer_name.is_empty())
                    })
                    .map(Secret::new)
                    .unwrap_or_else(|| Secret::new("name".to_string()));
                let exp_month = card_data.get_card_expiry_month_2_digit()?;
                let exp_year = card_data.get_expiry_year_4_digit();
                let exp_date = Secret::new(format!("{}{}", exp_month.peek(), exp_year.peek()));
                let amount_str = amount.get_amount_as_string();

                let signature = compute_payment_signature(PaymentSignatureInput {
                    mid: auth.mid.peek(),
                    order_id: &order_id,
                    payment_type: RDP_PAYMENT_TYPE_SALE,
                    amount: &amount_str,
                    ccy: &ccy,
                    card_no: card_data.card_number.peek(),
                    exp_date: exp_date.peek(),
                    cvv2: card_data.card_cvc.peek(),
                    secret_key: auth.secret.peek(),
                })?;

                // bill_to_* only for CyberSource-acquired MIDs (MGA account
                // detail flag); for those accounts billing is mandatory, so a
                // missing billing block is a hard error with a clear message.
                let bill_to = if auth.is_cybersource_acquired() {
                    Some(get_bill_to(&router_data.resource_common_data)?)
                } else {
                    None
                };
                let ReddotBillTo {
                    forename: bill_to_forename,
                    surname: bill_to_surname,
                    address_line1: bill_to_address_line1,
                    address_city: bill_to_address_city,
                    address_country: bill_to_address_country,
                    address_postal_code: bill_to_address_postal_code,
                    phone: bill_to_phone,
                } = bill_to.unwrap_or_default();

                Ok(Self {
                    mid: auth.mid,
                    api_mode: RDP_API_MODE_SOP.to_string(),
                    payment_type: RDP_PAYMENT_TYPE_SALE.to_string(),
                    order_id,
                    amount,
                    ccy,
                    card_no: card_data.card_number.clone(),
                    exp_date,
                    cvv2: card_data.card_cvc.clone(),
                    payer_name,
                    payer_email,
                    redirect_url,
                    notify_url,
                    merchant_reference: Some(merchant_reference),
                    signature: Secret::new(signature),
                    bill_to_forename,
                    bill_to_surname,
                    bill_to_address_line1,
                    bill_to_address_city,
                    bill_to_address_country,
                    bill_to_address_postal_code,
                    bill_to_phone,
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message("reddot"),
                errors::IntegrationErrorContext {
                    additional_context: Some("Only card payments are wired for reddot".to_string()),
                    suggested_action: Some(
                        "Retry with payment_method = card (SOP redirect)".to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.reddotpayment.com/redirect/#payment-api".to_string(),
                    ),
                },
            )
            .into()),
        }
    }
}

/// RDP serializes `response_code` as a JSON number in the success body (`0`)
/// and as a JSON string in error bodies (`"-1003"`).
fn deserialize_response_code<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;

    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(code) => Ok(code),
        serde_json::Value::Number(code) => Ok(code.to_string()),
        other => Err(D::Error::custom(format!(
            "unexpected type for response_code: {other}"
        ))),
    }
}

/// `POST /service/payment-api` response. Success bodies carry
/// `transaction_id` + `payment_url`; error bodies carry only
/// `response_status`/`response_code`/`response_msg`, so every other field is
/// optional. The synchronous response never reports the final payment outcome
/// (that arrives asynchronously via `notify_url`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReddotAuthorizeResponse {
    pub mid: Option<String>,
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub payment_url: Option<String>,
    pub created_timestamp: Option<i64>,
    pub expired_timestamp: Option<i64>,
    #[serde(deserialize_with = "deserialize_response_code")]
    pub response_code: String,
    pub response_msg: Option<String>,
    pub response_status: Option<String>,
    pub signature: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ReddotAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ReddotAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let raw_connector_response = serde_json::to_string(&response).ok().map(Secret::new);
        // Surface RDP's response_code/response_msg as rawConnectorStatus so the
        // caller (Euler) can persist the PG resp_code/resp_message from the data body.
        let raw_connector_status = Some(RawConnectorStatus {
            code: Some(response.response_code.clone()),
            message: response.response_msg.clone(),
            reason: None,
        });

        let (status, payment_response) = match response.response_code.as_str() {
            // Request accepted; the customer must complete 3DS at payment_url.
            // NOT a payment success — the outcome arrives via notify_url.
            "0" => {
                let transaction_id = response.transaction_id.ok_or_else(|| {
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some("reddot: success response missing transaction_id".to_string()),
                    )
                })?;
                let payment_url = response.payment_url.ok_or_else(|| {
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some("reddot: success response missing payment_url".to_string()),
                    )
                })?;
                (
                    AttemptStatus::AuthenticationPending,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                        redirection_data: Some(Box::new(RedirectForm::Uri { uri: payment_url })),
                        mandate_reference: None,
                        // The generated order_id goes back to euler inside
                        // connector_metadata; euler merges this connector-
                        // feature-data blob into second_factor.gateway_auth_
                        // req_params, and the refund request hands it right
                        // back. Refund's `order_number` needs the identical
                        // value (see reddot_order_id_from_blob).
                        connector_metadata: response
                            .order_id
                            .clone()
                            .map(|oid| serde_json::json!({ RDP_ORDER_ID_BLOB_KEY: oid })),
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: response.order_id.clone(),
                        incremental_authorization_allowed: None,
                        splits: None,
                        payment_account_reference: None,
                        status_code: http_code,
                    }),
                )
            }
            // Any other response_code is a synchronous failure (unsigned error body).
            // `-7995`/`-7997` are 3DS-auth failures → AUTHENTICATION_FAILED;
            // everything else (request/validation/decline) → AUTHORIZATION_FAILED
            // (mirrors maya's AuthFailed vs PaymentFailed split).
            error_code => {
                let attempt_status = match error_code {
                    "-7995" | "-7997" => AttemptStatus::AuthenticationFailed,
                    _ => AttemptStatus::AuthorizationFailed,
                };
                let response_msg = response.response_msg;
                (
                    attempt_status,
                    Err(ErrorResponse {
                        code: error_code.to_string(),
                        status_code: http_code,
                        message: response_msg.clone().unwrap_or_default(),
                        reason: response_msg,
                        attempt_status: None,
                        connector_transaction_id: response.transaction_id,
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        // Same body as the success arm — keep raw on the
                        // failure path too (flywire/kount convention).
                        raw_connector_response: raw_connector_response.clone(),
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
        };

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status,
                raw_connector_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// =============================================================================
// PSYNC FLOW (POST /service/Merchant_processor/query_redirection)
// =============================================================================

/// RDP redirection-enquiry (PSync) request body — exactly these 3 fields, no
/// others. `signature` is the GENERIC signature over the two data keys
/// (ASCII-sorted: `request_mid`, `transaction_id`), NOT the payment signature
/// used by `/service/payment-api`.
#[derive(Debug, Serialize)]
pub struct ReddotPSyncRequest {
    /// Merchant ID — the same credential value as `mid` (this API names the
    /// field `request_mid`).
    pub request_mid: Secret<String>,
    /// RDP transaction id returned by the Authorize response
    /// (`connector_transaction_id`).
    pub transaction_id: String,
    pub signature: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ReddotRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for ReddotPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ReddotRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let auth = ReddotAuthType::try_from(&router_data.connector_config)?;
        let transaction_id = router_data.request.get_connector_transaction_id()?;
        let signature = compute_generic_signature(
            vec![
                ("request_mid", auth.mid.peek().as_str()),
                ("transaction_id", transaction_id.as_str()),
            ],
            auth.secret.peek(),
        );

        Ok(Self {
            request_mid: auth.mid,
            transaction_id,
            signature: Secret::new(signature),
        })
    }
}

/// `POST /service/Merchant_processor/query_redirection` response. BOTH the
/// success body (`response_code == "0"`) and final-failure bodies (e.g.
/// `-1`, `-7997`) are signed data responses carrying the full transaction
/// record, while processing errors (e.g. `-1003` invalid signature) are the
/// unsigned 3-field bodies `{response_status, response_code, response_msg}` —
/// so every field except `response_code` is optional.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReddotPSyncResponse {
    pub transaction_id: Option<String>,
    pub order_id: Option<String>,
    /// RDP echoes the authorize-time `merchant_reference` (the full Juspay
    /// txnId) here — primary integrity/recon cross-check key.
    pub merchant_reference: Option<String>,
    pub authorized_amount: Option<StringMajorUnit>,
    pub authorized_ccy: Option<String>,
    #[serde(deserialize_with = "deserialize_response_code")]
    pub response_code: String,
    pub response_msg: Option<String>,
    pub response_status: Option<String>,
    // ---- remaining fields of RDP's signed transaction record ----
    // Modeled (all optional) so the typed round-trip in
    // `raw_connector_response` recreates the full body — orderstatus's
    // `gateway_response` and any strmap-key lookups (e.g. `mid`) depend
    // on these keys surviving. Mirrors how maya's webhook-body struct
    // models the entire Maya response.
    pub mid: Option<String>,
    pub request_mid: Option<String>,
    pub request_amount: Option<String>,
    pub request_ccy: Option<String>,
    pub acquirer_transaction_id: Option<String>,
    pub acquirer_authorized_amount: Option<String>,
    pub acquirer_authorized_ccy: Option<String>,
    pub acquirer_response_code: Option<String>,
    pub acquirer_response_msg: Option<String>,
    pub acquirer_authorization_code: Option<String>,
    pub acquirer_mpi_eci: Option<String>,
    pub created_timestamp: Option<String>,
    pub acquirer_created_timestamp: Option<String>,
    pub request_timestamp: Option<String>,
    pub first_6: Option<String>,
    pub last_4: Option<String>,
    pub payer_name: Option<String>,
    pub payer_id: Option<String>,
    pub transaction_type: Option<String>,
    pub payment_mode: Option<String>,
    pub signature: Option<String>,
}

impl TryFrom<ResponseRouterData<ReddotPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<ReddotPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let raw_connector_response = serde_json::to_string(&response).ok().map(Secret::new);
        // Surface RDP's response_code/response_msg as rawConnectorStatus so the
        // caller (Euler) can persist the PG resp_code/resp_message from the data body.
        let raw_connector_status = Some(RawConnectorStatus {
            code: Some(response.response_code.clone()),
            message: response.response_msg.clone(),
            reason: None,
        });

        // Echo RDP's authorized amount/currency through the money framework into
        // the PSync integrity object, but only when both are present (RDP omits
        // them on non-final states). Same shape as maya's PSync transform.
        let integrity_object = match (
            response.authorized_amount.clone(),
            response.authorized_ccy.clone(),
        ) {
            (Some(authorized_amount), Some(authorized_ccy)) => {
                let currency = Currency::from_str(&authorized_ccy).map_err(|_| {
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some(format!(
                            "reddot: PSync response has unknown authorized_ccy: {authorized_ccy}"
                        )),
                    )
                })?;
                let amount = ReddotAmountConvertor::convert_back(authorized_amount, currency)
                    .change_context(ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some(
                            "reddot: failed to parse authorized_amount from PSync response"
                                .to_string(),
                        ),
                    ))?;
                Some(PaymentSynIntegrityObject { amount, currency })
            }
            _ => None,
        };

        let (status, minor_amount_captured, payment_response) = match response
            .response_code
            .as_str()
        {
            // Payment successful (final). Every transaction this integration
            // creates is transaction_type == "S" (sale / auto-capture), so a
            // successful result means the funds are captured → Charged.
            "0" => {
                let transaction_id = response.transaction_id.ok_or_else(|| {
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some("reddot: PSync success response missing transaction_id".to_string()),
                    )
                })?;
                // Carry the amount/currency reported by RDP as the captured
                // amount (authorized_amount / authorized_ccy).
                let minor_amount_captured = match (
                    response.authorized_amount,
                    response.authorized_ccy,
                ) {
                    (Some(authorized_amount), Some(authorized_ccy)) => {
                        let currency = Currency::from_str(&authorized_ccy)
                                .map_err(|_| {
                                    ConnectorError::response_handling_failed_with_context(
                                        http_code,
                                        Some(format!(
                                            "reddot: PSync success response has unknown authorized_ccy: {authorized_ccy}"
                                        )),
                                    )
                                })?;
                        Some(
                                ReddotAmountConvertor::convert_back(
                                    authorized_amount,
                                    currency,
                                )
                                .change_context(
                                    ConnectorError::response_handling_failed_with_context(
                                        http_code,
                                        Some(
                                            "reddot: failed to parse authorized_amount from PSync success response"
                                                .to_string(),
                                        ),
                                    ),
                                )?,
                            )
                    }
                    _ => None,
                };
                (
                    AttemptStatus::Charged,
                    minor_amount_captured,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        // RDP echoes our full Juspay txnId in
                        // `merchant_reference` — THAT is the tracker-
                        // integrity/recon key euler compares against
                        // (verifyTxnId passes via the txnId clause).
                        // Fall back to `order_id` (capped uuid) when the
                        // acquirer doesn't round-trip merchant_reference.
                        connector_response_reference_id: response
                            .merchant_reference
                            .clone()
                            .or(response.order_id.clone()),
                        incremental_authorization_allowed: None,
                        splits: None,
                        payment_account_reference: None,
                        status_code: http_code,
                    }),
                )
            }
            // Pending — the final state has not been reached yet.
            "-01" => {
                let transaction_id = response.transaction_id.ok_or_else(|| {
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some("reddot: PSync pending response missing transaction_id".to_string()),
                    )
                })?;
                (
                    AttemptStatus::Pending,
                    None,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: response
                            .merchant_reference
                            .clone()
                            .or(response.order_id.clone()),
                        incremental_authorization_allowed: None,
                        splits: None,
                        payment_account_reference: None,
                        status_code: http_code,
                    }),
                )
            }
            // Terminal failure; response_msg carries the reason.
            // Status mapping mirrors maya: `-7995`/`-7997` (3DS-auth
            // failures) → AUTHENTICATION_FAILED; everything else
            // (`-1` bank/acquirer decline, `-1003` signature/validation,
            // ...) → AUTHORIZATION_FAILED.
            error_code => {
                let attempt_status = match error_code {
                    "-7995" | "-7997" => AttemptStatus::AuthenticationFailed,
                    _ => AttemptStatus::AuthorizationFailed,
                };
                let response_msg = response.response_msg;
                (
                    attempt_status,
                    None,
                    Err(ErrorResponse {
                        code: error_code.to_string(),
                        status_code: http_code,
                        message: response_msg.clone().unwrap_or_default(),
                        reason: response_msg,
                        attempt_status: None,
                        connector_transaction_id: response.transaction_id,
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: raw_connector_response.clone(),
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
        };

        let amount_captured = minor_amount_captured.map(|amount| amount.get_amount_as_i64());
        Ok(Self {
            response: payment_response,
            request: PaymentsSyncData {
                integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                minor_amount_captured,
                raw_connector_status,
                raw_connector_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// =============================================================================
// WEBHOOK TYPES (notify_url callback)
// =============================================================================

/// RDP notify_url callback body. Same field shapes as the query_redirection
/// data response (`response_code` serialized the same string-or-number way),
/// so the status mapping intentionally mirrors the PSync transform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReddotWebhookBody {
    pub mid: Option<String>,
    /// Merchant reference we sent at authorize (Juspay txnUuid for REDDOT),
    /// echoed back by RDP.
    pub order_id: Option<String>,
    /// RDP's gateway-side id for this payment (`transaction_id`).
    pub transaction_id: Option<String>,
    /// Echo of the authorize-time `merchant_reference` (full Juspay txnId).
    pub merchant_reference: Option<String>,
    #[serde(deserialize_with = "deserialize_response_code")]
    pub response_code: String,
    pub response_msg: Option<String>,
    pub authorized_amount: Option<StringMajorUnit>,
    pub authorized_ccy: Option<String>,
    pub response_status: Option<String>,
    // ---- remaining fields of the notify_url record (same shape as the
    // query_redirection data response) — modeled so the webhook's
    // `raw_connector_response` round-trip is lossless, like the psync one.
    pub request_mid: Option<String>,
    pub request_amount: Option<String>,
    pub request_ccy: Option<String>,
    pub acquirer_transaction_id: Option<String>,
    pub acquirer_authorized_amount: Option<String>,
    pub acquirer_authorized_ccy: Option<String>,
    pub acquirer_response_code: Option<String>,
    pub acquirer_response_msg: Option<String>,
    pub acquirer_authorization_code: Option<String>,
    pub acquirer_mpi_eci: Option<String>,
    pub created_timestamp: Option<String>,
    pub acquirer_created_timestamp: Option<String>,
    pub request_timestamp: Option<String>,
    pub first_6: Option<String>,
    pub last_4: Option<String>,
    pub payer_name: Option<String>,
    pub payer_id: Option<String>,
    pub exp_date: Option<String>,
    pub transaction_type: Option<String>,
    pub payment_mode: Option<String>,
    pub signature: Option<String>,
}

// =============================================================================
// REFUND FLOW (RDP Merchant API — POST /instanpanel/api/payment, form-encoded)
// =============================================================================

/// Merchant API negotiation: data format + operation selector.
const RDP_MERCHANT_API_RESPONSE_TYPE_JSON: &str = "json";
const RDP_MERCHANT_API_ACTION_REFUND: &str = "refund";

/// RDP Merchant API signature (docs §"Capture, Refund and Void API calls"):
/// ASCII-sort the request keys, build a `k=v&k=v` query string (excluding
/// `signature`), append `secret_key=<secret>` as the FINAL pair, hash with
/// MD5 (hex). Distinct from BOTH the SHA-512 payment-api signature
/// (`compute_payment_signature`) and the SHA-512 generic enquiry signature
/// (`compute_generic_signature`).
pub fn compute_merchant_api_signature(
    mut params: Vec<(&str, String)>,
    secret_key: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    params.sort_by_key(|(key_a, _)| *key_a);
    let mut pairs: Vec<String> = params
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    pairs.push(format!("secret_key={secret_key}"));
    let message = pairs.join("&");

    Ok(hex::encode(
        crypto::Md5
            .generate_digest(message.as_bytes())
            .change_context(IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "failed to MD5-hash the RDP Merchant API signature string (sort params + append secret_key)"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Investigate the signing internals; parameter set should be ASCII-sorted k=v pairs"
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.reddotpayment.com/merchant/#capture-refund-void"
                            .to_string(),
                    ),
                },
            })?,
    ))
}

/// `POST /instanpanel/api/payment` (action_type=refund) request body.
/// Form-url-encoded; credentials once again live inside the body.
/// RDP's optional `refund_id` is deliberately NOT sent: without it, RDP's
/// Enquiry API keys the refund to the original payment `transaction_id`,
/// which is exactly what RSync re-uses.
#[derive(Debug, Serialize)]
pub struct ReddotRefundRequest {
    pub response_type: String,
    pub action_type: String,
    pub mid: Secret<String>,
    /// Original payment order number — the order_id we sent at authorize.
    pub order_number: String,
    /// Original RDP transaction id of the payment being refunded.
    pub transaction_id: String,
    pub amount: StringMajorUnit,
    pub currency: Currency,
    pub signature: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ReddotRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for ReddotRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ReddotRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = ReddotAuthType::try_from(&router_data.connector_config)?;

        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert the RDP refund amount from minor units to major units"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Ensure refund_amount is positive and currency is a supported 3-letter ISO code"
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.reddotpayment.com/merchant/#capture-refund-void"
                            .to_string(),
                    ),
                },
            })?;

        let amount_str = amount.get_amount_as_string();
        let ccy = router_data.request.currency.to_string();
        // RDP `order_number` must equal the exact order_id RDP recorded at
        // authorize. That value is connector-minted (gen_reddot_order_id),
        // persisted by euler in second_factor.gateway_auth_req_params as
        // connector_feature_data["reddot_order_id"], and handed back here.
        let order_number = reddot_order_id_from_blob(&router_data.request.connector_feature_data)?;
        let transaction_id = router_data.request.connector_transaction_id.clone();

        let signature = compute_merchant_api_signature(
            vec![
                ("action_type", RDP_MERCHANT_API_ACTION_REFUND.to_string()),
                ("amount", amount_str.clone()),
                ("currency", ccy.clone()),
                ("mid", auth.mid.peek().clone()),
                ("order_number", order_number.clone()),
                (
                    "response_type",
                    RDP_MERCHANT_API_RESPONSE_TYPE_JSON.to_string(),
                ),
                ("transaction_id", transaction_id.clone()),
            ],
            auth.secret.peek(),
        )?;

        Ok(Self {
            response_type: RDP_MERCHANT_API_RESPONSE_TYPE_JSON.to_string(),
            action_type: RDP_MERCHANT_API_ACTION_REFUND.to_string(),
            mid: auth.mid,
            order_number,
            transaction_id,
            amount,
            currency: router_data.request.currency,
            signature: Secret::new(signature),
        })
    }
}

/// RDP Merchant API refund response body. `result_status` values per docs:
/// accepted | failed | pending | capture pending | pending third party.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReddotRefundResponse {
    pub result_status: Option<String>,
    pub reason_code: Option<String>,
    pub refund_id: Option<String>,
    pub order_number: Option<String>,
    pub amount: Option<StringMajorUnit>,
    pub currency: Option<Currency>,
    pub timestamp: Option<String>,
    pub description: Option<String>,
    pub signature: Option<String>,
}

impl TryFrom<ResponseRouterData<ReddotRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<ReddotRefundResponse, Self>) -> Result<Self, Self::Error> {
        let raw_connector_response = serde_json::to_string(&item.response).ok().map(Secret::new);

        // Only `accepted` is a definitive refund; `failed` is terminal.
        // RDP processes settlement asynchronously with the acquirer, so every
        // other explicit status (`pending`, `capture pending`,
        // `pending third party`) means "not final" → Pending, and the
        // follow-up RSync decides.
        let refund_status = match item.response.result_status.as_deref() {
            Some("accepted") => common_enums::RefundStatus::Success,
            Some("failed") => common_enums::RefundStatus::Failure,
            _ => common_enums::RefundStatus::Pending,
        };

        let connector_refund_id = item
            .response
            .refund_id
            .clone()
            .unwrap_or_else(|| item.router_data.request.refund_id.clone());

        // Echo the refund amount/currency into the refund integrity object
        // when RDP returns both (mirrors maya's refund transform).
        let integrity_object = match (item.response.amount.clone(), item.response.currency) {
            (Some(amount), Some(currency)) => {
                let refund_amount = ReddotAmountConvertor::convert_back(amount, currency)
                    .change_context(ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some(
                            "reddot: failed to parse refund amount from refund response"
                                .to_string(),
                        ),
                    ))?;
                Some(RefundIntegrityObject {
                    refund_amount,
                    currency,
                })
            }
            _ => None,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            request: RefundsData {
                integrity_object,
                ..item.router_data.request
            },
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(RawConnectorStatus {
                    // code = RDP machine reason code ("05"), message = human
                    // description, reason = lifecycle verdict — same ordering
                    // as the RSync transform below.
                    code: item.response.reason_code.clone(),
                    message: item.response.description.clone(),
                    reason: item.response.result_status.clone(),
                }),
                raw_connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REFUND SYNC FLOW (RDP Enquiry API — same endpoint/signature as psync)
// =============================================================================

const RDP_ENQUIRY_STATUS_REFUNDED: &str = "refunded";
const RDP_ENQUIRY_STATUS_REFUND_PENDING: &str = "refund pending";

/// RSync re-enquires with the same three-field shape as psync — an alias
/// keeps `create_all_prerequisites!` from emitting a duplicate templating
/// struct while still letting the flow's bridge types stay distinct.
pub type ReddotRefundSyncRequest = ReddotPSyncRequest;

/// Request: the enquiry endpoint takes exactly `request_mid` +
/// `transaction_id` + SHA-512 generic signature — the SAME body shape as
/// psync. Per the RDP Enquiry docs the `transaction_id` slot must carry the
/// **refund_id** only when the refund was initiated with one — we never
/// send `refund_id`, so RSync re-uses the original payment's
/// `transaction_id` and RDP resolves the refund through its refund history.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ReddotRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for ReddotPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ReddotRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = ReddotAuthType::try_from(&router_data.connector_config)?;
        let transaction_id = router_data.request.connector_transaction_id.clone();
        let signature = compute_generic_signature(
            vec![
                ("request_mid", auth.mid.peek().as_str()),
                ("transaction_id", transaction_id.as_str()),
            ],
            auth.secret.peek(),
        );
        Ok(Self {
            request_mid: auth.mid,
            transaction_id,
            signature: Secret::new(signature),
        })
    }
}

/// Enquiry response for refund sync. `transaction_status` is the low-case
/// lifecycle state; the optional `refund` array carries per-refund records.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReddotRefundSyncResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub refund_id: Option<String>,
    pub merchant_reference: Option<String>,
    pub authorized_amount: Option<StringMajorUnit>,
    pub authorized_currency: Option<String>,
    pub transaction_status: Option<String>,
    pub refund_response_code: Option<String>,
    pub refund_response_message: Option<String>,
    pub refund: Option<Vec<ReddotRefundHistoryEntry>>,
    pub signature: Option<String>,
}

/// One entry of the Enquiry `refund` history array.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReddotRefundHistoryEntry {
    pub refund_amount: Option<StringMajorUnit>,
    pub refund_currency: Option<String>,
    pub refund_at: Option<String>,
    pub refund_response_code: Option<String>,
    pub refund_response_message: Option<String>,
}

impl TryFrom<ResponseRouterData<ReddotRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ReddotRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let raw_connector_response = serde_json::to_string(&item.response).ok().map(Secret::new);

        // Only `refunded` is success; `refund pending` is non-final;
        // EVERYTHING else is terminal failure.
        let refund_status = match item.response.transaction_status.as_deref() {
            Some(RDP_ENQUIRY_STATUS_REFUNDED) => common_enums::RefundStatus::Success,
            Some(RDP_ENQUIRY_STATUS_REFUND_PENDING) => common_enums::RefundStatus::Pending,
            _ => common_enums::RefundStatus::Failure,
        };

        let connector_refund_id = item
            .response
            .refund_id
            .clone()
            .unwrap_or_else(|| item.router_data.request.connector_refund_id.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                refund_id: item.response.refund_id.clone().or(item
                    .router_data
                    .resource_common_data
                    .refund_id
                    .clone()),
                raw_connector_status: Some(RawConnectorStatus {
                    code: item.response.refund_response_code.clone(),
                    message: item.response.refund_response_message.clone(),
                    reason: item.response.transaction_status.clone(),
                }),
                raw_connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    fn blob_with_order_id(order_id: &str) -> common_utils::pii::SecretSerdeValue {
        Secret::new(serde_json::json!({ RDP_ORDER_ID_BLOB_KEY: order_id }))
    }

    // ── gen_reddot_order_id ────────────────────────────────────────────

    #[test]
    fn order_id_is_16_lowercase_hex() {
        let oid = gen_reddot_order_id();
        assert_eq!(oid.len(), RDP_ORDER_ID_GENERATED_LEN);
        assert!(oid
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn order_id_two_draws_differ() {
        // Birthday-paradox-level assertion of functioning RNG (not statistical)
        assert_ne!(gen_reddot_order_id(), gen_reddot_order_id());
    }

    // ── reddot_order_id_from_blob (refund read path) ────────────────────

    #[test]
    fn blob_round_trip_returns_order_id() {
        let blob = blob_with_order_id("0123abcd4567ef89");
        assert_eq!(
            reddot_order_id_from_blob(&Some(blob)).unwrap(),
            "0123abcd4567ef89"
        );
    }

    #[test]
    fn blob_missing_is_error() {
        assert!(reddot_order_id_from_blob(&None).is_err());
    }

    #[test]
    fn blob_missing_key_is_error() {
        let blob: common_utils::pii::SecretSerdeValue =
            Secret::new(serde_json::json!({ "some_other_key": "x" }));
        assert!(reddot_order_id_from_blob(&Some(blob)).is_err());
    }

    #[test]
    fn blob_non_string_value_is_error() {
        let blob: common_utils::pii::SecretSerdeValue =
            Secret::new(serde_json::json!({ RDP_ORDER_ID_BLOB_KEY: 42 }));
        assert!(reddot_order_id_from_blob(&Some(blob)).is_err());
    }

    // ── compute_merchant_api_signature (MD5, k=v pairs + secret last) ─────

    /// Golden vector: md5("a=1&b=2&secret_key=skey"). Sorting is exercised by
    /// feeding the params in reverse order.
    #[test]
    fn merchant_api_signature_golden_vector() {
        let sig = compute_merchant_api_signature(
            vec![("b", "2".to_string()), ("a", "1".to_string())],
            "skey",
        )
        .expect("md5 must digest fine");
        assert_eq!(sig, "de6af8a9207bf966d494421fa640a6d0");
    }

    // ── compute_generic_signature (SHA-512 over sorted values + secret) ───

    /// Golden vector: sha512("v1v2" + "mysecret").
    #[test]
    fn generic_signature_golden_vector() {
        let sig = compute_generic_signature(vec![("k2", "v2"), ("k1", "v1")], "mysecret");
        assert_eq!(
            sig,
            "c932cdf346bb82ec0f3923398da8fa9017343fea411cef0fe482325a14dde970cc20a64c5bb3629bb23dd07cecdc6f6954ac6f5b72db77f8fd5f9864f67c0768"
        );
    }
}
