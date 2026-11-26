use crate::{connectors::fire::FireRouterData, types::ResponseRouterData};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use common_enums::{AttemptStatus, BankNames, CountryAlpha2, Currency, RefundStatus};
use common_utils::{
    consts::NO_ERROR_MESSAGE,
    crypto::{GenerateDigest, Sha256},
    money::{AmountConvertor, Money},
    types::{MinorUnit, MinorUnitForConnector},
};
use domain_types::{
    connector_flow::{
        Authorize, PSync, RSync, Refund, ServerAuthenticationToken, VerifyWebhookSource,
    },
    connector_types::{
        ConnectorWebhookSecrets, EventType, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RawConnectorStatus, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, SettlementStatus, VerifyWebhookSourceFlowData,
    },
    errors::{self, IntegrationError, IntegrationErrorContext, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        BankRedirectData, OpenBankingData, PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{
        AuthoriseIntegrityObject, PaymentSynIntegrityObject, VerifyWebhookSourceRequestData,
    },
    router_response_types::{RedirectForm, VerifyWebhookSourceResponseData, VerifyWebhookStatus},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};

static LAST_NONCE: AtomicI64 = AtomicI64::new(0);
const FIRE_MAX_NUMBER_PAYMENTS: i32 = 1;
const FIRE_PAYMENT_NOT_AUTHORISED: &str = "NOT_AUTHORISED";
const FIRE_PAYMENT_NOT_ATTEMPTED: &str = "PAYMENT_NOT_ATTEMPTED";
const FIRE_REFUND_REJECTED: &str = "REFUND_REJECTED";
/// Sent as the refund `reason` (mandatory on FIRE's side) when the merchant
/// supplies none.
const FIRE_DEFAULT_REFUND_REASON: &str = "Refund requested by merchant";
const FIRE_BEARER_TOKEN_TYPE: &str = "Bearer";
/// Production JWKS used by FIRE to sign Bank Pay webhook JWTs (RS256).
/// The endpoint is fixed by the vendor, not derived from the base URL.
pub(crate) const FIRE_WEBHOOK_JWKS_URL: &str = "https://bankpay-api.fire.com/jwks/bupa_public.jwks";

const FIRE_ASPSP_AIB_IE_PERSONAL: &str = "FF8AB34B-9646-497C-8932-82E937651460";
const FIRE_ASPSP_AIB_GB_BUSINESS: &str = "86F6979F-D4CB-4DF8-B609-F7DB3A65A203";
const FIRE_ASPSP_AIB_GB_PERSONAL: &str = "FF8AB34B-9646-497C-8932-82E93765146D";
const FIRE_ASPSP_BOI_IE_BUSINESS: &str = "EA281726-FCD4-46D0-92C1-36817E92E544";
const FIRE_ASPSP_BOI_IE_PERSONAL: &str = "214E5759-C4B7-4555-A370-796A41A47B07";
const FIRE_ASPSP_BOI_GB_BUSINESS: &str = "A0F50801-02B7-4551-87F6-6183CAA58100";
const FIRE_ASPSP_BOI_GB_PERSONAL: &str = "1C02AAC1-4475-4990-AB75-DCF7D06945F7";
const FIRE_ASPSP_BOS_GB_BUSINESS: &str = "48410F57-28F5-4038-9F3B-CE61B7A59FEE";
const FIRE_ASPSP_BOS_GB_PERSONAL: &str = "48410F57-28F5-4038-9F3B-CE61B7A59FED";
const FIRE_ASPSP_BARCLAYS_GB: &str = "105CBAF1-8A49-4C99-8205-928EA4014D9F";

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum FirePaymentRequestType {
    Other,
}

#[derive(Debug, Serialize)]
enum FireGrantType {
    #[serde(rename = "AccessToken")]
    AccessToken,
}

#[derive(Debug, Clone)]
pub struct FireAuthType {
    pub client_id: Secret<String>,
    pub client_key: Secret<String>,
    pub refresh_token: Secret<String>,
    pub ican_to: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for FireAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Fire {
                client_id,
                client_key,
                refresh_token,
                ican_to,
                ..
            } => Ok(Self {
                client_id: client_id.to_owned(),
                client_key: client_key.to_owned(),
                refresh_token: refresh_token.to_owned(),
                ican_to: ican_to.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext::default()
                }
            )),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireServerAuthenticationTokenRequest {
    client_id: Secret<String>,
    client_secret: Secret<String>,
    refresh_token: Secret<String>,
    nonce: i64,
    grant_type: FireGrantType,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FireRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for FireServerAuthenticationTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: FireRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FireAuthType::try_from(&item.router_data.connector_config)?;
        let nonce = next_nonce()?;
        let preimage = format!("{nonce}{}", auth.client_key.expose());
        let digest = Sha256.generate_digest(preimage.as_bytes()).change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )?;

        Ok(Self {
            client_id: auth.client_id,
            client_secret: Secret::new(hex::encode(digest)),
            refresh_token: auth.refresh_token,
            nonce,
            grant_type: FireGrantType::AccessToken,
        })
    }
}

fn next_nonce() -> Result<i64, IntegrationError> {
    let timestamp = common_utils::date_time::now_unix_micros();
    let timestamp =
        i64::try_from(timestamp).map_err(|_| IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;

    // Microsecond Unix time is practically unique across hosts. The atomic
    // floor also guarantees strictly increasing nonces for concurrent calls
    // in this process, including when the wall clock repeats a timestamp.
    let mut previous = LAST_NONCE.load(Ordering::Acquire);
    loop {
        let candidate = if timestamp > previous {
            timestamp
        } else {
            previous
                .checked_add(1)
                .ok_or_else(|| IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?
        };
        match LAST_NONCE.compare_exchange(previous, candidate, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => return Ok(candidate),
            Err(actual) => previous = actual,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireServerAuthenticationTokenResponse {
    access_token: Secret<String>,
    expiry: Option<String>,
    token_type: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<FireServerAuthenticationTokenResponse, Self>>
    for RouterDataV2<F, MerchantAuthenticationFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FireServerAuthenticationTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let expires_in = item
            .response
            .expiry
            .as_deref()
            .and_then(|expiry| {
                time::OffsetDateTime::parse(expiry, &time::format_description::well_known::Rfc3339)
                    .ok()
            })
            .map(|expiry| (expiry - common_utils::date_time::now().assume_utc()).whole_seconds());

        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in,
                // FIRE's auth guide specifies Bearer tokens for protected API calls.
                token_type: item
                    .response
                    .token_type
                    .or_else(|| Some(FIRE_BEARER_TOKEN_TYPE.to_string())),
            }),
            ..item.router_data
        })
    }
}

