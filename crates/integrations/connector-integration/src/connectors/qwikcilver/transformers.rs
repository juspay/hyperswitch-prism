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
    connector_flow::{Authorize, Recharge, Refund, ServerAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, RechargeRequestData,
        RechargeResponseData, RefundFlowData, RefundsData, RefundsResponseData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodDataTypes, PaymentMethodDetails, WalletDetails},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::qwikcilver::QwikcilverRouterData, types::ResponseRouterData};

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
                context: Default::default(),
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
        Ok(Self {
            terminal_id: auth.terminal_id,
            username: auth.username,
            password: auth.password,
            transaction_id: numeric_transaction_id(),
            date_at_client: current_datetime_qwikcilver(),
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
    pub card_number: String,
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
    pub idempotencykey: String,
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
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for QwikcilverRedeemRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
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
                context: Default::default(),
            })?;
        let invoice_number = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let idempotencykey = item
            .router_data
            .request
            .merchant_order_id
            .clone()
            .unwrap_or_else(|| invoice_number.clone());
        Ok(Self {
            idempotencykey,
            invoice_number,
            amount,
            notes: None,
            bill_amount: None,
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
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    >
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            QwikcilverRedeemResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
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
                context: Default::default(),
            })
        })?;
        serde_json::from_value(raw.clone().expose()).map_err(|e| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "refund_metadata",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(format!("invalid Qwikcilver refund_metadata: {e}")),
                    ..Default::default()
                },
            })
        })
    }
}

/// Connector-specific feature data passed through `connector_feature_data`
/// on `PaymentService.Authorize`. Carries the destination wallet number for
/// Qwikcilver Redeem.
///
/// On-wire shape (inside `connector_feature_data.value`):
///
/// ```json
/// { "wallet_number": "4999771007702947" }
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QwikcilverAuthorizeFeatureData {
    /// Destination wallet for the Redeem. PII.
    pub wallet_number: Secret<String>,
}

impl QwikcilverAuthorizeFeatureData {
    pub(crate) fn from_request<T>(
        req: &PaymentsAuthorizeData<T>,
    ) -> Result<Self, error_stack::Report<IntegrationError>>
    where
        T: PaymentMethodDataTypes,
    {
        let raw = req.connector_feature_data.as_ref().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_feature_data.wallet_number",
                context: Default::default(),
            })
        })?;
        serde_json::from_value(raw.clone().expose()).map_err(|e| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_feature_data",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(format!(
                        "invalid Qwikcilver connector_feature_data: {e}"
                    )),
                    ..Default::default()
                },
            })
        })
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
                    context: Default::default(),
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
                context: Default::default(),
            })?;
        let idempotency_key = req.merchant_recharge_id.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "merchant_recharge_id",
                context: Default::default(),
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
        let payment_method_details = body.wallet.as_ref().map(|w| {
            PaymentMethodDetails::Wallet(WalletDetails {
                wallet_account_id: w.wallet_number.clone().expose(),
                wallet_pin: w.wallet_pin.clone(),
                wallet_status: None,
                wallet_holder_name: w
                    .wallet_holder_name
                    .as_ref()
                    .map(|h| h.clone().expose()),
                // Qwikcilver returns balance in major units; the domain type
                // is MinorUnit. We don't have the currency's exponent here,
                // so leave balance unset rather than risk a wrong conversion.
                balance: None,
                product_id: w.wallet_program_group_name.clone().unwrap_or_default(),
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

/// Numeric `TransactionId` header value. Qwikcilver requires a value that
/// fits in a signed 64-bit integer — values larger than `i64::MAX` come back
/// with ResponseCode=1 / "Value was either too large or too small for an Int64."
/// We mask to 62 bits to stay safely positive.
pub(crate) fn numeric_transaction_id() -> u64 {
    (uuid::Uuid::new_v4().as_u128() as u64) & 0x3FFF_FFFF_FFFF_FFFF
}

/// `DateAtClient` header value. Emits the `time` crate's
/// `Iso8601::DEFAULT` shape — UTC with a trailing `Z`, e.g.
/// `"2026-06-05T12:34:56.789012345Z"`. Verified accepted by Qwikcilver in
/// live UAE-sandbox testing; this is wider than the postman samples
/// (`YYYY-MM-DDTHH:MM:SS`) but the API tolerates the longer form.
pub(crate) fn current_datetime_qwikcilver() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Iso8601::DEFAULT)
        .unwrap_or_else(|_| "1970-01-01T00:00:00.000000000Z".to_string())
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

