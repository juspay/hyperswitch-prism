use std::str::FromStr;

use common_enums::{AttemptStatus, Currency};
use common_utils::{
    consts::NO_ERROR_CODE,
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
const RDP_ORDER_ID_GENERATED_LEN: usize = 20;

/// Key inside the `connector_feature_data` JSON blob under which the
/// persisted order_id lives between authorize and refund.
pub(crate) const RDP_ORDER_ID_BLOB_KEY: &str = "reddot_order_id";

/// Derive a stable 80-bit hexadecimal order ID within RDP's 20-character limit.
///
/// Authorize-only: the ID is deterministic over `(mid, reference)`, so retries
/// of one payment attempt reuse the same order ID without consulting
/// `connector_feature_data`.
fn generate_reddot_order_id(
    mid: &str,
    reference: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    use sha2::{Digest, Sha256};

    if reference.is_empty() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "connector_request_reference_id",
            context: errors::IntegrationErrorContext::default(),
        }
        .into());
    }

    // Domain separation and a length-prefixed MID avoid ambiguous inputs.
    let mut hasher = Sha256::new();
    hasher.update(b"reddot-order-v1\0");
    let mid_len =
        u64::try_from(mid.len()).change_context(IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext::default(),
        })?;
    hasher.update(mid_len.to_be_bytes());
    hasher.update(mid.as_bytes());
    hasher.update(reference.as_bytes());
    let digest = hasher.finalize();
    let truncated = digest
        .get(..RDP_ORDER_ID_GENERATED_LEN / 2)
        .ok_or_else(|| IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext::default(),
        })?;
    Ok(hex::encode(truncated))
}

/// Read the order ID sent at authorize from `connector_feature_data` for
/// subsequent flows (e.g. refund).
///
/// Subsequent flows must reuse the exact ID RDP recorded at authorize — a
/// missing or malformed entry is an error, never a freshly generated ID.
fn resolve_reddot_order_id(
    connector_feature_data: &Option<common_utils::pii::SecretSerdeValue>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let Some(stored) = connector_feature_data
        .as_ref()
        .and_then(|cfd| cfd.peek().get(RDP_ORDER_ID_BLOB_KEY))
    else {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "connector_feature_data.reddot_order_id",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "Original payment order ID missing from connector_feature_data.".to_string(),
                ),
                suggested_action: Some(
                    "Pass the original connector_metadata as connector_feature_data, or use the RDP console"
                        .to_string(),
                ),
                doc_url: Some(
                    "https://developers.reddotpayment.com/merchant/#capture-refund-void"
                        .to_string(),
                ),
            },
        }
        .into());
    };

    stored
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= RDP_ORDER_ID_GENERATED_LEN)
        .map(str::to_owned)
        .ok_or_else(|| {
            IntegrationError::InvalidDataFormat {
                field_name: "connector_feature_data.reddot_order_id",
                context: errors::IntegrationErrorContext::default(),
            }
            .into()
        })
}

/// Red Dot Payment (RDP) credentials.
///
/// Typed RedDot config: `mid` is the RDP merchant id, `secret` is the
/// RDP secret key used to build request signatures. RDP sends no auth headers;
/// credentials travel inside the JSON request body only.
#[derive(Debug, Clone)]
pub struct ReddotAuthType {
    pub mid: Secret<String>,
    pub secret: Secret<String>,
    /// Validated account flag; the public config accepts true/false strings.
    pub acquirer_cybersource: bool,
}