// FIRE's error body is `{"errors": [{"code": 51143, "message": "..."}]}` - an
// array with integer codes (see the vendor's error-code table). Every field is
// `Option` on purpose: a vendor error body that omits `code` or `message` must
// still deserialize. The NO_ERROR_CODE / NO_ERROR_MESSAGE sentinels are applied
// at the call site in the connector's `build_error_response` - never
// `.unwrap_or_default()`, which turns a missing code into an empty string
// indistinguishable from a real empty code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireErrorResponse {
    #[serde(default)]
    pub errors: Vec<FireErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireErrorDetail {
    pub code: Option<i64>,
    pub message: Option<String>,
}

impl FireErrorResponse {
    /// The first entry of the vendor `errors` array, if present.
    pub fn primary_error(&self) -> Option<&FireErrorDetail> {
        self.errors.first()
    }
}

/// The vendor status verbatim (wire spelling, e.g. `AWAITING_AUTHORISATION`),
/// surfaced through `raw_connector_status` for support and debugging.
fn fire_raw_connector_status<T: Serialize>(status: &T) -> Option<RawConnectorStatus> {
    let code = serde_json::to_value(status).ok()?.as_str()?.to_string();
    Some(RawConnectorStatus {
        code: Some(code),
        message: None,
        reason: None,
    })
}

/// `orderDetails.merchantNumber` is an integer on FIRE's side and carries the
/// merchant id. A merchant id that is not a valid unsigned integer leaves the
/// field out.
fn fire_merchant_number(merchant_id: &common_utils::id_type::MerchantId) -> Option<u64> {
    merchant_id.get_string_repr().parse::<u64>().ok()
}

/// FIRE recommends that `description` contain only letters, numbers, spaces and
/// a full stop, because most banks reject other characters and the error only
/// appears after the customer has approved the payment. Every other character
/// is replaced with a space, whitespace runs are collapsed and the result is
/// trimmed. If nothing usable is left, the original text is sent unchanged.
fn sanitize_fire_description(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let sanitized = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if sanitized.is_empty() {
        raw.to_string()
    } else {
        sanitized
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireAuthorizeRequest {
    #[serde(rename = "type")]
    payment_type: FirePaymentRequestType,
    ican_to: i64,
    currency: Currency,
    amount: MinorUnit,
    description: String,
    my_ref: String,
    max_number_payments: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_url: Option<String>,
    order_details: FireOrderDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    webhook_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry: Option<String>,
}

// `orderDetails` carries the delivery address as FLAT sibling fields
// (deliveryAddressLine1, deliveryCity, ...), not a nested `delivery` object -
// see FIRE's Create Payment Request reference.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FireOrderDetails {
    order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_customer_identification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_address_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_post_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_country: Option<CountryAlpha2>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FireRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FireAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: FireRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FireAuthType::try_from(&item.router_data.connector_config)?;
        let ican_to = auth.ican_to.expose().parse::<i64>().map_err(|_| {
            IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext::default(),
            }
        })?;
        let currency = item.router_data.request.currency;
        // Reject unsupported payment methods and banks before FIRE creates a
        // payment request; the response transformer only builds the URL.
        fire_aspsp_uuid(&item.router_data.request.payment_method_data)?;
        let order_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let description = item
            .router_data
            .resource_common_data
            .get_description()
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "description",
                context: IntegrationErrorContext::default(),
            })?;
        // Optional payment-request expiry (RFC 3339 date-time). Sent via the
        // first-class `payment_expiry` field (Unix epoch seconds, formatted to
        // RFC 3339 here).
        let expiry = item
            .router_data
            .request
            .payment_expiry
            .filter(|seconds| *seconds > 0)
            .and_then(|seconds| {
                time::OffsetDateTime::from_unix_timestamp(seconds)
                    .ok()
                    .and_then(|dt| {
                        dt.format(&time::format_description::well_known::Rfc3339)
                            .ok()
                    })
            });

        Ok(Self {
            payment_type: FirePaymentRequestType::Other,
            ican_to,
            currency,
            amount: item.router_data.request.money.amount(),
            // FIRE requires myRef and description to be unique across payment
            // requests; the merchant-facing description satisfies both. Only
            // the bank-facing `description` is sanitized; `myRef` is internal
            // and keeps the original text.
            description: sanitize_fire_description(&description),
            my_ref: description,
            max_number_payments: FIRE_MAX_NUMBER_PAYMENTS,
            return_url: item
                .router_data
                .request
                .router_return_url
                .clone()
                .or_else(|| item.router_data.resource_common_data.return_url.clone()),
            order_details: FireOrderDetails {
                order_id,
                merchant_number: fire_merchant_number(
                    &item.router_data.resource_common_data.merchant_id,
                ),
                merchant_customer_identification: item
                    .router_data
                    .request
                    .customer_id
                    .as_ref()
                    .map(|customer_id| customer_id.get_string_repr().to_string()),
                delivery_address_line1: item
                    .router_data
                    .resource_common_data
                    .get_optional_shipping_line1(),
                delivery_address_line2: item
                    .router_data
                    .resource_common_data
                    .get_optional_shipping_line2(),
                delivery_city: item
                    .router_data
                    .resource_common_data
                    .get_optional_shipping_city(),
                delivery_post_code: item
                    .router_data
                    .resource_common_data
                    .get_optional_shipping_zip(),
                delivery_country: item
                    .router_data
                    .resource_common_data
                    .get_optional_shipping_country(),
            },
            webhook_url: item.router_data.request.webhook_url.clone(),
            expiry,
        })
    }
}

