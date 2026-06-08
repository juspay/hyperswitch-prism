//! Qwikcilver / QwikWallet — Pine Labs stored-value wallet connector.
//!
//! Surface (active flows in this build):
//!   1. POST `/api/v2/authorize`                — session login (ServerAuthenticationToken)
//!   2. POST `/api/v2/wallet/{wn}/REDEEM`       — debit (Authorize)
//!   3. POST `/api/v2/wallet/{wn}/CANCELREDEEM` — reverse a prior Redeem (Refund)
//!   4. POST `/api/v2/wallet/{wn}/card`         — credit value to the wallet
//!                                                (Recharge — refund top-ups,
//!                                                promo credits, loyalty,
//!                                                cashback, gift loads, …)
//!
//! Notes:
//! * `/authorize` returns JSON with **camelCase** field names; every other
//!   endpoint uses **PascalCase**. Two separate response envelopes capture this.
//! * Money is decimal in **major** units (e.g. `10.0` = 10 AED), handled by
//!   `QwikcilverAmountConvertor` (FloatMajorUnit).
//! * Every authenticated call requires a numeric `TransactionId` header and
//!   a `DateAtClient` header alongside the standard `Authorization: Bearer`.

use common_enums::{AttemptStatus, RechargeStatus, RefundStatus};
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{
        Authorize, CreatePaymentMethod, GetPaymentMethod, Recharge, Refund,
        ServerAuthenticationToken,
    },
    connector_types::{
        CreatePaymentMethodData, CreatePaymentMethodResponseData, GetPaymentMethodData,
        GetPaymentMethodResponseData, PaymentFlowData, PaymentMethodCustomerInfo,
        PaymentsAuthorizeData, PaymentsResponseData, RechargeRequestData, RechargeResponseData,
        RefundFlowData, RefundsData, RefundsResponseData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodDataTypes, PaymentMethodDetails, WalletDetails},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::qwikcilver::QwikcilverRouterData, types::ResponseRouterData};

/// Pine Labs developer portal. Surfaced on every Qwikcilver error so callers
/// can self-serve the schema / sandbox creds / endpoint reference without
/// pinging us.
pub(crate) const QWIKCILVER_INTEGRATION_DOC_URL: &str = "https://developers.qwikcilver.com/";

/// Build an `IntegrationErrorContext` with all three diagnostic fields wired
/// up consistently — keeps the error sites below from drowning in literals.
pub(crate) fn qc_err_ctx(
    additional_context: impl Into<String>,
    suggested_action: impl Into<String>,
) -> domain_types::errors::IntegrationErrorContext {
    domain_types::errors::IntegrationErrorContext {
        additional_context: Some(additional_context.into()),
        suggested_action: Some(suggested_action.into()),
        doc_url: Some(QWIKCILVER_INTEGRATION_DOC_URL.to_string()),
    }
}

// ============================================================================
// AUTH — resolved from `ConnectorSpecificConfig::Qwikcilver`
//
// `bootstrap_bearer_token` is the long-lived bearer the merchant gets from
// Pine Labs onboarding; it authorizes ONLY the `/authorize` call. That call
// returns a session JWT, which the framework caches on
// `RouterDataV2.access_token` and threads onto every subsequent flow via
// `Authorization: Bearer <session-jwt>`. The body credentials
// (`terminal_id` / `username` / `password`) are sent in the `/authorize`
// JSON body alongside.
// ============================================================================

#[derive(Debug, Clone)]
pub struct QwikcilverAuthType {
    pub(super) bootstrap_bearer_token: Secret<String>,
    pub(super) terminal_id: Secret<String>,
    pub(super) username: Secret<String>,
    pub(super) password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for QwikcilverAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(value: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match value {
            ConnectorSpecificConfig::Qwikcilver {
                bootstrap_bearer_token,
                terminal_id,
                username,
                password,
                ..
            } => Ok(Self {
                bootstrap_bearer_token: bootstrap_bearer_token.clone(),
                terminal_id: terminal_id.clone(),
                username: username.clone(),
                password: password.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: qc_err_ctx(
                    "x-connector-config did not deserialize as the `Qwikcilver` variant — \
                     the resolved variant doesn't carry the required Pine Labs fields.",
                    "Send `x-connector-config: {\"config\":{\"Qwikcilver\":{\"bootstrap_bearer_token\":\"…\",\
                     \"terminal_id\":\"…\",\"username\":\"…\",\"password\":\"…\"}}}` exactly. \
                     Header value parsing is case-sensitive; the variant key must be `Qwikcilver`.",
                ),
            }
            .into()),
        }
    }
}

