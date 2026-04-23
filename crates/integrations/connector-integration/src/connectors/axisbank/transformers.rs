//! Axis Bank Transformer — Delegates to shared Juspay UPI Stack utilities

use crate::connectors::{
    axisbank::AxisbankRouterData,
    juspay_upi_stack::{
        transformers::{
            build_authorize_request, build_psync_request, build_refund_request,
            build_rsync_request, handle_authorize_response, handle_psync_response,
            handle_refund_response, handle_rsync_response,
        },
        types::{
            JuspayUpiAuthConfig as SharedAuthConfig, JwsObject, Refund360Response,
            RegisterIntentResponse, Status360Response,
        },
    },
};
use crate::types::ResponseRouterData;
use domain_types::{
    connector_flow::{Authorize, PSync, Refund, RSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundsData, RefundsResponseData, RefundSyncData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::Serialize;

// Re-export shared utilities for use in axisbank.rs
pub use crate::connectors::juspay_upi_stack::crypto::get_current_timestamp_ms;
pub use crate::connectors::juspay_upi_stack::transformers::build_error_response;
pub use crate::connectors::juspay_upi_stack::transformers::extract_merchant_identifiers_from_metadata;
pub use crate::connectors::juspay_upi_stack::transformers::map_outer_response_code;

/// Auth configuration for Axis Bank.
/// This struct extracts Axis-specific fields from ConnectorSpecificConfig.
#[derive(Debug, Clone)]
pub struct AxisbankAuthConfig {
    pub merchant_kid: String,
    pub juspay_kid: String,
    pub merchant_private_key: Secret<String>,
    pub juspay_public_key: Secret<String>,
    pub base_url: String,
}

impl TryFrom<&ConnectorSpecificConfig> for AxisbankAuthConfig {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Axisbank {
                merchant_kid,
                juspay_kid,
                merchant_private_key,
                juspay_public_key,
                base_url,
            } => Ok(Self {
                merchant_kid: merchant_kid.peek().clone(),
                juspay_kid: juspay_kid.peek().clone(),
                merchant_private_key: merchant_private_key.clone(),
                juspay_public_key: juspay_public_key.clone(),
                base_url: base_url.clone().unwrap_or_default(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some("Check connector_specific_config in merchant connector account configuration".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/register-intent".to_string()),
                    additional_context: Some("Expected Axisbank variant with fields: merchant_kid, juspay_kid, merchant_private_key, juspay_public_key".to_string()),
                },
            }
            .into()),
        }
    }
}

impl From<AxisbankAuthConfig> for SharedAuthConfig {
    fn from(config: AxisbankAuthConfig) -> Self {
        let jwe_kid = config.merchant_kid.clone();
        let merchant_private_key = config.merchant_private_key.clone();
        Self {
            merchant_kid: config.merchant_kid,
            juspay_kid: config.juspay_kid,
            merchant_private_key: config.merchant_private_key,
            juspay_public_key: config.juspay_public_key,
            use_jwe: true,  // Axis Bank uses JWE encryption for responses
            jwe_kid: Some(jwe_kid),
            juspay_jwe_public_key: None,
            merchant_jwe_private_key: Some(merchant_private_key),
        }
    }
}

/// Error response structure from Axis Bank API.
#[derive(Debug, serde::Deserialize)]
pub struct AxisbankErrorResponse {
    pub status: String,
    #[serde(rename = "responseCode")]
    pub response_code: String,
    #[serde(rename = "responseMessage")]
    pub response_message: String,
}

// ============================================================
// TYPE ALIASES — Point to shared types in juspay_upi_stack
// ============================================================

/// Authorize request body (Register Intent) — JWS object.
pub type AxisbankPaymentsRequest = JwsObject;
/// Authorize response — Register Intent response wrapper.
pub type AxisbankPaymentsResponse = RegisterIntentResponse;

/// PSync request body (Status 360) — JWS object.
pub type AxisbankSyncRequest = JwsObject;
/// PSync response — Status 360 response wrapper.
pub type AxisbankSyncResponse = Status360Response;

/// Refund request body (Refund 360) — JWS object.
pub type AxisbankRefundRequest = JwsObject;
/// Refund response — Refund 360 response wrapper.
pub type AxisbankRefundResponse = Refund360Response;

/// RSync request body (Refund Status 360) — JWS object.
pub type AxisbankRefundSyncRequest = JwsObject;
/// RSync response — Refund 360 response wrapper (same as Refund).
pub type AxisbankRefundSyncResponse = Refund360Response;

// ============================================================
// AUTHORIZE FLOW (Register Intent) — Delegate to shared
// ============================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AxisbankRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for AxisbankPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: AxisbankRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = AxisbankAuthConfig::try_from(&router_data.connector_config)?;
        let shared_auth: SharedAuthConfig = auth.into();

        // Convert amount from minor to major units
        let amount = wrapper
            .connector
            .amount_converter
            .convert(router_data.request.minor_amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some("Verify amount and currency values are valid".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/register-intent".to_string()),
                    additional_context: Some("Amount must be a positive integer in minor units (paise). Currency should be INR for UPI transactions.".to_string()),
                },
            })?;

        build_authorize_request(router_data, &shared_auth, amount.get_amount_as_string())
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static>
    TryFrom<
        ResponseRouterData<
            AxisbankPaymentsResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        resp: ResponseRouterData<
            AxisbankPaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        handle_authorize_response(resp.response, resp.http_code, resp.router_data)
    }
}