/// Payment status, in FIRE's SCREAMING_SNAKE_CASE wire spelling (Get Payment
/// Details v2 reference).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FirePaymentStatus {
    AwaitingAuthorisation,
    AwaitingMultiAuthorisation,
    NotAuthorised,
    Pending,
    Authorised,
    FundsConfirmed,
    Settled,
    /// A status this connector does not recognize. The raw wire value FIRE
    /// sent is preserved verbatim so `raw_connector_status` shows exactly
    /// what came in; it is treated as `Pending` - never terminal. Untagged,
    /// so it (de)serializes as the bare string and catches any value the
    /// variants above do not match.
    #[serde(untagged)]
    Unknown(String),
}

// Status meanings follow FIRE's Get Payment Details v2 reference:
// https://docs.fire.com/reference/getpaymentdetailsv2
impl FirePaymentStatus {
    fn is_successful(&self) -> bool {
        matches!(
            self,
            Self::Authorised | Self::FundsConfirmed | Self::Settled
        )
    }

    fn attempt_status(&self) -> AttemptStatus {
        match self {
            Self::AwaitingAuthorisation
            | Self::AwaitingMultiAuthorisation
            | Self::Pending
            | Self::Unknown(_) => AttemptStatus::Pending,
            // The PSU's ASPSP explicitly rejected (or the PSU cancelled) the
            // authorization.
            Self::NotAuthorised => AttemptStatus::AuthorizationFailed,
            // PSU authorization is the terminal charged state for FIRE's
            // open-banking flow; funds confirmation and settlement are
            // later stages of the same completed payment.
            Self::Authorised | Self::FundsConfirmed | Self::Settled => AttemptStatus::Charged,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirePaymentAttemptsResponse {
    pis_payment_request_payments: Vec<FirePaymentAttempt>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FirePaymentAttempt {
    /// "PAYMENT" for payment attempts; "REFUND" / "REFUND_REQUEST" for refund
    /// entries, which share this list and are skipped by PSync.
    #[serde(default)]
    transaction_type: Option<String>,
    status: FirePaymentStatus,
    payment_request_code: String,
    payment_uuid: String,
    currency: FireCurrency,
    amount_before_charges: MinorUnit,
    order_details: FireOrderDetailsResponse,
    date_created: Option<String>,
    last_updated: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FireCurrency {
    code: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FireOrderDetailsResponse {
    order_id: String,
}

pub(crate) fn payment_request_code_from_feature_data(
    feature_data: Option<&common_utils::pii::SecretSerdeValue>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    const FIELD: &str = "connector_feature_data.payment_request_code";
    let code = feature_data
        .and_then(|value| value.peek().as_object())
        .and_then(|object| object.get("payment_request_code"))
        .and_then(serde_json::Value::as_str)
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: FIELD,
            context: IntegrationErrorContext::default(),
        })?;
    if code.len() != 8 || !code.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: FIELD,
            context: IntegrationErrorContext::default(),
        }
        .into());
    }
    Ok(code.to_string())
}

impl TryFrom<ResponseRouterData<FirePaymentAttemptsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FirePaymentAttemptsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response_error =
            || crate::utils::response_handling_fail_for_connector(item.http_code, "fire");
        let expected_code = payment_request_code_from_feature_data(
            item.router_data.request.connector_feature_data.as_ref(),
        )
        .change_context(response_error())?;

        // The list also carries refund entries for this payment request;
        // only payment attempts decide the payment status.
        let attempts: Vec<FirePaymentAttempt> = item
            .response
            .pis_payment_request_payments
            .into_iter()
            .filter(|attempt| {
                !attempt
                    .transaction_type
                    .as_deref()
                    .is_some_and(is_fire_refund_transaction_type)
            })
            .collect();

        if attempts.is_empty() {
            let status = AttemptStatus::Pending;
            return Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: item.router_data.request.connector_transaction_id.clone(),
                    redirection_data: None,
                    connector_metadata: Some(serde_json::json!({
                        "payment_request_code": expected_code
                    })),
                    mandate_reference: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: item.http_code,
                    payment_account_reference: None,
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_status: Some(RawConnectorStatus {
                        code: Some(FIRE_PAYMENT_NOT_ATTEMPTED.to_string()),
                        message: None,
                        reason: None,
                    }),
                    settlement_status: Some(SettlementStatus::Unspecified),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let selected = select_payment_attempt(attempts).ok_or_else(response_error)?;
        // Integrity: `orderDetails.orderId` must echo the merchant transaction
        // id, and currency and `amountBeforeCharges` must match the original
        // payment. `paymentRequestCode` is only the lookup key and
        // `paymentUuid` can change between attempts, so neither is compared.
        let actual_order_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .as_str();
        if selected.order_details.order_id != actual_order_id {
            return Err(fire_integrity_error(
                item.http_code,
                "orderId",
                Some(selected.payment_uuid.clone()),
            ));
        }
        let currency =
            Currency::from_str(&selected.currency.code).change_context(response_error())?;
        if currency != item.router_data.request.money.currency() {
            return Err(fire_integrity_error(
                item.http_code,
                "currency",
                Some(selected.payment_uuid.clone()),
            ));
        }
        if selected.amount_before_charges != item.router_data.request.money.amount() {
            return Err(fire_integrity_error(
                item.http_code,
                "amountBeforeCharges",
                Some(selected.payment_uuid.clone()),
            ));
        }

        let raw_connector_status = fire_raw_connector_status(&selected.status);
        let status = selected.status.attempt_status();
        let settlement_status = fire_settlement_status(&selected.status);
        if domain_types::utils::is_payment_failure(status) {
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: FIRE_PAYMENT_NOT_AUTHORISED.to_string(),
                    message: NO_ERROR_MESSAGE.to_string(),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(selected.payment_uuid.clone()),
                    ..Default::default()
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    raw_connector_status,
                    settlement_status: Some(settlement_status),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let mut router_data = item.router_data;
        router_data.request.integrity_object = Some(PaymentSynIntegrityObject {
            amount: selected.amount_before_charges,
            currency,
        });
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(selected.payment_uuid),
                redirection_data: None,
                connector_metadata: Some(serde_json::json!({
                    "payment_request_code": expected_code
                })),
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                // FIRE echoes the merchant transaction id in
                // `orderDetails.orderId` (already integrity-checked above).
                connector_response_reference_id: Some(selected.order_details.order_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status,
                settlement_status: Some(settlement_status),
                ..router_data.resource_common_data
            },
            request: router_data.request,
            ..router_data
        })
    }
}