// ============================================================================
// AUTHORIZE (session login) — `/api/v2/authorize`
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverAuthorizeRequest {
    #[serde(rename = "TerminalID")]
    pub terminal_id: Secret<String>,
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub transaction_id: u64,
    pub date_at_client: String,
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for QwikcilverAuthorizeRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = QwikcilverAuthType::try_from(&item.router_data.connector_config)?;
        let common = &item.router_data.resource_common_data;
        let transaction_id =
            transaction_id_from_reference(&common.connector_request_reference_id);
        let date_at_client =
            resolve_date_at_client(common.connector_feature_data.as_ref().map(|s| s.peek()))?;
        Ok(Self {
            terminal_id: auth.terminal_id,
            username: auth.username,
            password: auth.password,
            transaction_id,
            date_at_client,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QwikcilverAuthorizeResponse {
    pub auth_token: Secret<String>,
    pub merchant_outlet_info: Option<QwikcilverMerchantOutletInfo>,
    pub locale_info: Option<QwikcilverLocaleInfo>,
    pub receipt_info: Option<serde_json::Value>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub batch_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QwikcilverMerchantOutletInfo {
    pub merchant_outlet_name: Option<String>,
    pub merchant_outlet_address1: Option<String>,
    pub merchant_outlet_address2: Option<String>,
    pub merchant_outlet_city: Option<String>,
    pub merchant_outlet_state: Option<String>,
    pub merchant_outlet_pin_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QwikcilverLocaleInfo {
    pub culture: Option<String>,
    pub currency_symbol: Option<String>,
    pub currency_position: Option<i32>,
    pub currency_decimal_digits: Option<i32>,
    pub display_unit_for_points: Option<String>,
}

/// Qwikcilver JWTs themselves are valid 7 days (verified via the `exp`
/// claim — 604800s), but we expose a conservative 20-minute TTL to the
/// framework so the access-token cache refreshes frequently and stays
/// well clear of any upstream invalidation (e.g. ops-side revocation,
/// terminal rebinding) we can't see.
const SESSION_EXPIRY_SECONDS: i64 = 60 * 20;

impl<F>
    TryFrom<
        ResponseRouterData<
            QwikcilverAuthorizeResponse,
            RouterDataV2<
                F,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        >,
    >
    for RouterDataV2<
        F,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            QwikcilverAuthorizeResponse,
            RouterDataV2<
                F,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        // Persist the raw connector response on every path (success + error) so
        // it's available for audit logs, debugging, and event-bus consumers.
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&item.response).ok().map(Secret::new);
        if item.response.response_code != QWIKCILVER_SUCCESS_CODE {
            // Token-bootstrap errors are NOT payment attempt failures —
            // leave attempt_status unset so they don't surface in payment
            // success/failure dashboards.
            data.response = Err(make_error_response(
                item.response.response_code,
                item.response.response_message.clone(),
                None,
                None,
                None,
                item.http_code,
                None,
            ));
            return Ok(data);
        }
        data.response = Ok(ServerAuthenticationTokenResponseData {
            access_token: item.response.auth_token,
            token_type: Some("Bearer".to_string()),
            expires_in: Some(SESSION_EXPIRY_SECONDS),
        });
        Ok(data)
    }
}

// ============================================================================
// COMMON ENVELOPE (PascalCase) — non-auth endpoints. `ResponseCode == 0` is
// success; non-zero is a domain error returned as HTTP 200.
// ============================================================================

pub(crate) const QWIKCILVER_SUCCESS_CODE: i64 = 0;

/// Optional wallet sub-payload returned by the Add Card refund. Only
/// `current_batch_number`/`transaction_id` are consumed; the rest is here
/// so serde can deserialize the full Qwikcilver envelope without losing
/// fields.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverWallet {
    /// The wallet's PAN-equivalent identifier. PII.
    pub wallet_number: Secret<String>,
    pub external_wallet_id: Option<Secret<String>>,
    pub wallet_pin: Option<Secret<String>>,
    pub status: Option<String>,
    pub wallet_program_group_name: Option<String>,
    pub wallet_holder_name: Option<Secret<String>>,
    pub balance: FloatMajorUnit,
    pub consolidated_balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub customer: Option<QwikcilverCustomer>,
    pub card: Option<QwikcilverCard>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCustomer {
    pub customer_type: Option<String>,
    pub salutation: Option<String>,
    pub firstname: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub phone_number: Option<Secret<String>>,
    pub email: Option<Secret<String>>,
    #[serde(rename = "DOB")]
    pub dob: Option<Secret<String>>,
    pub address_line1: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    pub external_customer_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCard {
    /// Gift-card / refund-eCard credential issued by Pine Labs. PII.
    pub card_number: Secret<String>,
    pub amount: FloatMajorUnit,
    pub card_program_name: Option<String>,
    pub card_status: Option<String>,
    pub card_type: Option<String>,
    pub expiry: Option<String>,
    pub bucket_type: Option<String>,
    pub notes: Option<String>,
}

// ============================================================================
// REDEEM (Authorize → debit) — `/api/v2/wallet/{wn}/REDEEM`
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverRedeemRequest {
    pub idempotency_key: String,
    pub invoice_number: String,
    pub amount: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_amount: Option<FloatMajorUnit>,
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for QwikcilverRedeemRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: qc_err_ctx(
                    format!(
                        "Failed to convert Redeem amount {} {} to FloatMajorUnit. \
                         Qwikcilver expects major-unit decimals (e.g. 0.20 AED).",
                        item.router_data.request.minor_amount.get_amount_as_i64(),
                        item.router_data.request.currency,
                    ),
                    "Verify `amount.minor_amount` is a non-negative integer and \
                     `amount.currency` is a 3-letter ISO 4217 code that Pine Labs supports \
                     for your terminal (e.g. AED, INR).",
                ),
            })?;
        let invoice_number = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let idempotency_key = item
            .router_data
            .request
            .merchant_order_id
            .clone()
            .unwrap_or_else(|| invoice_number.clone());
        let notes = item.router_data.resource_common_data.description.clone();
        Ok(Self {
            idempotency_key,
            invoice_number,
            amount: amount.clone(),
            notes,
            // Pure Redeem with no upstream discount field: the cart's total
            // bill equals what we're charging the wallet for. If a future
            // proto field surfaces "amount before discount", switch to that.
            bill_amount: Some(amount),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverRedeemResponse {
    pub current_batch_number: i64,
    /// Wallet PAN-equivalent. PII.
    pub wallet_number: Secret<String>,
    pub invoice_number: Option<String>,
    pub date_at_server: Option<String>,
    pub batch_number: i64,
    pub amount: FloatMajorUnit,
    pub balance: Option<FloatMajorUnit>,
    pub consolidated_balance: Option<FloatMajorUnit>,
    pub bill_amount: Option<FloatMajorUnit>,
    pub excluded_buckets_balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub approval_code: Option<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub transaction_id: i64,
    pub transaction_type: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

impl<T>
    TryFrom<
        ResponseRouterData<
            QwikcilverRedeemResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            QwikcilverRedeemResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        if body.response_code != QWIKCILVER_SUCCESS_CODE {
            data.response = Err(make_error_response(
                body.response_code,
                body.response_message,
                body.error_code,
                body.error_description,
                Some(body.transaction_id.to_string()),
                item.http_code,
                Some(AttemptStatus::Failure),
            ));
            return Ok(data);
        }
        // `connector_metadata` is itself stored as `SecretSerdeValue` on
        // `RouterDataV2`, so the inner wallet_number is masked at the prism
        // layer. We must `.expose()` here to embed the raw value in JSON.
        let metadata = serde_json::json!({
            "batch_number": body.batch_number,
            "transaction_id": body.transaction_id,
            "wallet_number": body.wallet_number.expose(),
        });
        let resource_id = format_composite_txn_id(body.batch_number, body.transaction_id);
        data.resource_common_data.status = AttemptStatus::Charged;
        data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(resource_id),
            redirection_data: None,
            connector_metadata: Some(metadata),
            network_txn_id: None,
            connector_response_reference_id: body.invoice_number,
            incremental_authorization_allowed: None,
            mandate_reference: None,
            status_code: item.http_code,
        });
        Ok(data)
    }
}

// ============================================================================
// REFUND — Cancel Redeem (`/api/v2/wallet/{wn}/CANCELREDEEM`)
//
// Refund means "reverse a prior Redeem" here — money flows back out of the
// merchant's settlement and the wallet's balance is restored. The Add Card
// "credit value to a wallet" operation that used to also live behind Refund
// has moved to the dedicated `Recharge` flow below; it isn't a refund in
// any semantic sense (it issues fresh value, no prior debit required).
// ============================================================================

/// Connector-specific refund metadata passed through `refund_metadata` on
/// `PaymentService.Refund`. Typed end-to-end — no ad-hoc JSON poking.
///
/// The on-wire JSON shape lives in `refund_connector_metadata.value`:
///
/// ```json
/// {
///   "wallet_number":          "4999771007702947",
///   "original_batch_number":  17302801,
///   "original_transaction_id":3486942062100047824
/// }
/// ```
///
/// `wallet_number` is required. `original_batch_number` +
/// `original_transaction_id` are also required, but as a convenience we fall
/// back to parsing a composite `connector_transaction_id` of the form
/// `"{batch}:{txn}"` if the explicit fields are absent.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QwikcilverRefundMetadata {
    /// Destination wallet for the reversal. PII.
    pub wallet_number: Secret<String>,
    #[serde(default)]
    pub original_batch_number: Option<i64>,
    #[serde(default)]
    pub original_transaction_id: Option<i64>,
}

