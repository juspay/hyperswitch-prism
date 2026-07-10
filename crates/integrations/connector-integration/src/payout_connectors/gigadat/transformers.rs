use std::fmt::Debug;

use common_enums::{Currency, PayoutStatus};
use common_utils::{collect_missing_value_keys, id_type, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{PayoutCreate, PayoutGet, PayoutStage, PayoutTransfer},
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutCreateRequest, PayoutCreateResponse, PayoutFlowData, PayoutGetRequest,
        PayoutGetResponse, PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest,
        PayoutTransferResponse,
    },
    router_data_v2::RouterDataV2,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::GigadatPayoutsRouterData;
use crate::types::ResponseRouterData;

// Shared with the payments-side connector: same connector config and API conventions.
pub use crate::connectors::gigadat::transformers::{
    GigadatAuthType, GigadatErrorResponse, GigadatTransactionType,
};

// ===== PAYOUT RESPONSE TYPES =====
#[derive(Debug, Serialize, Deserialize)]
pub struct GigadatPayoutMeta {
    pub token: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GigadatPayoutData {
    pub transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GigadatPayoutStatus {
    StatusInited,
    StatusSuccess,
    StatusRejected,
    StatusRejected1,
    StatusExpired,
    StatusAborted1,
    StatusPending,
    StatusFailed,
}

impl From<GigadatPayoutStatus> for PayoutStatus {
    fn from(item: GigadatPayoutStatus) -> Self {
        match item {
            GigadatPayoutStatus::StatusSuccess => Self::Success,
            GigadatPayoutStatus::StatusPending => Self::RequiresFulfillment,
            GigadatPayoutStatus::StatusInited => Self::Pending,
            GigadatPayoutStatus::StatusRejected
            | GigadatPayoutStatus::StatusExpired
            | GigadatPayoutStatus::StatusRejected1
            | GigadatPayoutStatus::StatusAborted1
            | GigadatPayoutStatus::StatusFailed => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatPayoutTransferResponse {
    pub id: String,
    pub status: GigadatPayoutStatus,
    pub data: GigadatPayoutData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatPayoutCreateResponse {
    pub id: String,
    pub status: GigadatPayoutStatus,
    pub data: GigadatPayoutData,
}

// ===== RESPONSE TRANSFORMER (PAYOUT TRANSFER) =====
impl TryFrom<ResponseRouterData<GigadatPayoutTransferResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GigadatPayoutTransferResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: None,
                payout_status: PayoutStatus::from(response.status.clone()),
                connector_payout_id: Some(response.data.transaction_id.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// ===== PAYOUT SYNC RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatPayoutSyncResponse {
    pub status: GigadatPayoutStatus,
}

impl TryFrom<ResponseRouterData<GigadatPayoutSyncResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GigadatPayoutSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        Ok(Self {
            response: Ok(PayoutGetResponse {
                merchant_payout_id: None,
                payout_status: PayoutStatus::from(response.status.clone()),
                connector_payout_id: None,
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// ===== RESPONSE TRANSFORMER (PAYOUT CREATE) =====
impl TryFrom<ResponseRouterData<GigadatPayoutCreateResponse, Self>>
    for RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GigadatPayoutCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        Ok(Self {
            response: Ok(PayoutCreateResponse {
                merchant_payout_id: None,
                payout_status: PayoutStatus::from(response.status.clone()),
                connector_payout_id: Some(response.data.transaction_id.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// ===== PAYOUT STAGE REQUEST/RESPONSE =====
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GigadatPayoutStageRequest {
    pub amount: FloatMajorUnit,
    pub campaign: Secret<String>,
    pub currency: Currency,
    pub email: common_utils::pii::Email,
    pub mobile: Secret<String>,
    pub name: Secret<String>,
    pub site: String,
    pub transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: GigadatTransactionType,
    pub user_id: id_type::CustomerId,
    pub user_ip: Secret<String>,
    pub sandbox: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatPayoutStageResponse {
    pub token: Secret<String>,
    pub data: GigadatPayoutData,
}

// ===== REQUEST TRANSFORMER (PAYOUT STAGE) =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GigadatPayoutsRouterData<
            RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
            T,
        >,
    > for GigadatPayoutStageRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: GigadatPayoutsRouterData<
            RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = GigadatAuthType::try_from(&item.router_data.connector_config)?;

        let site = auth.site.ok_or_else(|| {
            Report::from(IntegrationError::InvalidConnectorConfig {
                config: "missing 'site' in connector config",
                context: Default::default(),
            })
        })?;

        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.amount,
                item.router_data.request.destination_currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let missing_fields = collect_missing_value_keys!(
            ("email", item.router_data.request.email.as_ref()),
            ("name", item.router_data.request.name.as_ref()),
            ("mobile", item.router_data.request.mobile.as_ref()),
            ("user_ip", item.router_data.request.user_ip.as_ref())
        );

        if !missing_fields.is_empty() {
            return Err(IntegrationError::MissingRequiredFields {
                field_names: missing_fields,
                context: Default::default(),
            }
            .into());
        }

        let email =
            item.router_data
                .request
                .email
                .clone()
                .ok_or(IntegrationError::InvariantViolation(
                    "email should be present after validation",
                ))?;
        let name =
            item.router_data
                .request
                .name
                .clone()
                .ok_or(IntegrationError::InvariantViolation(
                    "name should be present after validation",
                ))?;
        let mobile =
            item.router_data
                .request
                .mobile
                .clone()
                .ok_or(IntegrationError::InvariantViolation(
                    "mobile should be present after validation",
                ))?;
        let user_ip = item.router_data.request.user_ip.clone().ok_or(
            IntegrationError::InvariantViolation("user_ip should be present after validation"),
        )?;

        let customer_id = id_type::CustomerId::try_from(std::borrow::Cow::from(
            item.router_data
                .resource_common_data
                .merchant_id
                .get_string_repr()
                .to_owned(),
        ))
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "customer_id",
            context: Default::default(),
        })?;

        let sandbox = auth.test_mode.unwrap_or(true);

        Ok(Self {
            amount,
            campaign: auth.campaign_id,
            currency: item.router_data.request.destination_currency,
            email,
            mobile,
            name,
            site,
            transaction_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            transaction_type: GigadatTransactionType::Eto,
            user_id: customer_id,
            user_ip,
            sandbox,
        })
    }
}

// ===== RESPONSE TRANSFORMER (PAYOUT STAGE) =====
impl TryFrom<ResponseRouterData<GigadatPayoutStageResponse, Self>>
    for RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GigadatPayoutStageResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let connector_metadata = serde_json::json!({
            "token": response.token.peek().clone()
        });
        let connector_metadata_string = connector_metadata.to_string();

        Ok(Self {
            response: Ok(PayoutStageResponse {
                merchant_payout_id: None,
                payout_status: None,
                connector_payout_id: Some(response.data.transaction_id.clone()),
                status_code: item.http_code,
                connector_metadata: Some(connector_metadata_string.clone()),
            }),
            resource_common_data: PayoutFlowData {
                raw_connector_response: Some(Secret::new(connector_metadata_string)),
                ..router_data.resource_common_data.clone()
            },
            ..router_data.clone()
        })
    }
}