// Settlement phase per FIRE's Get Payment Details v2 reference
// (https://docs.fire.com/reference/getpaymentdetailsv2): AUTHORISED and
// FUNDS_CONFIRMED mean the PSU has completed the payment but funds are not
// settled to the merchant account yet; SETTLED is the terminal settled phase.
fn fire_settlement_status(status: &FirePaymentStatus) -> SettlementStatus {
    match status {
        FirePaymentStatus::Settled => SettlementStatus::Settled,
        FirePaymentStatus::Authorised | FirePaymentStatus::FundsConfirmed => {
            SettlementStatus::NotSettled
        }
        _ => SettlementStatus::Unspecified,
    }
}

fn fire_integrity_error(
    http_code: u16,
    field_name: &str,
    connector_transaction_id: Option<String>,
) -> error_stack::Report<errors::ConnectorError> {
    error_stack::report!(errors::ConnectorError::IntegrityCheckFailed {
        context: errors::ResponseTransformationErrorContext {
            http_status_code: Some(http_code),
            additional_context: Some(format!(
                "FIRE response {field_name} did not match the request"
            )),
        },
        field_names: field_name.to_string(),
        connector_transaction_id,
    })
}

fn select_payment_attempt(mut attempts: Vec<FirePaymentAttempt>) -> Option<FirePaymentAttempt> {
    if attempts
        .iter()
        .any(|attempt| attempt.status.is_successful())
    {
        attempts.retain(|attempt| attempt.status.is_successful());
    }
    if attempts.len() == 1 {
        return attempts.pop();
    }

    let parse_timestamp = |value: Option<&str>| {
        value.and_then(|value| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
        })
    };
    let timestamps = attempts
        .iter()
        .map(|attempt| {
            parse_timestamp(attempt.last_updated.as_deref())
                .or_else(|| parse_timestamp(attempt.date_created.as_deref()))
        })
        .collect::<Option<Vec<_>>>();

    match timestamps {
        Some(timestamps) => {
            let mut timestamped_attempts = timestamps
                .into_iter()
                .zip(attempts)
                .map(|(timestamp, attempt)| (attempt.status.is_successful(), timestamp, attempt))
                .collect::<Vec<_>>();
            timestamped_attempts
                .sort_by_key(|(is_authorised, timestamp, _)| (*is_authorised, *timestamp));
            timestamped_attempts.pop().map(|(_, _, attempt)| attempt)
        }
        // Some attempt has no usable `lastUpdated`/`dateCreated`, so the
        // attempts cannot be ranked by time: fall back to the first entry
        // FIRE returned instead of failing the sync.
        None => attempts.into_iter().next(),
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireCreatePaymentRequestResponse {
    code: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FireCreatePaymentRequestResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FireCreatePaymentRequestResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;
        let payment_request_code = item.response.code;
        let aspsp_uuid = fire_aspsp_uuid(&router_data.request.payment_method_data).change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "fire"),
        )?;
        // Bank Pay hosted page host, resolved per environment from config
        // (`fire.secondary_base_url`): https://preprod.securebankpay.com in
        // preprod, https://securebankpay.com in production.
        let payment_host = router_data
            .resource_common_data
            .connectors
            .fire
            .secondary_base_url
            .as_deref()
            .ok_or_else(|| {
                error_stack::report!(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "fire",
                ))
                .attach_printable(
                    "fire.secondary_base_url (Bank Pay hosted page host) is not configured",
                )
            })?;
        let redirect_url = match aspsp_uuid {
            Some(aspsp_uuid) => {
                format!("{payment_host}/{payment_request_code}/aspsps/{aspsp_uuid}")
            }
            None => format!("{payment_host}/{payment_request_code}"),
        };
        // The create response's only status-bearing field is the payment
        // request code; surface it verbatim as the raw connector status.
        let raw_connector_status = RawConnectorStatus {
            code: Some(payment_request_code.clone()),
            message: None,
            reason: None,
        };

        // FIRE's create response exposes the request code, but not amount or
        // currency. Preserve those request values until PSync can validate
        // them against FIRE's payment-attempt response.
        router_data.request.integrity_object = Some(AuthoriseIntegrityObject {
            amount: router_data.request.money.amount(),
            currency: router_data.request.currency,
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // The create endpoint returns a lookup code, not a transaction
                // UUID. PSync uses connector_metadata for the lookup and
                // returns paymentUuid as the connector transaction ID.
                resource_id: ResponseId::NoResponseId,
                redirection_data: Some(Box::new(RedirectForm::Uri { uri: redirect_url })),
                connector_metadata: Some(serde_json::json!({
                    "payment_request_code": payment_request_code
                })),
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Pending,
                raw_connector_status: Some(raw_connector_status),
                ..router_data.resource_common_data
            },
            request: router_data.request,
            ..router_data
        })
    }
}

fn fire_aspsp_uuid<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<Option<&'static str>, IntegrationError> {
    let issuer = match payment_method_data {
        PaymentMethodData::BankRedirect(BankRedirectData::OpenBankingUk { issuer, .. }) => {
            issuer.as_ref().copied()
        }
        PaymentMethodData::BankRedirect(BankRedirectData::OpenBanking { .. })
        | PaymentMethodData::OpenBanking(OpenBankingData::OpenBankingPIS {}) => return Ok(None),
        _ => {
            return Err(IntegrationError::not_implemented(
                "FIRE supports Open Banking payment methods only",
                IntegrationErrorContext::default(),
            ));
        }
    };

    let Some(issuer) = issuer else {
        return Ok(None);
    };
    // BankNames variants are self-disambiguating (the bank_of_ireland_uk
    // pattern): the variant alone identifies the ASPSP, country is ignored.
    let aspsp_uuid = match issuer {
        BankNames::AlliedIrishBankUk => FIRE_ASPSP_AIB_GB_PERSONAL,
        BankNames::BankOfIrelandUkBusiness => FIRE_ASPSP_BOI_GB_BUSINESS,
        BankNames::BankOfIrelandUk => FIRE_ASPSP_BOI_GB_PERSONAL,
        BankNames::AlliedIrishBankCorporate => FIRE_ASPSP_AIB_GB_BUSINESS,
        BankNames::BankOfScotlandBusiness => FIRE_ASPSP_BOS_GB_BUSINESS,
        BankNames::BankOfScotland => FIRE_ASPSP_BOS_GB_PERSONAL,
        BankNames::Barclays | BankNames::BarclaysBusiness => FIRE_ASPSP_BARCLAYS_GB,
        BankNames::AlliedIrishBank => FIRE_ASPSP_AIB_IE_PERSONAL,
        BankNames::BankOfIrelandBusiness => FIRE_ASPSP_BOI_IE_BUSINESS,
        BankNames::BankOfIreland => FIRE_ASPSP_BOI_IE_PERSONAL,
        _ => {
            return Err(IntegrationError::not_implemented(
                format!("FIRE does not support {issuer:?} as an ASPSP"),
                IntegrationErrorContext::default(),
            ));
        }
    };

    Ok(Some(aspsp_uuid))
}