impl QwikcilverRefundMetadata {
    /// Typed extraction from the request; returns `MissingRequiredField` if
    /// `refund_metadata` is missing or doesn't parse as our schema.
    pub(crate) fn from_request(
        req: &RefundsData,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let raw = req.refund_connector_metadata.as_ref().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "refund_metadata",
                context: qc_err_ctx(
                    "Qwikcilver Cancel Redeem needs the wallet number and the original \
                     batch/transaction ids of the Redeem being reversed.",
                    "Send `refund_metadata` as a JSON-string on the gRPC request: \
                     `{\"wallet_number\":\"<wn>\",\"original_batch_number\":<i64>,\
                     \"original_transaction_id\":<i64>,\"date_at_client\":\"<ISO-8601>\"}`. \
                     Batch and txn id come from the Redeem response's `connector_transaction_id` \
                     (`\"<batch>:<txn>\"`).",
                ),
            })
        })?;
        serde_json::from_value(raw.clone().expose()).map_err(|e| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "refund_metadata",
                context: qc_err_ctx(
                    format!("invalid Qwikcilver refund_metadata: {e}"),
                    "Ensure `refund_metadata` is a JSON-encoded string (not a nested object) \
                     and that `original_batch_number` / `original_transaction_id` are JSON numbers, \
                     not strings.",
                ),
            })
        })
    }
}

/// Extract the destination wallet number from the typed `payment_method`
/// tree for Authorize / Redeem. Qwikcilver wallet ops are first-class
/// (since the `qwikcilver_direct` variant was added to the proto's
/// `PaymentMethod` oneof) — no more sniffing through
/// `connector_feature_data` JSON.
pub(crate) fn qwikcilver_wallet_number_from_authorize<T>(
    req: &PaymentsAuthorizeData<T>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    use domain_types::payment_method_data::{PaymentMethodData, WalletData};
    match &req.payment_method_data {
        PaymentMethodData::Wallet(WalletData::QwikcilverDirect(d)) => {
            Ok(d.wallet_number.clone())
        }
        _ => Err(error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "payment_method.qwikcilver_direct.wallet_number",
            context: qc_err_ctx(
                "Qwikcilver Redeem requires the destination wallet number via the typed \
                 `qwikcilver_direct` payment_method variant — no other PaymentMethod variant \
                 is supported on this connector.",
                "Send the body with \
                 `payment_method.payment_method.qwikcilver_direct.wallet_number: \"<wallet-number>\"`. \
                 Verify with the curl recipe at docs/qwikcilver-grpcurl-recipes.md.",
            ),
        })),
    }
}

/// Cancel Redeem body — reverses a prior Redeem on the same wallet.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCancelRedeemBody {
    pub original_batch_number: i64,
    pub original_transaction_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for QwikcilverCancelRedeemBody
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let metadata = QwikcilverRefundMetadata::from_request(req)?;
        let (batch, txn) = metadata
            .original_batch_number
            .zip(metadata.original_transaction_id)
            .or_else(|| parse_composite_txn_id(&req.connector_transaction_id))
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "refund_metadata.{original_batch_number,original_transaction_id}",
                    context: qc_err_ctx(
                        "Couldn't locate the original Redeem's batch/transaction pair — \
                         neither `refund_metadata` carried them explicitly nor was \
                         `connector_transaction_id` a parseable `<batch>:<txn>` composite.",
                        "Either set both `refund_metadata.original_batch_number` and \
                         `refund_metadata.original_transaction_id` (both i64), or pass the \
                         original Redeem's `connector_transaction_id` verbatim — Prism stores \
                         it as `\"<batch>:<txn>\"` precisely so it round-trips into Cancel Redeem.",
                    ),
                })
            })?;
        Ok(Self {
            original_batch_number: batch,
            original_transaction_id: txn,
            notes: req.reason.clone(),
        })
    }
}

/// Cancel Redeem response. Optional fields reflect Qwikcilver returning a
/// pared-down envelope on this endpoint; only the success-discriminator
/// (`response_code`), batch/txn ids, and the error fields are load-bearing.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCancelRedeemResponse {
    pub current_batch_number: i64,
    pub invoice_number: Option<String>,
    pub bill_amount: Option<FloatMajorUnit>,
    /// Wallet whose Redeem was reversed. PII.
    pub wallet_number: Option<Secret<String>>,
    /// The batch under which the reversal posted (distinct from
    /// `current_batch_number`, which is the merchant's running batch).
    pub batch_number: Option<i64>,
    pub amount: Option<FloatMajorUnit>,
    pub balance: Option<FloatMajorUnit>,
    pub consolidated_balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub approval_code: Option<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub transaction_id: i64,
    pub transaction_type: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

