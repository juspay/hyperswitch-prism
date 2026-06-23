use std::fmt::Debug;

use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::types::MinorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, PayLaterData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::affirm::AffirmRouterData, types::ResponseRouterData};

// ===== AUTH =====

#[derive(Debug, Clone)]
pub struct AffirmAuthType {
    pub public_key: Secret<String>,
    pub private_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AffirmAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Affirm {
                public_key,
                private_key,
                ..
            } => Ok(Self {
                public_key: public_key.to_owned(),
                private_key: private_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Expected Affirm public_key and private_key".to_string()
                        ),
                        ..Default::default()
                    }
                }
            )),
        }
    }
}

// ===== ERROR =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffirmErrorResponse {
    pub status_code: Option<serde_json::Value>,
    pub message: Option<String>,
    pub code: Option<String>,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub field: Option<String>,
}

// ===== STATUS =====

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AffirmTransactionStatus {
    Authorized,
    Captured,
    Voided,
    Refunded,
    Declined,
    #[serde(other)]
    Unknown,
}

impl From<AffirmTransactionStatus> for AttemptStatus {
    fn from(status: AffirmTransactionStatus) -> Self {
        match status {
            AffirmTransactionStatus::Authorized => Self::Authorized,
            AffirmTransactionStatus::Captured => Self::Charged,
            AffirmTransactionStatus::Voided => Self::Voided,
            AffirmTransactionStatus::Refunded => Self::Charged,
            AffirmTransactionStatus::Declined => Self::Failure,
            AffirmTransactionStatus::Unknown => Self::Pending,
        }
    }
}

// ===== AUTHORIZE =====
// POST /api/v1/transactions  body: { transaction_id: checkout_token, order_id, currency, total }

#[derive(Debug, Serialize)]
pub struct AffirmPaymentsRequest {
    pub transaction_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<MinorUnit>,
}

/// Extract the Affirm `checkout_token` produced by the hosted-checkout redirect.
/// On return to the merchant, the token arrives either in
/// `redirect_response.params` (single value) or in the `redirect_response.payload`
/// object under the `checkout_token` key.
fn extract_checkout_token<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<Secret<String>, error_stack::Report<errors::IntegrationError>> {
    // Only PayLater (Affirm BNPL redirect) is supported.
    match &router_data.request.payment_method_data {
        PaymentMethodData::PayLater(PayLaterData::AffirmRedirect {}) => {}
        _ => {
            return Err(error_stack::report!(
                errors::IntegrationError::NotImplemented(
                    "Affirm only supports the PayLater (Affirm) payment method".to_string(),
                    Default::default(),
                )
            ))
        }
    }

    if let Some(redirect) = router_data.request.redirect_response.as_ref() {
        if let Some(payload) = redirect.payload.as_ref() {
            if let Some(token) = payload
                .clone()
                .expose()
                .as_object()
                .and_then(|map| map.get("checkout_token"))
                .and_then(|val| val.as_str())
            {
                return Ok(Secret::new(token.to_string()));
            }
        }
        if let Some(params) = redirect.params.as_ref() {
            let raw = params.clone().expose();
            if !raw.is_empty() {
                return Ok(Secret::new(raw));
            }
        }
    }

    Err(error_stack::report!(
        errors::IntegrationError::MissingRequiredField {
            field_name: "checkout_token",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "Affirm checkout_token must be supplied via the redirect response \
                     (redirect_response.params or redirect_response.payload.checkout_token)"
                        .to_string(),
                ),
                ..Default::default()
            }
        }
    ))
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AffirmRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for AffirmPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: AffirmRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let transaction_id = extract_checkout_token(router_data)?;
        let order_id = Some(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );
        Ok(Self {
            transaction_id,
            order_id,
            currency: Some(router_data.request.currency),
            total: Some(router_data.request.minor_amount),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub amount: Option<MinorUnit>,
    pub currency: Option<String>,
    pub created: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmPaymentsResponse {
    pub id: String,
    pub status: AffirmTransactionStatus,
    pub amount: Option<MinorUnit>,
    pub amount_refunded: Option<MinorUnit>,
    pub currency: Option<String>,
    pub order_id: Option<String>,
    pub checkout_id: Option<String>,
    pub events: Option<Vec<AffirmEvent>>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AffirmPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AffirmPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());
        let transaction_id = item.response.id.clone();

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_order_id: Some(transaction_id.clone()),
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== PSYNC =====
// GET /api/v1/transactions/{id}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmSyncResponse {
    pub id: String,
    pub status: AffirmTransactionStatus,
    pub order_id: Option<String>,
    pub amount_refunded: Option<MinorUnit>,
    pub events: Option<Vec<AffirmEvent>>,
}

impl TryFrom<ResponseRouterData<AffirmSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<AffirmSyncResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== CAPTURE =====
// POST /api/v1/transactions/{id}/capture  -> returns capture event { id, type, amount }

#[derive(Debug, Serialize)]
pub struct AffirmCaptureRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AffirmRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AffirmCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: AffirmRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            order_id: item
                .router_data
                .resource_common_data
                .reference_id
                .clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmCaptureResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub amount: Option<MinorUnit>,
    pub order_id: Option<String>,
}

impl TryFrom<ResponseRouterData<AffirmCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AffirmCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // A successful (HTTP 2xx) capture event settles the funds.
        let connector_transaction_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== VOID =====
// POST /api/v1/transactions/{id}/void -> returns void event { id, type, amount }

#[derive(Debug, Serialize)]
pub struct AffirmVoidRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AffirmRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AffirmVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: AffirmRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            reference_id: item
                .router_data
                .resource_common_data
                .reference_id
                .clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmVoidResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub amount: Option<MinorUnit>,
}

impl TryFrom<ResponseRouterData<AffirmVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<AffirmVoidResponse, Self>) -> Result<Self, Self::Error> {
        let connector_transaction_id = item.router_data.request.connector_transaction_id.clone();

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== REFUND =====
// POST /api/v1/transactions/{id}/refund -> returns refund event { id, type, amount }

#[derive(Debug, Serialize)]
pub struct AffirmRefundRequest {
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AffirmRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for AffirmRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: AffirmRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.minor_refund_amount,
            reference_id: Some(item.router_data.request.refund_id.clone()),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmRefundResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub amount: Option<MinorUnit>,
}

impl TryFrom<ResponseRouterData<AffirmRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<AffirmRefundResponse, Self>) -> Result<Self, Self::Error> {
        // A successful (HTTP 2xx) refund event indicates the refund was accepted.
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: RefundStatus::Success,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== RSYNC =====
// GET /api/v1/transactions/{id}?expand=events
// Inspect amount_refunded / events[type=refund] to derive refund completion.

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmRSyncResponse {
    pub id: String,
    pub status: AffirmTransactionStatus,
    pub amount_refunded: Option<MinorUnit>,
    pub events: Option<Vec<AffirmEvent>>,
}

impl TryFrom<ResponseRouterData<AffirmRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<AffirmRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let connector_refund_id = item.router_data.request.connector_refund_id.clone();

        // The refund is complete when a matching refund event is present in the
        // transaction's events array, or the transaction is marked refunded.
        let refund_event_present = item
            .response
            .events
            .as_ref()
            .map(|events| events.iter().any(|event| event.event_type == "refund"))
            .unwrap_or(false);

        let refund_status = if refund_event_present
            || item.response.status == AffirmTransactionStatus::Refunded
        {
            RefundStatus::Success
        } else {
            RefundStatus::Pending
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}