// =============================================================================
// REFUND
// =============================================================================
// POST /business/v1/payments/{paymentUuid}/bankpayrefund (Bank Pay-specific,
// not part of FIRE's public API). Mandatory request fields per the vendor
// guide: currency, amount, reason, orderDetails. The response carries
// RECEIVED or REJECTED plus refundRequestUuid.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireRefundRequest {
    currency: Currency,
    amount: MinorUnit,
    my_ref: String,
    reason: String,
    order_details: FireRefundOrderDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FireRefundOrderDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_number: Option<u64>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FireRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for FireRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: FireRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let currency = item.router_data.request.currency;
        // FIRE requires a reason; fall back to a default when the merchant
        // sends none (or only whitespace).
        let reason = item
            .router_data
            .request
            .reason
            .clone()
            .filter(|reason| !reason.trim().is_empty())
            .unwrap_or_else(|| FIRE_DEFAULT_REFUND_REASON.to_string());

        Ok(Self {
            currency,
            amount: item.router_data.request.refund_money.amount(),
            // FIRE's myRef carries the merchant's refund id so the refund
            // can be reconciled against the merchant's records on FIRE's
            // side.
            my_ref: item.router_data.request.refund_id.clone(),
            reason,
            order_details: FireRefundOrderDetails {
                merchant_number: fire_merchant_number(
                    &item.router_data.resource_common_data.merchant_id,
                ),
            },
        })
    }
}

/// Refund status, in FIRE's SCREAMING_SNAKE_CASE wire spelling. FIRE reports
/// two: RECEIVED (terminal success) and REJECTED (terminal failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FireRefundStatus {
    Received,
    Rejected,
    /// A status this connector does not recognize. The raw wire value FIRE
    /// sent is preserved verbatim for `raw_connector_status`; it is treated
    /// as pending - never terminal. Untagged, so it (de)serializes as the
    /// bare string and catches any value the variants above do not match.
    #[serde(untagged)]
    Unknown(String),
}

impl FireRefundStatus {
    fn refund_status(&self) -> RefundStatus {
        match self {
            Self::Received => RefundStatus::Success,
            Self::Rejected => RefundStatus::Failure,
            // Never terminal: the webhook or a later RSync reconciles.
            Self::Unknown(_) => RefundStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireRefundResponse {
    #[serde(default)]
    status: Option<FireRefundStatus>,
    refund_request_uuid: Option<String>,
    payment_uuid: Option<String>,
}

impl TryFrom<ResponseRouterData<FireRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<FireRefundResponse, Self>) -> Result<Self, Self::Error> {
        // The raw status FIRE sent (when present) is kept verbatim in
        // raw_connector_status, including statuses we do not recognize.
        let raw_connector_status = item
            .response
            .status
            .as_ref()
            .and_then(fire_raw_connector_status);
        // FIRE reports two refund statuses: RECEIVED (terminal success) and
        // REJECTED (terminal failure). A missing or unrecognized status must
        // not fail the call - FIRE accepted the request, and erroring here
        // invites a duplicate refund on retry - so it stays pending for
        // RSync or the webhook to reconcile.
        let refund_status = item
            .response
            .status
            .as_ref()
            .map_or(RefundStatus::Pending, FireRefundStatus::refund_status);
        if refund_status == RefundStatus::Failure {
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: FIRE_REFUND_REJECTED.to_string(),
                    message: "FIRE rejected the refund request".to_string(),
                    reason: None,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Refund(refund_status)),
                    ..Default::default()
                }),
                resource_common_data: RefundFlowData {
                    raw_connector_status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // FIRE accepted the refund, so a missing `refundRequestUuid` must not
        // fail the call (a retry would create a duplicate refund). Fall back to
        // our own refund reference and keep the refund pending until a webhook
        // or manual reconciliation confirms it.
        let (connector_refund_id, refund_status) = match item.response.refund_request_uuid {
            Some(refund_request_uuid) => (refund_request_uuid, refund_status),
            None => (
                item.router_data.request.refund_id.clone(),
                RefundStatus::Pending,
            ),
        };
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                raw_connector_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REFUND SYNC
// =============================================================================
// FIRE has no refund-specific lookup: RSync uses the same List Payment
// Attempts call as PSync (GET /business/v2/paymentrequests/{code}/payments).
// The list carries payments and refunds side by side; refund entries have
// `transactionType` "REFUND" or "REFUND_REQUEST". The refund being synced is
// found by:
//   1. its FIRE uuid — `refundRequestUuid` (or, when absent, `paymentUuid`)
//      equal to connector_refund_id, the uuid the Refund call returned; else
//   2. its `myRef`, which the Refund call set to the merchant refund id.
//      This covers a connector_refund_id that is not a FIRE uuid: the Refund
//      response omitted `refundRequestUuid` (the Refund flow then stores the
//      merchant refund id), or the caller synced without one (RefundSyncData
//      falls back to the merchant refund id).

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireRefundSyncResponse {
    #[serde(default)]
    pis_payment_request_payments: Vec<FireRefundSyncEntry>,
}

/// One entry of the List Payment Attempts response, read leniently: only the
/// fields RSync needs, all optional except `status`, so payment entries (and
/// fields FIRE adds) never fail decoding.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FireRefundSyncEntry {
    #[serde(default)]
    transaction_type: Option<String>,
    status: FireRefundStatus,
    #[serde(default)]
    payment_uuid: Option<String>,
    #[serde(default)]
    refund_request_uuid: Option<String>,
    /// Echoes the Refund request's `myRef` — the merchant refund id.
    #[serde(default)]
    my_ref: Option<String>,
}

impl FireRefundSyncEntry {
    fn is_refund(&self) -> bool {
        self.transaction_type
            .as_deref()
            .is_some_and(is_fire_refund_transaction_type)
    }