impl
    TryFrom<
        ResponseRouterData<
            QwikcilverCancelRedeemResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    > for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            QwikcilverCancelRedeemResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        if body.response_code != QWIKCILVER_SUCCESS_CODE {
            data.response = Err(make_error_response(
                body.response_code,
                body.response_message,
                body.error_code,
                body.error_description,
                Some(body.transaction_id.to_string()),
                item.http_code,
                Some(AttemptStatus::Failure),
            ));
            return Ok(data);
        }
        let batch = body.batch_number.unwrap_or(body.current_batch_number);
        let resource_id = format_composite_txn_id(batch, body.transaction_id);
        data.response = Ok(RefundsResponseData {
            connector_refund_id: resource_id,
            refund_status: RefundStatus::Success,
            status_code: item.http_code,
        });
        Ok(data)
    }
}

// ============================================================================
// RECHARGE — Add Card (`/api/v2/wallet/{wn}/card`)
//
// Credits value to an existing wallet by issuing a new "card" on it. The
// operation is intentionally use-case-agnostic — the same endpoint serves
// refund top-ups, promo credits, loyalty rewards, cashback, gift loads, …
//
// Domain → wire field mapping (RechargeRequestData → Qwikcilver POST body):
//   connector_payment_method_id → URL path /wallet/{wn}/card  (wallet number)
//   merchant_recharge_id        → IdempotencyKey  +  InvoiceNumber
//   product_id                  → CardProgramName (Pine Labs program, region-
//                                   specific — e.g. "Blue Retail UAE Refund
//                                   eCard" for UAE)
//   amount + currency           → Amount  (FloatMajorUnit, e.g. 10.0 AED)
//   description                 → Notes
// ============================================================================

/// Add Card wire body — credits a card to the wallet.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverRechargeRequest {
    pub idempotency_key: String,
    pub amount: FloatMajorUnit,
    pub card_program_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub invoice_number: String,
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
            T,
        >,
    > for QwikcilverRechargeRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let amount = item
            .connector
            .amount_converter
            .convert(req.amount, req.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: qc_err_ctx(
                    format!(
                        "Failed to convert Recharge amount {} {} to FloatMajorUnit.",
                        req.amount.get_amount_as_i64(),
                        req.currency,
                    ),
                    "Verify `amount.minor_amount` is a non-negative integer and \
                     `amount.currency` is supported by your Pine Labs program (e.g. AED for \
                     `Blue Retail UAE Refund eCard`).",
                ),
            })?;
        let idempotency_key = req.merchant_recharge_id.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "merchant_recharge_id",
                context: qc_err_ctx(
                    "Qwikcilver Add Card needs an idempotency key so repeated submissions of \
                     the same recharge don't double-credit the wallet.",
                    "Set `merchant_recharge_id` on every PaymentMethodService.Recharge request \
                     to a value unique per logical recharge attempt (e.g. \
                     `\"rch-<merchant>-<order>-<timestamp>\"`). Pine Labs maps it onto both \
                     `IdempotencyKey` and `InvoiceNumber` on the wire.",
                ),
            })
        })?;
        Ok(Self {
            invoice_number: idempotency_key.clone(),
            idempotency_key,
            amount,
            card_program_name: req.product_id.clone(),
            notes: req.description.clone(),
        })
    }
}

/// Add Card response. Carries a `Wallet` sub-payload reflecting the wallet's
/// post-credit snapshot, alongside the standard envelope.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverRechargeResponse {
    pub current_batch_number: i64,
    pub invoice_number: Option<String>,
    pub bill_amount: Option<FloatMajorUnit>,
    /// Snapshot of the wallet after the credit posted.
    pub wallet: Option<QwikcilverWallet>,
    pub amount: Option<FloatMajorUnit>,
    pub balance: Option<FloatMajorUnit>,
    pub consolidated_balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub approval_code: Option<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub transaction_id: i64,
    pub transaction_type: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

impl
    TryFrom<
        ResponseRouterData<
            QwikcilverRechargeResponse,
            RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
        >,
    > for RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            QwikcilverRechargeResponse,
            RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        if body.response_code != QWIKCILVER_SUCCESS_CODE {
            // Recharge failure: surface the connector error, leave
            // attempt_status unset (Recharge isn't an AttemptStatus flow).
            data.response = Err(make_error_response(
                body.response_code,
                body.response_message,
                body.error_code,
                body.error_description,
                Some(body.transaction_id.to_string()),
                item.http_code,
                None,
            ));
            return Ok(data);
        }
        let connector_recharge_id =
            format_composite_txn_id(body.current_batch_number, body.transaction_id);
        let merchant_recharge_id = data.request.merchant_recharge_id.clone();
        let connector_payment_method_id = body
            .wallet
            .as_ref()
            .map(|w| w.wallet_number.clone().expose())
            .or_else(|| data.request.connector_payment_method_id.clone());
        let recharge_currency = data.request.currency;
        let payment_method_details = body.wallet.as_ref().map(|w| {
            let balance = crate::connectors::qwikcilver::QwikcilverAmountConvertor::convert_back(
                w.balance,
                recharge_currency,
            )
            .ok();
            PaymentMethodDetails::Wallet(WalletDetails {
                wallet_account_id: w.wallet_number.clone().expose(),
                wallet_pin: w.wallet_pin.clone(),
                wallet_status: map_wallet_status(w.status.as_ref()),
                wallet_holder_name: w.wallet_holder_name.as_ref().map(|h| h.clone().expose()),
                balance,
                product_id: w.wallet_program_group_name.clone().unwrap_or_default(),
                // Pine Labs's Wallet.Card carries the most recently added
                // card, not a full inventory — see wallet_details_to_payment_method_details
                // for the same rationale.
                items: Vec::new(),
            })
        });
        data.response = Ok(RechargeResponseData {
            merchant_payment_method_id: data.request.merchant_payment_method_id.clone(),
            connector_payment_method_id,
            merchant_recharge_id,
            connector_recharge_id: Some(connector_recharge_id),
            status: RechargeStatus::Success,
            payment_method_details,
            status_code: item.http_code,
        });
        Ok(data)
    }
}