// ============================================================
// PSYNC FLOW (Status 360) — Delegate to shared
// ============================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AxisbankRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for AxisbankSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: AxisbankRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = AxisbankAuthConfig::try_from(&router_data.connector_config)?;
        let shared_auth: SharedAuthConfig = auth.into();

        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "connector_transaction_id",
                context: IntegrationErrorContext {
                    suggested_action: Some("Verify connector_transaction_id is provided in the PSync request".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/transaction-status-360".to_string()),
                    additional_context: Some("connector_transaction_id must be the original merchant_request_id from the Register Intent response. Used to query transaction status.".to_string()),
                },
            })?;

        build_psync_request(connector_transaction_id, &shared_auth)
    }
}

impl TryFrom<ResponseRouterData<AxisbankSyncResponse, RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        resp: ResponseRouterData<AxisbankSyncResponse, RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        handle_psync_response(resp.response, resp.http_code, resp.router_data)
    }
}

// ============================================================
// REFUND FLOW (Refund 360) — Delegate to shared
// ============================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AxisbankRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AxisbankRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: AxisbankRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = AxisbankAuthConfig::try_from(&router_data.connector_config)?;
        let shared_auth: SharedAuthConfig = auth.into();

        build_refund_request(&router_data.request, &shared_auth)
    }
}

impl
    TryFrom<
        ResponseRouterData<
            AxisbankRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    > for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        resp: ResponseRouterData<
            AxisbankRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        handle_refund_response(resp.response, resp.http_code, resp.router_data)
    }
}

// ============================================================
// RSYNC FLOW (Refund Status 360) — Delegate to shared
// ============================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AxisbankRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for AxisbankRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: AxisbankRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = AxisbankAuthConfig::try_from(&router_data.connector_config)?;
        let shared_auth: SharedAuthConfig = auth.into();

        let connector_transaction_id = router_data.request.connector_transaction_id.clone();
        let connector_refund_id = router_data.request.connector_refund_id.clone();

        build_rsync_request(connector_transaction_id, connector_refund_id, &shared_auth)
    }
}

impl
    TryFrom<
        ResponseRouterData<
            AxisbankRefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    > for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        resp: ResponseRouterData<
            AxisbankRefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        handle_rsync_response(resp.response, resp.http_code, resp.router_data)
    }
}