    /// FIRE's id for this refund: `refundRequestUuid`, else `paymentUuid`.
    fn fire_refund_uuid(&self) -> Option<&str> {
        self.refund_request_uuid
            .as_deref()
            .or(self.payment_uuid.as_deref())
    }

    fn matches_fire_uuid(&self, connector_refund_id: &str) -> bool {
        self.fire_refund_uuid()
            .is_some_and(|uuid| uuid.eq_ignore_ascii_case(connector_refund_id))
    }

    fn matches_my_ref(&self, merchant_refund_id: &str) -> bool {
        self.my_ref.as_deref() == Some(merchant_refund_id)
    }
}

impl TryFrom<ResponseRouterData<FireRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FireRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let connector_refund_id = item.router_data.request.connector_refund_id.clone();
        let refunds = || {
            item.response
                .pis_payment_request_payments
                .iter()
                .filter(|entry| entry.is_refund())
        };
        // Merchant refund ids to try against `myRef`: the flow's refund id,
        // and connector_refund_id itself when it is a merchant refund id
        // rather than a FIRE uuid.
        let merchant_refund_ids = [
            item.router_data.resource_common_data.refund_id.as_deref(),
            Some(connector_refund_id.as_str()),
        ];
        let refund = refunds()
            .find(|entry| entry.matches_fire_uuid(&connector_refund_id))
            .or_else(|| {
                merchant_refund_ids
                    .into_iter()
                    .flatten()
                    .filter(|id| !id.is_empty())
                    .find_map(|id| refunds().find(|entry| entry.matches_my_ref(id)))
            });
        // Report FIRE's uuid once the refund is found, so later syncs and
        // webhooks key on it instead of the merchant refund id.
        let resolved_refund_id = refund
            .and_then(FireRefundSyncEntry::fire_refund_uuid)
            .map(str::to_string)
            .unwrap_or_else(|| connector_refund_id.clone());

        let (refund_status, raw_connector_status) = match refund {
            Some(refund) => (
                refund.status.refund_status(),
                fire_raw_connector_status(&refund.status),
            ),
            // The refund is not listed (yet) under the payment request:
            // never guess a terminal state — stay pending.
            None => (RefundStatus::Pending, None),
        };