/// Composite `connector_transaction_id` for Qwikcilver: `"{batch}:{txn}"`.
///
/// Qwikcilver identifies transactions by a `(BatchNumber, TransactionId)`
/// pair rather than a single id, so we encode both into the single
/// `connector_transaction_id` slot the framework gives us. Refund →
/// CancelRedeem needs to recover the pair to address the original Redeem,
/// hence the inverse [`parse_composite_txn_id`].
///
/// Both writers (Redeem response, the metadata JSON) and the reader
/// (CancelRedeem fallback path) MUST route through this pair — if you
/// change the encoding here, update both.
pub(crate) fn format_composite_txn_id(batch: i64, txn: i64) -> String {
    format!("{batch}:{txn}")
}

/// Inverse of [`format_composite_txn_id`]. Returns `None` if the input
/// doesn't match `"{i64}:{i64}"` exactly.
pub(crate) fn parse_composite_txn_id(id: &str) -> Option<(i64, i64)> {
    let mut parts = id.splitn(2, ':');
    let batch = parts.next()?.parse().ok()?;
    let txn = parts.next()?.parse().ok()?;
    Some((batch, txn))
}

// ============================================================================
// CREATE WALLET — POST `/api/v2/wallet`
//
// Provisions a new wallet for a customer. Wire shape per the QwikWallet
// V2 PDF §10.2:
//   Request body : { Externalwalletid, WalletProgramGroupName, Customer{…}, Notes }
//   Response     : { Wallet{WalletNumber, ExternalWalletId, Status, …, Customer{…}},
//                    CurrentBatchNumber, ResponseCode, ResponseMessage, TransactionId, … }
//
// Domain mapping (CreatePaymentMethodData → wire):
//   customer.phone_number       → Externalwalletid (customer mobile)
//   customer.first_name/last_name/email → Customer.* (passed through verbatim)
//   description                 → Notes
//   product_id  (NOT present in the domain type) → WalletProgramGroupName,
//     so we fetch it from the description for now; future revision can move
//     it into `connector_feature_data` JSON.
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCreateWalletRequest {
    /// Customer's unique mobile number used to associate the wallet.
    /// Stored as ExternalWalletId on the connector side. PII.
    #[serde(rename = "Externalwalletid")]
    pub externalwalletid: Secret<String>,
    pub wallet_program_group_name: String,
    pub customer: QwikcilverCreateCustomer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCreateCustomer {
    pub customer_type: Option<String>,
    pub salutation: String,
    pub firstname: Secret<String>,
    pub last_name: Secret<String>,
    pub phone_number: Secret<String>,
    pub email: Secret<String>,
    pub prefered_notification_language: String,
}

/// `connector_feature_data` carries connector-specific fields the generic
/// domain type can't express. For Create we need at least
/// `wallet_program_group_name` (Pine Labs program identifier, region-specific).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct QwikcilverCreateFeatureData {
    #[serde(default)]
    pub wallet_program_group_name: Option<String>,
}

impl QwikcilverCreateFeatureData {
    /// Typed extraction. Absent feature data is fine (the surrounding
    /// `try_from` reports a `MissingRequiredField` on the actual missing
    /// scalar field), but *malformed* feature data must surface as
    /// `InvalidDataFormat` so callers can fix the payload instead of
    /// chasing a misleading `wallet_program_group_name` error.
    pub(crate) fn from_request(
        req: &CreatePaymentMethodData,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let Some(raw) = req.connector_feature_data.as_ref() else {
            return Ok(Self::default());
        };
        serde_json::from_value(raw.clone().expose()).map_err(|e| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_feature_data",
                context: qc_err_ctx(
                    format!("invalid Qwikcilver connector_feature_data for Create: {e}"),
                    "Send `connector_feature_data` as a JSON-string (e.g. \
                     `\"{\\\"wallet_program_group_name\\\":\\\"…\\\",\\\"date_at_client\\\":\\\"…\\\",\
                     \\\"currency\\\":\\\"AED\\\"}\"`) — not a nested JSON object. The string is \
                     parsed server-side after secret-unwrap.",
                ),
            })
        })
    }
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<
                CreatePaymentMethod,
                PaymentFlowData,
                CreatePaymentMethodData,
                CreatePaymentMethodResponseData,
            >,
            T,
        >,
    > for QwikcilverCreateWalletRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<
                CreatePaymentMethod,
                PaymentFlowData,
                CreatePaymentMethodData,
                CreatePaymentMethodResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let customer = req.customer.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "customer",
                context: qc_err_ctx(
                    "Qwikcilver provisions wallets against a real customer record — the \
                     `customer` block on PaymentMethodService.Create is required, not optional.",
                    "Send a full `customer` object including `first_name` and `phone_number` \
                     at minimum. `phone_number` becomes the wallet's `ExternalWalletId`.",
                ),
            })
        })?;
        let phone = customer.phone_number.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "customer.phone_number",
                context: qc_err_ctx(
                    "Pine Labs uses the customer's phone number as `ExternalWalletId`, which is \
                     how subsequent lookups (Get / Recharge / Redeem) identify the wallet's owner.",
                    "Set `customer.phone_number` to the mobile number in E.164 or local format. \
                     Country-code rules vary by Pine Labs region/program.",
                ),
            })
        })?;
        let first = customer.first_name.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "customer.first_name",
                context: qc_err_ctx(
                    "Qwikcilver requires a non-empty first name on wallet creation — the wallet \
                     holder name is rendered on receipts and reports.",
                    "Set `customer.first_name`. `last_name` is optional but recommended.",
                ),
            })
        })?;
        let feature = QwikcilverCreateFeatureData::from_request(req)?;
        let program = feature.wallet_program_group_name.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_feature_data.wallet_program_group_name",
                context: qc_err_ctx(
                    "Pine Labs requires a region-specific program identifier to know which \
                     `WalletProgramGroup` the new wallet belongs to.",
                    "Set `connector_feature_data` as a JSON-string containing \
                     `wallet_program_group_name` (e.g. `\"PAWC-AFG Payblue Wallet UAE\"`). \
                     Ask Pine Labs ops for the exact program name configured against your terminal.",
                ),
            })
        })?;
        Ok(Self {
            externalwalletid: phone.clone(),
            wallet_program_group_name: program,
            customer: QwikcilverCreateCustomer {
                customer_type: None,
                salutation: String::new(),
                firstname: first,
                last_name: customer
                    .last_name
                    .clone()
                    .unwrap_or_else(|| Secret::new(String::new())),
                phone_number: phone,
                email: customer
                    .email
                    .as_ref()
                    .map(|e| Secret::new(e.peek().to_string()))
                    .unwrap_or_else(|| Secret::new(String::new())),
                prefered_notification_language: "tel".to_string(),
            },
            notes: req.description.clone(),
        })
    }
}