impl ReddotAuthType {
    fn is_cybersource_acquired(&self) -> bool {
        self.acquirer_cybersource
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
                acquirer_cybersource: match acquirer_cybersource.to_ascii_lowercase().as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err(IntegrationError::InvalidDataFormat {
                        field_name: "acquirer_cybersource",
                        context: errors::IntegrationErrorContext {
                            additional_context: Some("Expected a true or false string".to_string()),
                            ..Default::default()
                        },
                    }.into()),
                },
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
    /// fallback: no gateway error code is available; body text (truncated,
    /// decoded lossily) becomes the `message`.
    pub fn from_non_json_body(_status_code: u16, body: &[u8]) -> Self {
        let text = String::from_utf8_lossy(body);
        Self {
            response_status: "ERROR".to_string(),
            response_code: NO_ERROR_CODE.to_string(),
            response_msg: text.chars().take(200).collect(),
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
    pub ccy: Currency,
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
    /// notification webhook for caller-side reconciliation.
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
/// Crate visibility allows reuse by connector response verification.
pub(crate) fn compute_generic_signature(mut params: Vec<(&str, &str)>, secret_key: &str) -> String {
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
            router_data.request.amount.amount,
            router_data.request.currency,
        )?;
        let redirect_url = router_data.request.get_router_return_url()?;
        let notify_url = router_data.request.get_webhook_url()?;
        let payer_email = router_data.request.get_email()?;
        let ccy = router_data.request.currency.to_string();
        // Retries of one payment attempt reuse the same order ID. Keep the
        // full caller reference separately in merchant_reference.
        let order_id = generate_reddot_order_id(
            auth.mid.peek(),
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;
        let merchant_reference = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Prefer the cardholder name, then the supplied customer name.
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
                    .ok_or_else(|| IntegrationError::MissingRequiredField {
                        field_name: "card_holder_name or customer_name",
                        context: errors::IntegrationErrorContext::default(),
                    })?;
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
                    ccy: router_data.request.currency,
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
        // caller can persist the PG resp_code/resp_message from the data body.
        let raw_connector_status = Some(RawConnectorStatus {
            code: Some(response.response_code.clone()),
            message: response.response_msg.clone(),
            reason: None,
        });

        let (status, payment_response) = match response.response_code.as_str() {
            // A redirect requires customer authentication before the payment
            // is final. An explicit -01 remains pending even with a redirect.
            "0" | "-01" => {
                let auth = ReddotAuthType::try_from(&router_data.connector_config).change_context(
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some("reddot: could not resolve request order_id".to_string()),
                    ),
                )?;
                let order_id = generate_reddot_order_id(
                    auth.mid.peek(),
                    &router_data
                        .resource_common_data
                        .connector_request_reference_id,
                )
                .change_context(
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some("reddot: could not resolve request order_id".to_string()),
                    ),
                )?;

                let transaction_id = response.transaction_id.ok_or_else(|| {
                    ConnectorError::response_handling_failed_with_context(
                        http_code,
                        Some("reddot: success response missing transaction_id".to_string()),
                    )
                })?;
                let payment_url = if response.response_code == "0" {
                    Some(response.payment_url.ok_or_else(|| {
                        ConnectorError::response_handling_failed_with_context(
                            http_code,
                            Some("reddot: success response missing payment_url".to_string()),
                        )
                    })?)
                } else {
                    response.payment_url
                };
                let redirection_data = payment_url.map(|uri| Box::new(RedirectForm::Uri { uri }));
                let status = if response.response_code == "-01" {
                    AttemptStatus::Pending
                } else {
                    AttemptStatus::AuthenticationPending
                };
                (
                    status,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                        redirection_data,
                        mandate_reference: None,
                        // Persist the ID sent on the request even when the
                        // gateway omits order_id. Refund needs this exact value.
                        connector_metadata: Some(
                            serde_json::json!({ RDP_ORDER_ID_BLOB_KEY: order_id }),
                        ),
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: Some(
                            router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                        ),
                        incremental_authorization_allowed: None,
                        splits: None,
                        payment_account_reference: None,
                        status_code: http_code,
                    }),
                )
            }
            // All codes other than 0 and -01 are authentication failures.
            error_code => {
                let attempt_status = AttemptStatus::AuthenticationFailed;
                let response_msg = response.response_msg;
                (
                    attempt_status,
                    Err(ErrorResponse {
                        code: error_code.to_string(),
                        status_code: http_code,
                        message: response_msg.clone().unwrap_or_default(),
                        reason: response_msg,
                        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                            attempt_status,
                        )),
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

/// RDP confirmed that only -01 is pending; all other nonzero payment codes
/// indicate an unsuccessful payment.
pub(crate) fn reddot_payment_status(code: &str) -> AttemptStatus {
    match code {
        "0" => AttemptStatus::Charged,
        "-01" => AttemptStatus::Pending,
        _ => AttemptStatus::AuthenticationFailed,
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
    /// Echo of the caller's transaction reference sent at authorization.
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
    pub first_6: Option<Secret<String>>,
    pub last_4: Option<Secret<String>>,
    pub payer_name: Option<Secret<String>>,
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
        // caller can persist the PG resp_code/resp_message from the data body.
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
                        // Keep the caller reference consistent across payment flows.
                        connector_response_reference_id: response.merchant_reference.clone(),
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
                        connector_response_reference_id: response.merchant_reference.clone(),
                        incremental_authorization_allowed: None,
                        splits: None,
                        payment_account_reference: None,
                        status_code: http_code,
                    }),
                )
            }
            // RDP confirms every nonzero code except -01 is an unsuccessful payment.
            error_code => {
                let attempt_status = reddot_payment_status(error_code);
                let response_msg = response.response_msg;
                (
                    attempt_status,
                    None,
                    Err(ErrorResponse {
                        code: error_code.to_string(),
                        status_code: http_code,
                        message: response_msg.clone().unwrap_or_default(),
                        reason: response_msg,
                        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                            attempt_status,
                        )),
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

        let amount_captured = minor_amount_captured.map(|amount| common_utils::types::Money {
            amount,
            currency: router_data.request.currency,
        });
        Ok(Self {
            response: payment_response,
            request: PaymentsSyncData {
                integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
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
    /// Connector order ID sent at authorization and echoed by RDP.
    pub order_id: Option<String>,
    /// RDP's gateway-side id for this payment (`transaction_id`).
    pub transaction_id: Option<String>,
    /// Echo of the caller transaction reference sent at authorization.
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
    pub first_6: Option<Secret<String>>,
    pub last_4: Option<Secret<String>>,
    pub payer_name: Option<Secret<String>>,
    pub payer_id: Option<String>,
    pub exp_date: Option<Secret<String>>,
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
pub(crate) fn compute_merchant_api_signature(
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
/// `refund_id` carries the caller's unique refund reference and is included in the
/// signed parameters so each refund can be enquired independently.
#[derive(Debug, Serialize)]
pub struct ReddotRefundRequest {
    pub refund_id: String,
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
                router_data.request.refund_amount.amount,
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
        // authorize. It is persisted in connector_metadata["reddot_order_id"]
        // and handed back here through connector_feature_data.
        let order_number = resolve_reddot_order_id(&router_data.request.connector_feature_data)?;
        let transaction_id = router_data.request.connector_transaction_id.clone();

        let refund_id = required_refund_reference(&router_data.request.refund_id)?.to_string();
        let signature = compute_merchant_api_signature(
            vec![
                ("action_type", RDP_MERCHANT_API_ACTION_REFUND.to_string()),
                ("amount", amount_str.clone()),
                ("currency", ccy.clone()),
                ("mid", auth.mid.peek().clone()),
                ("order_number", order_number.clone()),
                ("refund_id", refund_id.clone()),
                (
                    "response_type",
                    RDP_MERCHANT_API_RESPONSE_TYPE_JSON.to_string(),
                ),
                ("transaction_id", transaction_id.clone()),
            ],
            auth.secret.peek(),
        )?;

        Ok(Self {
            refund_id,
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
    /// Some gateway versions additionally return a transaction identifier.
    pub transaction_id: Option<String>,
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

        // Merchant API enquiry explicitly accepts the refund_id echoed by RDP.
        // Do not substitute the caller reference when the gateway omits it.
        let connector_refund_id = item.response.refund_id.clone().unwrap_or_default();

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
// REFUND SYNC FLOW (RDP Enquiry API on the secondary host, generic signature)
// =============================================================================

const RDP_ENQUIRY_STATUS_REFUNDED: &str = "refunded";

/// RSync re-enquires with the same three-field shape as psync — an alias
/// keeps `create_all_prerequisites!` from emitting a duplicate templating
/// struct while still letting the flow's bridge types stay distinct.
pub type ReddotRefundSyncRequest = ReddotPSyncRequest;

/// Enquire using the refund_id echoed by RDP, in the transaction_id field
/// (Merchant API, Enquiry request, page 6). This scopes partial refunds separately.
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
        let transaction_id = router_data.request.connector_refund_id.clone();
        if transaction_id.is_empty() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "connector_refund_id",
                context: errors::IntegrationErrorContext::default(),
            }
            .into());
        }
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
    #[serde(default, deserialize_with = "deserialize_optional_response_code")]
    pub response_code: Option<String>,
    pub response_status: Option<String>,
    pub response_msg: Option<String>,
    pub refund_transaction_id: Option<String>,
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

fn deserialize_optional_response_code<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    Option::<serde_json::Value>::deserialize(deserializer)?
        .map(|value| match value {
            serde_json::Value::String(code) => Ok(code),
            serde_json::Value::Number(code) => Ok(code.to_string()),
            other => Err(D::Error::custom(format!(
                "unexpected type for response_code: {other}"
            ))),
        })
        .transpose()
}

/// One entry of the Enquiry `refund` history array.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReddotRefundHistoryEntry {
    pub refund_id: Option<String>,
    pub refund_transaction_id: Option<String>,
    pub refund_status: Option<String>,
    pub refund_amount: Option<StringMajorUnit>,
    pub refund_currency: Option<String>,
    pub refund_at: Option<String>,
    pub refund_response_code: Option<String>,
    pub refund_response_message: Option<String>,
}

fn required_refund_reference(id: &str) -> Result<&str, error_stack::Report<IntegrationError>> {
    if id.is_empty() {
        Err(IntegrationError::MissingRequiredField {
            field_name: "merchant_refund_id",
            context: errors::IntegrationErrorContext::default(),
        }
        .into())
    } else {
        Ok(id)
    }
}

fn reddot_refund_sync_status(status: Option<&str>) -> common_enums::RefundStatus {
    // Payment response_code semantics do not establish the meaning of
    // refund_response_code. Refund enquiry has its own lifecycle status.
    match status {
        Some(RDP_ENQUIRY_STATUS_REFUNDED | "voided") => common_enums::RefundStatus::Success,
        Some("rejected" | "failed") => common_enums::RefundStatus::Failure,
        _ => common_enums::RefundStatus::Pending,
    }
}

impl TryFrom<ResponseRouterData<ReddotRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ReddotRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let raw_connector_response = serde_json::to_string(&item.response).ok().map(Secret::new);

        // An enquiry failure does not establish that the refund failed. Return
        // the connector error and preserve the refund's previous status.
        if item
            .response
            .response_code
            .as_deref()
            .is_some_and(|code| code != "0")
            || item
                .response
                .response_status
                .as_deref()
                .is_some_and(|status| {
                    status.eq_ignore_ascii_case("error") || status.eq_ignore_ascii_case("failed")
                })
        {
            let code = item.response.response_code.clone();
            let message = item.response.response_msg.clone();
            return Ok(Self {
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: code.clone().unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: message
                        .clone()
                        .unwrap_or_else(|| "Refund enquiry failed".to_string()),
                    reason: message.clone(),
                    raw_connector_response: raw_connector_response.clone(),
                    ..ErrorResponse::default()
                }),
                resource_common_data: RefundFlowData {
                    raw_connector_status: Some(RawConnectorStatus {
                        code,
                        message,
                        reason: item.response.response_status.clone(),
                    }),
                    raw_connector_response,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        if !item
            .response
            .transaction_status
            .as_deref()
            .is_some_and(|status| !status.is_empty())
            && !item
                .response
                .refund
                .as_ref()
                .is_some_and(|history| !history.is_empty())
        {
            return Err(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("reddot: enquiry response contains neither transaction_status nor refund history".to_string()),
            ).into());
        }

        // The request is scoped to the echoed refund_id, so the top-level
        // transaction_status describes this refund, including partial refunds.
        let connector_refund_id = item.router_data.request.connector_refund_id.clone();
        let status = item.response.transaction_status.clone();
        let code = item.response.refund_response_code.clone();
        let message = item.response.refund_response_message.clone();
        let refund_status = reddot_refund_sync_status(status.as_deref());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(RawConnectorStatus {
                    code,
                    message,
                    reason: status,
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
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn payment_codes_follow_rdp_confirmed_status_mapping() {
        for (code, expected) in [
            (json!(0), AttemptStatus::Charged),
            (json!("0"), AttemptStatus::Charged),
            (json!("-01"), AttemptStatus::Pending),
            (json!(-1009), AttemptStatus::AuthenticationFailed),
            (json!("-1003"), AttemptStatus::AuthenticationFailed),
            (json!("new-code"), AttemptStatus::AuthenticationFailed),
            (json!("-1"), AttemptStatus::AuthenticationFailed),
            (json!("-7997"), AttemptStatus::AuthenticationFailed),
        ] {
            let fixture = json!({"response_code": code, "response_msg": "gateway message"});
            let sync: ReddotPSyncResponse = serde_json::from_value(fixture.clone()).unwrap();
            let webhook: ReddotWebhookBody = serde_json::from_value(fixture).unwrap();
            assert_eq!(reddot_payment_status(&sync.response_code), expected);
            assert_eq!(reddot_payment_status(&webhook.response_code), expected);
        }
    }

    #[test]
    fn authorize_ids_are_stable_and_scoped_to_merchant_and_reference() {
        let id = generate_reddot_order_id("merchant", "attempt").unwrap();
        assert_eq!(id.len(), 20);
        assert_eq!(id, generate_reddot_order_id("merchant", "attempt").unwrap());
        assert_ne!(id, generate_reddot_order_id("other", "attempt").unwrap());
        assert_ne!(id, generate_reddot_order_id("merchant", "other").unwrap());
        assert!(generate_reddot_order_id("merchant", "").is_err());
    }

    #[test]
    fn subsequent_flows_require_the_original_order_id() {
        assert!(resolve_reddot_order_id(&None).is_err());
        for value in [json!(null), json!(123), json!(""), json!("x".repeat(21))] {
            assert!(resolve_reddot_order_id(&Some(Secret::new(json!({
                "reddot_order_id": value
            }))))
            .is_err());
        }
        assert_eq!(
            resolve_reddot_order_id(&Some(Secret::new(json!({
                "reddot_order_id": "legacy-order"
            }))))
            .unwrap(),
            "legacy-order"
        );
    }

    #[test]
    fn non_json_errors_use_the_shared_missing_code_constant() {
        let response = ReddotErrorResponse::from_non_json_body(403, b"  <html>blocked</html>  ");
        assert_eq!(response.response_code, NO_ERROR_CODE);
        assert_eq!(response.response_msg, "  <html>blocked</html>  ");
    }

    #[test]
    fn refund_enquiry_keeps_unknown_and_payment_states_pending() {
        for (status, expected) in [
            (Some("refunded"), common_enums::RefundStatus::Success),
            (Some("refund pending"), common_enums::RefundStatus::Pending),
            (Some("paid"), common_enums::RefundStatus::Pending),
            (Some("captured"), common_enums::RefundStatus::Pending),
            (Some("new-status"), common_enums::RefundStatus::Pending),
            (None, common_enums::RefundStatus::Pending),
            (Some("rejected"), common_enums::RefundStatus::Failure),
        ] {
            let response: ReddotRefundSyncResponse = serde_json::from_value(json!({
                "response_code": 0, "transaction_status": status
            }))
            .unwrap();
            assert_eq!(
                reddot_refund_sync_status(response.transaction_status.as_deref()),
                expected
            );
        }
    }

    fn config() -> ConnectorSpecificConfig {
        ConnectorSpecificConfig::Reddot {
            mid: Secret::new("test-mid".to_string()),
            secret: Secret::new("test-secret".to_string()),
            acquirer_cybersource: "false".to_string(),
            base_url: None,
        }
    }

    fn payment_sync_data(
    ) -> RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData> {
        RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: Default::default(),
                customer_id: Default::default(),
                connector_customer: Default::default(),
                payment_id: Default::default(),
                attempt_id: Default::default(),
                status: Default::default(),
                payment_method: Default::default(),
                payment_method_type: Default::default(),
                description: Default::default(),
                return_url: Default::default(),
                address: Default::default(),
                auth_type: Default::default(),
                connector_feature_data: Default::default(),
                amount_captured: Default::default(),
                amount_capturable: Default::default(),
                amount: Default::default(),
                access_token: Default::default(),
                session_token: Default::default(),
                reference_id: Default::default(),
                connector_order_id: Default::default(),
                preprocessing_id: Default::default(),
                connector_api_version: Default::default(),
                connector_request_reference_id: Default::default(),
                test_mode: Default::default(),
                connector_http_status_code: Default::default(),
                connector_response_headers: Default::default(),
                external_latency: Default::default(),
                connectors: Default::default(),
                raw_connector_response: Default::default(),
                typed_connector_response: Default::default(),
                raw_connector_request: Default::default(),
                typed_connector_request: Default::default(),
                vault_headers: Default::default(),
                connector_response: Default::default(),
                recurring_mandate_payment_data: Default::default(),
                order_details: Default::default(),
                amount_authorized: Default::default(),
                l2_l3_data: Default::default(),
                merchant_request_id: Default::default(),
                sender_payment_instrument_id: Default::default(),
                connector_returned_payment_method_details: Default::default(),
                settlement_status: Default::default(),
                raw_connector_status: Default::default(),
            },
            connector_config: config(),
            request: PaymentsSyncData::default(),
            response: Err(ErrorResponse::default()),
        }
    }

    fn refund_sync_data(
        id: &str,
    ) -> RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData> {
        RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: RefundFlowData {
                merchant_id: Default::default(),
                status: Default::default(),
                refund_id: Default::default(),
                connectors: Default::default(),
                connector_request_reference_id: Default::default(),
                raw_connector_response: Default::default(),
                typed_connector_response: Default::default(),
                connector_response_headers: Default::default(),
                raw_connector_request: Default::default(),
                typed_connector_request: Default::default(),
                access_token: Default::default(),
                connector_feature_data: Default::default(),
                test_mode: Default::default(),
                payment_method: Default::default(),
                merchant_request_id: Default::default(),
                raw_connector_status: Default::default(),
            },
            connector_config: config(),
            request: RefundSyncData {
                connector_transaction_id: "original-payment".to_string(),
                connector_refund_id: id.to_string(),
                reason: Default::default(),
                refund_connector_metadata: Default::default(),
                refund_status: Default::default(),
                all_keys_required: Default::default(),
                integrity_object: Default::default(),
                browser_info: Default::default(),
                split_refunds: Default::default(),
                connector_feature_data: Default::default(),
                refund_money: Default::default(),
                connector_order_id: Default::default(),
            },
            response: Err(ErrorResponse::default()),
        }
    }

    #[test]
    fn psync_transformer_fails_all_nonzero_codes_except_pending() {
        for (code, expected) in [
            (json!(0), AttemptStatus::Charged),
            (json!("-01"), AttemptStatus::Pending),
            (json!(-1009), AttemptStatus::AuthenticationFailed),
            (json!("-1003"), AttemptStatus::AuthenticationFailed),
            (json!("new-code"), AttemptStatus::AuthenticationFailed),
            (json!("-1"), AttemptStatus::AuthenticationFailed),
            (json!("-7997"), AttemptStatus::AuthenticationFailed),
        ] {
            let response: ReddotPSyncResponse = serde_json::from_value(json!({
                "response_code": code, "transaction_id": "payment-1"
            }))
            .unwrap();
            let result = RouterDataV2::try_from(ResponseRouterData {
                response,
                router_data: payment_sync_data(),
                http_code: 200,
            })
            .unwrap();
            assert_eq!(result.resource_common_data.status, expected);
        }
        let response: ReddotPSyncResponse = serde_json::from_value(json!({
            "response_code": "-1003", "response_msg": "Invalid signature"
        }))
        .unwrap();
        let result = RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: payment_sync_data(),
            http_code: 200,
        })
        .unwrap();
        assert_eq!(
            result.resource_common_data.status,
            AttemptStatus::AuthenticationFailed
        );
    }

    #[test]
    fn refund_enquiry_request_is_scoped_to_the_echoed_refund_id() {
        use domain_types::payment_method_data::DefaultPCIHolder;
        for id in ["refund-1", "refund-2"] {
            let request = ReddotPSyncRequest::try_from(ReddotRouterData {
                connector: crate::connectors::reddot::Reddot::<DefaultPCIHolder>::new().clone(),
                router_data: refund_sync_data(id),
            })
            .unwrap();
            assert_eq!(request.transaction_id, id);
            assert_eq!(
                request.signature.peek(),
                &compute_generic_signature(
                    vec![("request_mid", "test-mid"), ("transaction_id", id)],
                    "test-secret"
                )
            );
        }
    }

    #[test]
    fn rsync_transformer_uses_the_refund_scoped_status() {
        for (status, expected) in [
            ("refunded", common_enums::RefundStatus::Success),
            ("refund pending", common_enums::RefundStatus::Pending),
            ("paid", common_enums::RefundStatus::Pending),
            ("unknown", common_enums::RefundStatus::Pending),
            ("rejected", common_enums::RefundStatus::Failure),
        ] {
            let response: ReddotRefundSyncResponse = serde_json::from_value(json!({
                "transaction_id": "refund-2", "refund_id": "refund-2", "transaction_status": status
            }))
            .unwrap();
            let result = RouterDataV2::try_from(ResponseRouterData {
                response,
                router_data: refund_sync_data("refund-2"),
                http_code: 200,
            })
            .unwrap();
            let response = result.response.unwrap();
            assert_eq!(response.connector_refund_id, "refund-2");
            assert_eq!(response.refund_status, expected);
        }
    }
    #[test]
    fn cybersource_string_config_rejects_typos_instead_of_disabling_billing() {
        for (value, expected) in [
            ("true", true),
            ("TRUE", true),
            ("false", false),
            ("False", false),
        ] {
            let mut config = config();
            if let ConnectorSpecificConfig::Reddot {
                acquirer_cybersource,
                ..
            } = &mut config
            {
                *acquirer_cybersource = value.to_string();
            }
            assert_eq!(
                ReddotAuthType::try_from(&config)
                    .unwrap()
                    .acquirer_cybersource,
                expected
            );
        }
        for value in ["", "tru", "yes", "1"] {
            let mut config = config();
            if let ConnectorSpecificConfig::Reddot {
                acquirer_cybersource,
                ..
            } = &mut config
            {
                *acquirer_cybersource = value.to_string();
            }
            assert!(ReddotAuthType::try_from(&config).is_err());
        }
    }
    #[test]
    fn rsync_handles_observed_full_and_partial_refund_responses() {
        // Refund-scoped enquiries still echo the original payment transaction_id.
        // Partial refund entries contain refund_id but no refund_status.
        for (refund_id, payment_id, status, history) in [
            (
                "refund_1793166922",
                "Mansi_1793166898_171032068910700",
                "voided",
                None,
            ),
            (
                "mansi_khatri_refund_1",
                "Mansi_1793167012_165242157421379",
                "refunded",
                Some(json!([
                    {"refund_amount": "0.01", "refund_currency": "SGD", "refund_at": "2026-10-28 13:57:28", "refund_id": "mansi_khatri_refund_1"}
                ])),
            ),
            (
                "mansi_khatri_refund_2",
                "Mansi_1793167012_165242157421379",
                "refunded",
                Some(json!([
                    {"refund_amount": "0.02", "refund_currency": "SGD", "refund_at": "2026-10-28 13:57:52", "refund_id": "mansi_khatri_refund_2"}
                ])),
            ),
        ] {
            let response: ReddotRefundSyncResponse = serde_json::from_value(json!({
                "order_id": "original-order",
                "transaction_id": payment_id,
                "authorized_amount": "1.10",
                "authorized_currency": "SGD",
                "transaction_status": status,
                "refund": history,
                "transaction_created_at": "2026-10-28 13:56:52",
                "transaction_updated_at": "2026-10-28 13:57:52"
            }))
            .unwrap();
            assert!(response.response_code.is_none());
            let result = RouterDataV2::try_from(ResponseRouterData {
                response,
                router_data: refund_sync_data(refund_id),
                http_code: 200,
            })
            .unwrap();
            let response = result.response.unwrap();
            assert_eq!(response.refund_status, common_enums::RefundStatus::Success);
            assert_eq!(response.connector_refund_id, refund_id);
        }
    }

    #[test]
    fn payment_scoped_refund_history_deserializes_with_gateway_ids_and_statuses() {
        let response: ReddotRefundSyncResponse = serde_json::from_value(json!({
            "transaction_id": "Mansi_1793167012_165242157421379",
            "transaction_status": "refunded",
            "refund": [
                {"refund_amount": "0.02", "refund_currency": "SGD", "refund_transaction_id": "mansi_khatri_refund_2_MIP", "refund_status": "refunded"},
                {"refund_amount": "0.01", "refund_currency": "SGD", "refund_transaction_id": "mansi_khatri_refund_1_MIP", "refund_status": "refunded"}
            ]
        })).unwrap();
        let history = response.refund.unwrap();
        assert_eq!(history.len(), 2);
        let second_refund = history.first().unwrap();
        assert_eq!(
            second_refund.refund_transaction_id.as_deref(),
            Some("mansi_khatri_refund_2_MIP")
        );
        assert_eq!(second_refund.refund_status.as_deref(), Some("refunded"));
        assert!(second_refund.refund_id.is_none());
    }
}