        if refund_status == RefundStatus::Failure {
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: FIRE_REFUND_REJECTED.to_string(),
                    message: "FIRE rejected the refund request".to_string(),
                    reason: None,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Refund(refund_status)),
                    ..Default::default()
                }),
                resource_common_data: RefundFlowData {
                    raw_connector_status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: resolved_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                raw_connector_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// WEBHOOKS
// =============================================================================
// FIRE Bank Pay webhooks arrive as `application/jwt` bodies: RS256 in
// production (signing key published at the fixed JWKS URL above) or HS256
// in preprod (shared secret from the merchant webhook config). Two phases:
//   1. ParseEvent (stateless): decode the JWT payload WITHOUT verification and
//      classify the event / extract resource references.
//   2. HandleEvent: VerifyWebhookSource verifies the JWT — RS256 against the
//      vendor JWKS via the external flow, or HS256 in-process with the
//      merchant webhook secret — and process_*_webhook then maps the event.
// Observed preprod payload: there is no event-name string. `type` is a
// category (e.g. "OTHER"); classification rides on `status` (the same
// SCREAMING_SNAKE_CASE wire values as the synchronous API) plus
// `transactionType` (PAYMENT vs REFUND).

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FireJwks {
    #[serde(default)]
    pub keys: Vec<FireJwk>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FireJwk {
    #[serde(default)]
    pub kid: Option<String>,
    #[serde(default)]
    pub kty: Option<String>,
    #[serde(default)]
    pub n: Option<String>,
    #[serde(default)]
    pub e: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FireWebhookClaims {
    /// Category label, e.g. "OTHER". NOT the event type — kept only for raw
    /// visibility; classification uses `status` + `transaction_type`.
    #[serde(default, rename = "eventType", alias = "type", alias = "event")]
    pub event_type: Option<String>,
    /// The payment/refund status, using the same SCREAMING_SNAKE_CASE wire
    /// values as the synchronous API (AUTHORISED, NOT_AUTHORISED, ...).
    #[serde(default)]
    pub status: Option<String>,
    /// PAYMENT vs REFUND routing.
    #[serde(default, rename = "transactionType", alias = "transaction_type")]
    pub transaction_type: Option<String>,
    #[serde(default, rename = "paymentUuid", alias = "payment_uuid")]
    pub payment_uuid: Option<String>,
    #[serde(default, rename = "refundRequestUuid", alias = "refund_request_uuid")]
    pub refund_request_uuid: Option<String>,
    /// Echoes the `orderDetails` sent at Authorize; `orderId` is the merchant
    /// transaction id.
    #[serde(default, rename = "orderDetails", alias = "order_details")]
    pub order_details: Option<FireWebhookOrderDetails>,
    // The remaining claims mirror the Get Payment Details payload. All are
    // optional so a field FIRE omits (or adds) never fails decoding; they are
    // carried for `typed_connector_response` only.
    #[serde(default, rename = "paymentRequestCode")]
    pub payment_request_code: Option<String>,
    #[serde(default, rename = "relatedPaymentUuid")]
    pub related_payment_uuid: Option<String>,
    #[serde(default, rename = "endToEndId")]
    pub end_to_end_id: Option<String>,
    #[serde(default, rename = "domesticPaymentId")]
    pub domestic_payment_id: Option<String>,
    #[serde(default, rename = "webhookUrl")]
    pub webhook_url: Option<String>,
    #[serde(default, rename = "returnUrl")]
    pub return_url: Option<String>,
    #[serde(default)]
    pub currency: Option<FireWebhookCodeDescription>,
    #[serde(default, rename = "amountBeforeCharges")]
    pub amount_before_charges: Option<MinorUnit>,
    #[serde(default, rename = "myRef")]
    pub my_ref: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub to: Option<FireWebhookDestination>,
    #[serde(default)]
    pub bank: Option<FireWebhookBank>,
    #[serde(default, rename = "allowFopReceipt")]
    pub allow_fop_receipt: Option<bool>,
    #[serde(default, rename = "dateCreated")]
    pub date_created: Option<String>,
    #[serde(default, rename = "lastUpdated")]
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireWebhookOrderDetails {
    #[serde(default, alias = "order_id")]
    pub order_id: Option<String>,
    #[serde(default)]
    pub merchant_customer_identification: Option<Secret<String>>,
    #[serde(default)]
    pub delivery_address_line1: Option<Secret<String>>,
    #[serde(default)]
    pub delivery_address_line2: Option<Secret<String>>,
    #[serde(default)]
    pub delivery_city: Option<Secret<String>>,
    #[serde(default)]
    pub delivery_post_code: Option<Secret<String>>,
    #[serde(default)]
    pub delivery_country: Option<String>,
}

/// A `{ code, description }` pair, used for currency and country.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FireWebhookCodeDescription {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// The FIRE account that received the payment.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FireWebhookDestination {
    #[serde(default, rename = "type")]
    pub destination_type: Option<String>,
    #[serde(default)]
    pub account: Option<FireWebhookAccount>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireWebhookAccount {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub nsc: Option<Secret<String>>,
    #[serde(default)]
    pub account_number: Option<Secret<String>>,
    #[serde(default)]
    pub iban: Option<Secret<String>>,
}

/// The payer's bank (ASPSP) used for the Open Banking payment.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FireWebhookBank {
    #[serde(default)]
    pub aspsp_uuid: Option<String>,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub country: Option<FireWebhookCodeDescription>,
    #[serde(default)]
    pub currency: Option<FireWebhookCodeDescription>,
}

impl FireWebhookClaims {
    /// The merchant transaction id FIRE echoes back in `orderDetails.orderId`.
    pub fn merchant_transaction_id(&self) -> Option<String> {
        self.order_details
            .as_ref()
            .and_then(|details| details.order_id.clone())
    }

    /// `amountBeforeCharges` (minor units) in `currency.code`. `None` when
    /// either is missing or the currency code is not recognised, so a
    /// malformed amount never fails webhook decoding.
    pub fn money(&self) -> Option<Money> {
        let amount = self.amount_before_charges?;
        let currency = self
            .currency
            .as_ref()
            .and_then(|currency| currency.code.as_deref())
            .and_then(|code| Currency::from_str(code).ok())?;
        MinorUnitForConnector.convert_back(amount, currency).ok()
    }
}

/// Decode the JWT payload segment without verifying the signature. Only used
/// in the stateless ParseEvent phase; the signature is checked later by the
/// VerifyWebhookSource flow against the vendor JWKS.
pub(crate) fn parse_fire_webhook_claims(
    body: &[u8],
) -> Result<FireWebhookClaims, error_stack::Report<WebhookError>> {
    let jwt = std::str::from_utf8(body).change_context(WebhookError::WebhookBodyDecodingFailed)?;
    let payload_b64 = jwt
        .split('.')
        .nth(1)
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookBodyDecodingFailed))?;
    let claims_json = URL_SAFE_NO_PAD
        .decode(payload_b64.trim_end_matches('='))
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;
    serde_json::from_slice::<FireWebhookClaims>(&claims_json)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

/// Whether the JWT's JOSE header declares HS256 — FIRE's preprod signing
/// algorithm. Production webhooks are RS256 (verified against the vendor
/// JWKS by the external VerifyWebhookSource flow); preprod webhooks are
/// HS256 and verified in-process with the merchant webhook secret. Used by
/// the event service to route between the two paths.
pub(crate) fn fire_webhook_uses_hs256(body: &[u8]) -> bool {
    std::str::from_utf8(body)
        .ok()
        .and_then(|jwt| decode_header(jwt).ok())
        .is_some_and(|header| header.alg == Algorithm::HS256)
}

/// Verify a preprod HS256 webhook in-process against the merchant's webhook
/// secret (`webhook_secrets.secret` in the HandleEvent request — optional,
/// because the production RS256 path needs no secret). Fail-closed: a
/// missing or empty secret, a non-HS256 signature, or a bad HMAC all fail.
pub(crate) fn verify_fire_hs256_webhook(
    body: &[u8],
    secrets: Option<ConnectorWebhookSecrets>,
) -> Result<bool, error_stack::Report<WebhookError>> {
    let jwt = std::str::from_utf8(body).change_context(WebhookError::WebhookBodyDecodingFailed)?;
    // An empty secret is treated as absent: never verify against an empty key.
    let secrets = secrets
        .filter(|secrets| !secrets.secret.is_empty())
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookSourceVerificationFailed))?;
    let mut validation = Validation::new(Algorithm::HS256);
    // FIRE's webhook JWTs are not documented to carry `exp`; the HMAC
    // signature against the configured secret is the trust anchor here.
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    // Pinning the algorithm set to HS256 also rejects `alg: none` and any
    // RS256-signed token that reaches this path.
    Ok(
        decode::<serde_json::Value>(jwt, &DecodingKey::from_secret(&secrets.secret), &validation)
            .is_ok(),
    )
}

/// FIRE labels refund transactions "REFUND" or "REFUND_REQUEST"; the webhook
/// and RSync paths accept both.
fn is_fire_refund_transaction_type(transaction_type: &str) -> bool {
    transaction_type.eq_ignore_ascii_case("REFUND")
        || transaction_type.eq_ignore_ascii_case("REFUND_REQUEST")
}