/// Get Wallet uses an empty request body — only headers + URL.
#[derive(Debug, Serialize)]
pub struct QwikcilverEmptyBody {}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >,
            T,
        >,
    > for QwikcilverEmptyBody
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: QwikcilverRouterData<
            RouterDataV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

/// Shared Create/Get response envelope. Both endpoints return the same
/// `Wallet{…}` sub-payload + standard envelope (see PDF §10.2.2 and §10.3.2).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverWalletEnvelope {
    pub wallet: Option<QwikcilverWalletDetails>,
    pub current_batch_number: Option<i64>,
    pub notes: Option<String>,
    pub approval_code: Option<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub transaction_id: Option<i64>,
    pub transaction_type: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

/// Newtype wrapper for the Get response. Get + Create return the same wire
/// envelope but the framework's `create_all_prerequisites!` macro requires
/// distinct response types per flow (it generates a `*Templating` struct
/// per response, and duplicate names collide). The wrapper deserializes
/// transparently — same shape, just a distinct Rust nominal type.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct QwikcilverGetWalletResponse(pub QwikcilverWalletEnvelope);

/// Detailed Wallet snapshot returned in Create/Get responses. Wider than the
/// `QwikcilverWallet` used by Recharge because Create/Get return a fuller
/// customer record alongside the wallet basics.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverWalletDetails {
    /// Wallet PAN-equivalent identifier. PII.
    pub wallet_number: Secret<String>,
    pub external_wallet_id: Option<Secret<String>>,
    pub wallet_pin: Option<Secret<String>>,
    pub status: Option<String>,
    /// Mag-stripe Track 1/2 content for physical-card-backed wallets. PII.
    /// Always `null` for our virtual wallet deployment, but kept Secret so
    /// a future physical-card rollout doesn't start dumping it to logs.
    pub track_data: Option<Secret<String>>,
    /// Barcode payload — typically encodes the wallet number. PII.
    pub bar_code: Option<Secret<String>>,
    pub wallet_program_group_name: Option<String>,
    pub wallet_holder_name: Option<Secret<String>>,
    pub balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub card: Option<serde_json::Value>,
    pub customer: Option<QwikcilverCustomerDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCustomerDetails {
    pub customer_type: Option<String>,
    pub salutation: Option<String>,
    pub firstname: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub phone_number: Option<Secret<String>>,
    pub email: Option<Secret<String>>,
    #[serde(rename = "DOB")]
    pub dob: Option<Secret<String>>,
    pub external_customer_id: Option<Secret<String>>,
}

/// Map Qwikcilver's free-text wallet status string to the typed domain
/// enum. Unknown / blank → `None` (don't silently lie via `Unspecified`).
fn map_wallet_status(s: Option<&String>) -> Option<common_enums::WalletStatus> {
    s.and_then(|raw| match raw.to_ascii_uppercase().as_str() {
        "ACTIVE" => Some(common_enums::WalletStatus::Active),
        "INACTIVE" => Some(common_enums::WalletStatus::Inactive),
        _ => None,
    })
}

fn wallet_details_to_payment_method_details(
    wallet: &QwikcilverWalletDetails,
    currency: Option<common_enums::Currency>,
) -> PaymentMethodDetails {
    // Qwikcilver returns balance in major units (e.g. `12.5` AED). We need
    // a currency exponent to convert to MinorUnit — caller passes it in
    // (typically from `connector_feature_data.currency`). Without it we
    // leave balance unset rather than risk a wrong conversion.
    let balance = wallet
        .balance
        .zip(currency)
        .and_then(|(b, c)| {
            crate::connectors::qwikcilver::QwikcilverAmountConvertor::convert_back(b, c).ok()
        });
    PaymentMethodDetails::Wallet(WalletDetails {
        wallet_account_id: wallet.wallet_number.clone().expose(),
        wallet_pin: wallet.wallet_pin.clone(),
        wallet_status: map_wallet_status(wallet.status.as_ref()),
        wallet_holder_name: wallet
            .wallet_holder_name
            .as_ref()
            .map(|h| h.clone().expose()),
        balance,
        product_id: wallet.wallet_program_group_name.clone().unwrap_or_default(),
        // Pine Labs's Wallet.Card carries the most recently added card (the one
        // from the current operation when it's an Add Card), NOT a snapshot of
        // all cards in the wallet. Mapping it onto `items` would suggest the
        // wallet holds exactly one card. Leaving empty until Qwikcilver exposes
        // a wallet-contents endpoint (none today, per PDF §9).
        items: Vec::new(),
    })
}

