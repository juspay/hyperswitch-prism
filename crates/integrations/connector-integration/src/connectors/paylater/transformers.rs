use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{Authorize, PSync, Refund, ServerAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundsData, RefundsResponseData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PayLaterData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::PaylaterAmountConvertor;

/// PayLater auth mapping:
///   - `client_id`     = `ConnectorSpecificConfig::Paylater.api_key`
///   - `client_secret` = `ConnectorSpecificConfig::Paylater.key1`
///   - `outlet_id`     = issued to the merchant at onboarding (e.g. sandbox `1000000061`),
///                       sent as `outlet_id` in the Authorize (web-checkout) body.
/// These are exchanged for a short-lived Bearer JWT via the OAuth2
/// client_credentials token endpoint (`/auth/realms/api/protocol/openid-connect/token`).
/// The variant fields are positional names chosen by the framework (`api_key`, `key1`);
/// from PayLater's point of view they are simply an OAuth client_id/client_secret pair.
#[derive(Debug, Clone)]
pub struct PaylaterAuthType {
    /// OAuth `client_id`. Map from `ConnectorSpecificConfig::Paylater.api_key`.
    pub client_id: Secret<String>,
    /// OAuth `client_secret`. Map from `ConnectorSpecificConfig::Paylater.key1`.
    pub client_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PaylaterAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paylater { api_key, key1, .. } => Ok(Self {
                client_id: api_key.to_owned(),
                client_secret: key1.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

/// Resolve the merchant's `outlet_id` (issued by PayLater at onboarding).
///
/// The `ConnectorSpecificConfig::Paylater` variant carries only `api_key` / `key1`
/// (mapped to OAuth client_id / client_secret), so the per-merchant `outlet_id`
/// is supplied per-request via `PaymentsAuthorizeData::metadata`.
///
/// Accepted metadata shapes (first hit wins):
/// - `{"outlet_id": 1000000061}` or `{"paylater": {"outlet_id": 1000000061}}`
/// - `{"outlet_id": "1000000061"}` or `{"paylater": {"outlet_id": "1000000061"}}`
pub fn get_outlet_id_from_metadata(
    metadata: Option<&common_utils::pii::SecretSerdeValue>,
) -> Result<i64, error_stack::Report<errors::IntegrationError>> {
    let missing_or_invalid = || {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "metadata.outlet_id",
            context: Default::default(),
        })
    };

    let metadata = metadata.ok_or_else(missing_or_invalid)?;
    let value = metadata.peek();

    let from_json = |v: &serde_json::Value| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
    };

    match value {
        serde_json::Value::Object(map) => map
            .get("outlet_id")
            .or_else(|| map.get("paylater").and_then(|p| p.get("outlet_id")))
            .and_then(from_json)
            .ok_or_else(missing_or_invalid),
        _ => Err(missing_or_invalid()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaylaterErrorResponse {
    /// The reference docs ship the error envelope as `{"error": "...", "message": "..."}`,
    /// while the OAuth client_credentials token endpoint emits
    /// `{"error": "...", "error_description": "..."}`. Accept either spelling.
    #[serde(alias = "error")]
    pub code: Option<String>,
    pub message: Option<String>,
    /// OAuth token endpoint's human-readable detail field.
    pub error_description: Option<String>,
}

// ===== SERVER_AUTHENTICATION_TOKEN (OAuth2 client_credentials) =====

#[derive(Debug, Serialize)]
pub struct PaylaterAuthorizeRequest {
    pub grant_type: String,
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PaylaterRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for PaylaterAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::PaylaterRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PaylaterAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            grant_type: "client_credentials".to_string(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaylaterAuthorizeResponse {
    pub access_token: Secret<String>,
    pub expires_in: i64,
    pub token_type: String,
    #[serde(default)]
    pub refresh_token: Option<Secret<String>>,
    #[serde(default)]
    pub refresh_expires_in: Option<i64>,
    #[serde(default)]
    pub scope: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<PaylaterAuthorizeResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaylaterAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        data.response = Ok(ServerAuthenticationTokenResponseData {
            access_token: body.access_token,
            token_type: Some(body.token_type),
            expires_in: Some(body.expires_in),
        });
        Ok(data)
    }
}

// ===== AUTHORIZE — Generate Payment Link (hosted web checkout) =====
// POST /api/paylater/merchant-portal/v2/web-checkout
// On success the gateway returns a `paymentLinkUrl` — the shopper is redirected
// there to complete the BNPL payment on PayLater's hosted page.

/// Default payment-link validity in minutes (API accepts 1–1440).
const PAYLATER_DEFAULT_EXPIRY_DURATION_MINUTES: i64 = 60;

#[derive(Debug, Serialize)]
pub struct PaylaterWebCheckoutRequest {
    pub outlet_id: i64,
    pub currency: String,
    /// Base-unit amount as JSON double (e.g. 500.00 QAR). Do NOT scale to minor units.
    pub amount: FloatMajorUnit,
    pub order_id: String,
    pub success_redirect_url: String,
    pub fail_redirect_url: String,
    pub expiry_duration: i64,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PaylaterRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaylaterWebCheckoutRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::PaylaterRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Only PayLaterRedirect BNPL payment method is supported by this connector.
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::PayLater(PayLaterData::PayLaterRedirect {}) => Ok(()),
            _ => Err(error_stack::report!(
                errors::IntegrationError::NotSupported {
                    message: "PayLater only supports PayLaterRedirect BNPL payments".to_string(),
                    connector: "paylater",
                    context: Default::default(),
                }
            )),
        }?;

        // PayLater is QAR-only; reject any other currency before hitting the gateway.
        if item.router_data.request.currency != Currency::QAR {
            return Err(error_stack::report!(
                errors::IntegrationError::CurrencyNotSupported {
                    message: format!(
                        "PayLater supports only QAR currency, got {}",
                        item.router_data.request.currency
                    ),
                    connector: "paylater",
                    context: Default::default(),
                }
            ));
        }

        let amount = PaylaterAmountConvertor::convert(
            item.router_data.request.minor_amount,
            item.router_data.request.currency,
        )
        .change_context(errors::IntegrationError::AmountConversionFailed {
            context: Default::default(),
        })?;

        let return_url = item
            .router_data
            .request
            .get_router_return_url()
            .change_context(errors::IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            })?;

        // `outlet_id` is issued per-merchant at PayLater onboarding. Since the
        // `ConnectorSpecificConfig::Paylater` variant only carries the OAuth
        // client_id/client_secret, the outlet_id arrives per-request via metadata.
        let outlet_id = get_outlet_id_from_metadata(item.router_data.request.metadata.as_ref())?;

        Ok(Self {
            outlet_id,
            currency: item.router_data.request.currency.to_string(),
            amount,
            order_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            // The hosted checkout sends the shopper back to the merchant's return
            // URL on both success and failure.
            success_redirect_url: return_url.clone(),
            fail_redirect_url: return_url,
            expiry_duration: PAYLATER_DEFAULT_EXPIRY_DURATION_MINUTES,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaylaterWebCheckoutResponse {
    #[serde(rename = "paymentLinkUrl")]
    pub payment_link_url: String,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        ResponseRouterData<
            PaylaterWebCheckoutResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            PaylaterWebCheckoutResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let order_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // No gateway-side transaction id exists yet — it appears as
                // `payLaterOrderId` only after the shopper initiates on the hosted page.
                resource_id: ResponseId::NoResponseId,
                redirection_data: Some(Box::new(RedirectForm::Uri {
                    uri: item.response.payment_link_url,
                })),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(order_id),
                incremental_authorization_allowed: None,
                network_txn_link_id: None,
                splits: None,
                payment_account_reference: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                // Shopper must visit the payment link to complete the payment.
                status: AttemptStatus::AuthenticationPending,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// Map PayLater's integer status to UCS `AttemptStatus`.
///
/// | PayLater `status` | `message`             | `AttemptStatus`         |
/// |-------------------|-----------------------|-------------------------|
/// | 0                 | `Order not initiated` | `AuthenticationPending` |
/// | 1                 | `pending`             | `Pending`               |
/// | 2                 | `success`             | `Charged`               |
/// | 3                 | `failed`              | `Failure`               |
/// | anything else     | —                     | `Pending` (conservative)|
pub(crate) fn map_paylater_status_to_attempt_status(status: i64) -> AttemptStatus {
    match status {
        0 => AttemptStatus::AuthenticationPending,
        1 => AttemptStatus::Pending,
        2 => AttemptStatus::Charged,
        3 => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    }
}

// ===== PSYNC — Check Payment Status =====
// `GET /api/paylater/merchant-portal/v2/web-checkout/status?order_id=<order_id>`
//
// No request body — the connector macro is invoked without `curl_request` and
// the HTTP method is `Get`. Bearer auth is supplied via the access_token that
// the ServerAuthenticationToken flow previously cached on
// `resource_common_data.access_token`.
//
// `order_id` is the merchant's reference (not the gateway's `payLaterOrderId`),
// so it is read from `resource_common_data.connector_request_reference_id` —
// the same value originally sent as `order_id` in the Authorize body.

#[derive(Debug, Deserialize, Serialize)]
pub struct PaylaterPSyncResponse {
    /// 0 = Order not initiated, 1 = Pending, 2 = Success, 3 = Failed.
    pub status: i64,
    /// Human hint: "Order not initiated" | "pending" | "success" | "failed".
    /// Retained for debugging; status mapping uses the integer code.
    pub message: String,
    /// PayLater's own transaction reference. Present only after the shopper
    /// has initiated on the hosted page.
    #[serde(default, rename = "payLaterOrderId")]
    pub pay_later_order_id: Option<String>,
    /// Echoes the merchant's `order_id`. Present only after initiation.
    #[serde(default, rename = "merchantReference")]
    pub merchant_reference: Option<String>,
}

impl TryFrom<
        ResponseRouterData<
            PaylaterPSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    > for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            PaylaterPSyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let status = map_paylater_status_to_attempt_status(item.response.status);

        // `payLaterOrderId` is PayLater's own reference — surface it as the
        // UCS `connector_transaction_id`. It is absent pre-initiation (the
        // shopper hasn't clicked the link yet); fall back to NoResponseId so
        // we never fabricate an id the gateway won't recognise later.
        let resource_id = match item.response.pay_later_order_id.clone() {
            Some(id) => ResponseId::ConnectorTransactionId(id),
            None => ResponseId::NoResponseId,
        };

        // `merchantReference` echoes our `order_id`; when absent (pre-initiation)
        // fall back to the `connector_request_reference_id` we sent so the UCS
        // consumer always has a stable cross-reference.
        let connector_response_reference_id =
            item.response.merchant_reference.clone().or_else(|| {
                Some(
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                )
            });

        let mut router_data = item.router_data;
        router_data.resource_common_data.raw_connector_response =
            serde_json::to_string(&item.response).ok().map(Secret::new);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id,
                incremental_authorization_allowed: None,
                network_txn_link_id: None,
                splits: None,
                payment_account_reference: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ===== REFUND =====
//
// `POST /api/paylater/merchant-portal/v2/web-checkout/refund`
//
// Refund is a **full-amount**, synchronous-accepted refund keyed by the merchant
// `order_id`. Per the PayLater merchant-portal spec the request body carries
// only the `order_id` — there is no amount/currency/reason field — so the
// gateway always refunds the full captured amount.
//
// **Full-refund semantics.** If UCS relays a partial refund (`minor_refund_amount
// < minor_payment_amount`), we still forward the request — PayLater treats every
// call as a full refund on the `order_id`. We do not surface this as a hard
// failure here because rejecting it locally would diverge from the gateway's
// actual behaviour. Merchants needing partial refunds must use the gateway's
// out-of-band channels.
//
// **29-day / 10-minute windows.** The gateway rejects refunds where the original
// transaction is either older than 29 days or younger than 10 minutes, returning
// an error envelope such as:
//   * `"Order cannot be refunded as it happened more than 29 days ago."`
//   * `"Transaction happened less than 10 minutes ago. Please try again later."`
// We cannot pre-validate these locally because `RefundsData` carries no original
// payment timestamp. We rely on the gateway to enforce these rules; the verbatim
// error is surfaced through `PaylaterErrorResponse`.
//
// **idempotency.** Refund is keyed by `order_id` only (not by `refund_id`), so
// it is NOT idempotent at the gateway level. UCS-level idempotency must be
// enforced upstream by deduplicating on `refund_id`.
#[derive(Debug, Serialize)]
pub struct PaylaterRefundRequest {
    pub order_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::PaylaterRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PaylaterRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::PaylaterRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // PayLater Authorize never populates `connector_transaction_id` (the
        // gateway only emits `payLaterOrderId` AFTER the shopper initiates on the
        // hosted page), so the merchant-facing order id is the value UCS
        // originally sent as `order_id` — stored on
        // `resource_common_data.connector_request_reference_id`. Prefer it over
        // `RefundsData.connector_transaction_id` (which may be the gateway-side
        // `payLaterOrderId` if the payment has progressed past initiation).
        let order_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        if order_id.is_empty() {
            return Err(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "connector_request_reference_id (order_id)",
                    context: Default::default(),
                }
            ));
        }

        Ok(Self { order_id })
    }
}

/// The refund response has no structured `refund_id` or `status` — only a
/// human-readable message like `"Refund request accepted for reference Id: <id>"`.
#[derive(Debug, Deserialize, Serialize)]
pub struct PaylaterRefundResponse {
    pub message: String,
}

impl TryFrom<
        ResponseRouterData<
            PaylaterRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    > for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            PaylaterRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        // The gateway does not return its own refund reference; echo back the
        // UCS refund_id so RSync-by-us has a stable key if ever invoked. Status
        // is `Pending` rather than `Success` because the gateway marks the refund
        // as "accepted", not committed — there is no follow-up RSync polling
        // endpoint to confirm finality.
        let connector_refund_id = item.router_data.request.refund_id.clone();

        let mut router_data = item.router_data;
        router_data.resource_common_data.raw_connector_response =
            serde_json::to_string(&item.response).ok().map(Secret::new);
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id,
            refund_status: RefundStatus::Pending,
            status_code: item.http_code,
            acquirer_reference_number: None,
        });
        Ok(router_data)
    }
}