pub(crate) fn get_fire_webhook_event_type(
    claims: &FireWebhookClaims,
) -> Result<EventType, error_stack::Report<WebhookError>> {
    // FIRE webhooks carry no event-name string (observed payload sends
    // `type: "OTHER"` as a category): classification rides on `status` —
    // the same wire values as the synchronous API — plus `transactionType`
    // for PAYMENT vs REFUND routing.
    let status = claims
        .status
        .as_deref()
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookEventTypeNotFound))?;

    if claims
        .transaction_type
        .as_deref()
        .is_some_and(is_fire_refund_transaction_type)
    {
        let refund_status: FireRefundStatus =
            serde_json::from_value(serde_json::Value::String(status.to_string()))
                .unwrap_or(FireRefundStatus::Unknown(status.to_string()));
        return match refund_status {
            FireRefundStatus::Rejected => Ok(EventType::RefundFailure),
            // RECEIVED is the terminal refund success.
            FireRefundStatus::Received => Ok(EventType::RefundSuccess),
            // Unrecognized statuses stay pending; RSync reconciles against
            // the synchronous API.
            FireRefundStatus::Unknown(_) => Ok(EventType::RefundProcessing),
        };
    }

    // The payment-status wire values are shared with the synchronous API,
    // so reuse the same enum (unknown statuses land in `Unknown(raw)` and
    // are treated as pending, never terminal).
    let payment_status: FirePaymentStatus =
        serde_json::from_value(serde_json::Value::String(status.to_string()))
            .unwrap_or(FirePaymentStatus::Unknown(status.to_string()));
    Ok(match payment_status {
        // PSU authorized the payment: a terminal charged state for FIRE
        // (see the PSync `attempt_status` mapping).
        FirePaymentStatus::Authorised | FirePaymentStatus::FundsConfirmed => {
            EventType::PaymentIntentAuthorizationSuccess
        }
        // Funds settled to the merchant's Fire account (terminal).
        FirePaymentStatus::Settled => EventType::PaymentIntentSuccess,
        // The PSU's ASPSP explicitly rejected (or the PSU cancelled) the
        // authorization.
        FirePaymentStatus::NotAuthorised => EventType::PaymentIntentFailure,
        // Created/awaiting the PSU, and unrecognized statuses: never
        // terminal — PSync reconciles, and the raw value surfaces through
        // `raw_connector_status`.
        FirePaymentStatus::AwaitingAuthorisation
        | FirePaymentStatus::AwaitingMultiAuthorisation
        | FirePaymentStatus::Pending
        | FirePaymentStatus::Unknown(_) => EventType::PaymentIntentProcessing,
    })
}

pub(crate) fn get_fire_payment_webhook_status(
    event_type: &EventType,
) -> Result<AttemptStatus, error_stack::Report<WebhookError>> {
    match event_type {
        // PSU authorized the payment: a terminal charged state for FIRE
        // (see the PSync `attempt_status` mapping).
        EventType::PaymentIntentAuthorizationSuccess => Ok(AttemptStatus::Charged),
        // Funds settled to the merchant's Fire account (terminal).
        EventType::PaymentIntentSuccess => Ok(AttemptStatus::Charged),
        // The PSU's authorization was rejected or the payment failed — same
        // mapping as the PSync `attempt_status` for NOT_AUTHORISED.
        EventType::PaymentIntentFailure => Ok(AttemptStatus::AuthorizationFailed),
        // Payment created, awaiting the PSU's authorization in their
        // ASPSP's app.
        EventType::PaymentIntentProcessing => Ok(AttemptStatus::Pending),
        _ => Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound)),
    }
}

pub(crate) fn get_fire_refund_webhook_status(
    event_type: &EventType,
) -> Result<RefundStatus, error_stack::Report<WebhookError>> {
    match event_type {
        EventType::RefundProcessing => Ok(RefundStatus::Pending),
        EventType::RefundSuccess => Ok(RefundStatus::Success),
        EventType::RefundFailure => Ok(RefundStatus::Failure),
        _ => Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound)),
    }
}

// VerifyWebhookSource: the outbound call fetched the JWKS; this transformer
// now verifies the stored webhook JWT against the matching RSA key.
impl TryFrom<ResponseRouterData<FireJwks, Self>>
    for RouterDataV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    >
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: ResponseRouterData<FireJwks, Self>) -> Result<Self, Self::Error> {
        let invalid_format = |field_name: &'static str| IntegrationError::InvalidDataFormat {
            field_name,
            context: IntegrationErrorContext::default(),
        };

        let jwt = std::str::from_utf8(item.router_data.request.webhook_body.as_slice())
            .change_context(invalid_format("webhook_body"))?;
        let header = decode_header(jwt).change_context(invalid_format("jwt_header"))?;
        if header.alg != Algorithm::RS256 {
            return Err(invalid_format("jwt_alg").into());
        }
        let jwk = if let Some(kid) = header.kid.as_deref() {
            item.response
                .keys
                .iter()
                .find(|key| key.kty.as_deref() == Some("RSA") && key.kid.as_deref() == Some(kid))
        } else {
            item.response
                .keys
                .iter()
                .find(|key| key.kty.as_deref() == Some("RSA"))
        }
        .ok_or_else(|| invalid_format("jwks"))?;
        let modulus = jwk
            .n
            .as_deref()
            .ok_or_else(|| invalid_format("jwk_modulus"))?;
        let exponent = jwk
            .e
            .as_deref()
            .ok_or_else(|| invalid_format("jwk_exponent"))?;
        let decoding_key = DecodingKey::from_rsa_components(modulus, exponent)
            .change_context(invalid_format("jwk"))?;
        let mut validation = Validation::new(Algorithm::RS256);
        // Fire's webhook JWTs are not documented to carry `exp`; the RS256
        // signature against the vendor JWKS is the trust anchor here.
        validation.required_spec_claims.clear();
        validation.validate_exp = false;
        let verified = decode::<serde_json::Value>(jwt, &decoding_key, &validation).is_ok();

        Ok(Self {
            response: Ok(VerifyWebhookSourceResponseData {
                verify_webhook_status: if verified {
                    VerifyWebhookStatus::SourceVerified
                } else {
                    VerifyWebhookStatus::SourceNotVerified
                },
            }),
            ..item.router_data
        })
    }
}