/// Pull an optional `currency` out of the caller's `connector_feature_data`
/// for response-side balance conversion. Accepts the standard ISO 4217 code
/// (`"AED"`, `"INR"`, …) — same shape the rest of the API uses.
fn currency_from_feature_data(
    feature: Option<&common_utils::pii::SecretSerdeValue>,
) -> Option<common_enums::Currency> {
    feature
        .map(|s| s.peek())
        .and_then(|v| v.get("currency"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
}

fn customer_details_to_payment_method_customer_info(
    customer: &QwikcilverCustomerDetails,
    wallet_external_id: Option<&Secret<String>>,
) -> PaymentMethodCustomerInfo {
    // Pine Labs convention: ExternalWalletId IS the customer's mobile. Use it
    // as a fallback when the Customer block doesn't carry phone_number directly.
    let phone_number = customer
        .phone_number
        .clone()
        .or_else(|| wallet_external_id.cloned());
    PaymentMethodCustomerInfo {
        merchant_customer_id: customer
            .external_customer_id
            .as_ref()
            .map(|id| id.clone().expose()),
        first_name: customer.firstname.clone(),
        last_name: customer.last_name.clone(),
        email: customer
            .email
            .as_ref()
            .and_then(|e| common_utils::pii::Email::try_from(e.clone().expose()).ok()),
        phone_number,
    }
}

impl
    TryFrom<
        ResponseRouterData<
            QwikcilverWalletEnvelope,
            RouterDataV2<
                CreatePaymentMethod,
                PaymentFlowData,
                CreatePaymentMethodData,
                CreatePaymentMethodResponseData,
            >,
        >,
    >
    for RouterDataV2<
        CreatePaymentMethod,
        PaymentFlowData,
        CreatePaymentMethodData,
        CreatePaymentMethodResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            QwikcilverWalletEnvelope,
            RouterDataV2<
                CreatePaymentMethod,
                PaymentFlowData,
                CreatePaymentMethodData,
                CreatePaymentMethodResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        if body.response_code != QWIKCILVER_SUCCESS_CODE {
            data.response = Err(make_error_response(
                body.response_code,
                body.response_message,
                body.error_code,
                body.error_description,
                body.transaction_id.map(|t| t.to_string()),
                item.http_code,
                None,
            ));
            return Ok(data);
        }
        let merchant_pm_id = data.request.merchant_payment_method_id.clone();
        let currency = currency_from_feature_data(data.request.connector_feature_data.as_ref());
        let (connector_pm_id, payment_method_details, customer) =
            if let Some(wallet) = body.wallet.as_ref() {
                (
                    Some(wallet.wallet_number.clone().expose()),
                    Some(wallet_details_to_payment_method_details(wallet, currency)),
                    wallet.customer.as_ref().map(|c| {
                        customer_details_to_payment_method_customer_info(
                            c,
                            wallet.external_wallet_id.as_ref(),
                        )
                    }),
                )
            } else {
                (None, None, None)
            };
        data.response = Ok(CreatePaymentMethodResponseData {
            merchant_payment_method_id: merchant_pm_id,
            connector_payment_method_id: connector_pm_id,
            payment_method_details,
            customer,
            status_code: item.http_code,
        });
        Ok(data)
    }
}

impl
    TryFrom<
        ResponseRouterData<
            QwikcilverGetWalletResponse,
            RouterDataV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >,
        >,
    >
    for RouterDataV2<
        GetPaymentMethod,
        PaymentFlowData,
        GetPaymentMethodData,
        GetPaymentMethodResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            QwikcilverGetWalletResponse,
            RouterDataV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response.0;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        if body.response_code != QWIKCILVER_SUCCESS_CODE {
            data.response = Err(make_error_response(
                body.response_code,
                body.response_message,
                body.error_code,
                body.error_description,
                body.transaction_id.map(|t| t.to_string()),
                item.http_code,
                None,
            ));
            return Ok(data);
        }
        let merchant_pm_id = data.request.merchant_payment_method_id.clone();
        let currency = currency_from_feature_data(data.request.connector_feature_data.as_ref());
        let (connector_pm_id, payment_method_details, customer) =
            if let Some(wallet) = body.wallet.as_ref() {
                (
                    Some(wallet.wallet_number.clone().expose()),
                    Some(wallet_details_to_payment_method_details(wallet, currency)),
                    wallet.customer.as_ref().map(|c| {
                        customer_details_to_payment_method_customer_info(
                            c,
                            wallet.external_wallet_id.as_ref(),
                        )
                    }),
                )
            } else {
                (data.request.connector_payment_method_id.clone(), None, None)
            };
        data.response = Ok(GetPaymentMethodResponseData {
            merchant_payment_method_id: merchant_pm_id,
            connector_payment_method_id: connector_pm_id,
            payment_method_details,
            customer,
            status_code: item.http_code,
        });
        Ok(data)
    }
}

// ============================================================================
// ERROR ENVELOPE — parsed by ConnectorCommon::build_error_response when a
// non-2xx HTTP status comes back.
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverErrorResponse {
    pub response_code: Option<i64>,
    pub response_message: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    /// PII.
    pub wallet_number: Option<Secret<String>>,
}

// ============================================================================
// HELPERS
// ============================================================================

/// Derive Qwikcilver's numeric `TransactionId` header from the caller's
/// `connector_request_reference_id`. Qwikcilver requires a positive i64;
/// we take the first 8 bytes of the SHA-256 of the reference id and mask
/// to 62 bits. Deterministic: same reference id → same TransactionId, so
/// callers can reproduce/correlate replays without us generating anything
/// on the server side.
pub(crate) fn transaction_id_from_reference(reference_id: &str) -> u64 {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(reference_id.as_bytes());
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(buf) & 0x3FFF_FFFF_FFFF_FFFF
}

/// Resolve the `DateAtClient` header. Strict: the caller MUST supply it
/// via `connector_feature_data.date_at_client`; we never substitute a
/// server-side `now()`, because every header value Qwikcilver sees needs
/// to be reconcilable from the caller's request.
pub(crate) fn resolve_date_at_client(
    feature: Option<&serde_json::Value>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    feature
        .and_then(|v| v.get("date_at_client"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_feature_data.date_at_client",
                context: qc_err_ctx(
                    "Qwikcilver hard-requires a `DateAtClient` header on every authenticated \
                     call (including the JWT-bootstrap), and Prism never substitutes a server-\
                     side `now()` so the value is fully reconcilable from the caller's request.",
                    "Send `connector_feature_data` as a JSON-string with a `date_at_client` key \
                     in ISO-8601/RFC-3339 (e.g. `\"2026-06-08T12:00:00Z\"`). For composite Refund, \
                     it also needs to live inside `refund_metadata`. The Postman collection uses \
                     `{{$isoTimestamp}}` to set it per send.",
                ),
            })
        })
}

/// Build an `ErrorResponse` from a Qwikcilver domain error. `attempt_status`
/// is parameterized because the token-bootstrap flow shouldn't tag its
/// errors as `AttemptStatus::Failure` (those metrics roll into the payment
/// success/failure pipeline); payment flows pass `Some(Failure)`.
fn make_error_response(
    response_code: i64,
    response_message: Option<String>,
    error_code: Option<String>,
    error_description: Option<String>,
    connector_txn_id: Option<String>,
    http_code: u16,
    attempt_status: Option<AttemptStatus>,
) -> ErrorResponse {
    ErrorResponse {
        status_code: http_code,
        code: error_code.unwrap_or_else(|| response_code.to_string()),
        message: response_message
            .clone()
            .or_else(|| error_description.clone())
            .unwrap_or_else(|| "Qwikcilver returned a non-zero response code".to_string()),
        reason: error_description.or(response_message),
        attempt_status,
        connector_transaction_id: connector_txn_id,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_id_from_reference_is_deterministic_and_fits_in_i64() {
        let inputs = [
            "a1b2c3d4-e5f6-7890-abcd-ef0123456789",
            "qc-redeem-1780660887",
            "",
            "🚀",
        ];
        for input in inputs {
            let a = transaction_id_from_reference(input);
            let b = transaction_id_from_reference(input);
            assert_eq!(a, b, "non-deterministic for {input:?}");
            assert!(a <= i64::MAX as u64, "id {a} exceeds i64::MAX for {input:?}");
        }
        // Different inputs should generally produce different ids.
        assert_ne!(
            transaction_id_from_reference("ref-1"),
            transaction_id_from_reference("ref-2"),
        );
    }

    #[test]
    fn composite_txn_id_round_trip() {
        let cases: &[(i64, i64)] = &[
            (1, 1),
            (17_302_801, 4_316_951_125_949_827_754),
            (i64::MAX, i64::MAX),
            (0, 0),
        ];
        for &(batch, txn) in cases {
            let s = format_composite_txn_id(batch, txn);
            let (b, t) = parse_composite_txn_id(&s).expect("round-trip parses");
            assert_eq!((b, t), (batch, txn));
        }
    }

    #[test]
    fn parse_composite_txn_id_rejects_malformed() {
        for bad in ["", "123", "a:b", "123:", ":456", "1:2:3"] {
            assert!(parse_composite_txn_id(bad).is_none(), "should reject {bad:?}");
        }
    }

    #[test]
    fn refund_metadata_parses_happy() {
        let raw = serde_json::json!({
            "wallet_number": "4999771007702947",
            "original_batch_number": 17_306_153_i64,
            "original_transaction_id": 1_129_324_760_429_976_552_i64,
        });
        let parsed: QwikcilverRefundMetadata = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.wallet_number.peek(), "4999771007702947");
        assert_eq!(parsed.original_batch_number, Some(17_306_153));
        assert_eq!(parsed.original_transaction_id, Some(1_129_324_760_429_976_552));
    }

    #[test]
    fn refund_metadata_rejects_missing_wallet_number() {
        let raw = serde_json::json!({ "original_batch_number": 1 });
        let parsed: Result<QwikcilverRefundMetadata, _> = serde_json::from_value(raw);
        assert!(parsed.is_err());
    }

    fn make_create_req(
        feature: Option<common_utils::pii::SecretSerdeValue>,
    ) -> CreatePaymentMethodData {
        CreatePaymentMethodData {
            merchant_payment_method_id: None,
            customer: None,
            description: None,
            payment_method_type: common_enums::PaymentMethodType::QwikcilverDirect,
            connector_feature_data: feature,
        }
    }

    #[test]
    fn create_feature_data_absent_returns_default() {
        let req = make_create_req(None);
        let out = QwikcilverCreateFeatureData::from_request(&req).expect("absent is ok");
        assert!(out.wallet_program_group_name.is_none());
    }

    #[test]
    fn create_feature_data_malformed_returns_error() {
        let req = make_create_req(Some(Secret::new(serde_json::json!("not-an-object"))));
        let err = QwikcilverCreateFeatureData::from_request(&req).unwrap_err();
        match err.current_context() {
            IntegrationError::InvalidDataFormat { field_name, .. } => {
                assert_eq!(*field_name, "connector_feature_data");
            }
            other => panic!("expected InvalidDataFormat, got {other:?}"),
        }
    }

    #[test]
    fn resolve_date_at_client_uses_provided() {
        let v = serde_json::json!({ "date_at_client": "2024-01-01T00:00:00Z" });
        assert_eq!(resolve_date_at_client(Some(&v)).unwrap(), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn resolve_date_at_client_hard_errors_when_missing() {
        for missing in [None, Some(serde_json::json!({})), Some(serde_json::json!({"other": "x"})), Some(serde_json::json!({"date_at_client": ""}))] {
            let err = resolve_date_at_client(missing.as_ref()).unwrap_err();
            match err.current_context() {
                IntegrationError::MissingRequiredField { field_name, .. } => {
                    assert_eq!(*field_name, "connector_feature_data.date_at_client");
                }
                other => panic!("expected MissingRequiredField, got {other:?}"),
            }
        }
    }

    #[test]
    fn map_wallet_status_covers_known_strings() {
        let active = "ACTIVE".to_string();
        let active_mixed = "Active".to_string();
        let inactive_upper = "INACTIVE".to_string();
        let inactive_mixed = "Inactive".to_string();
        let unknown = "foo".to_string();
        let empty = String::new();

        assert_eq!(map_wallet_status(Some(&active)), Some(common_enums::WalletStatus::Active));
        assert_eq!(map_wallet_status(Some(&active_mixed)), Some(common_enums::WalletStatus::Active));
        assert_eq!(map_wallet_status(Some(&inactive_upper)), Some(common_enums::WalletStatus::Inactive));
        assert_eq!(map_wallet_status(Some(&inactive_mixed)), Some(common_enums::WalletStatus::Inactive));
        assert_eq!(map_wallet_status(Some(&unknown)), None);
        assert_eq!(map_wallet_status(Some(&empty)), None);
        assert_eq!(map_wallet_status(None), None);
    }
}
