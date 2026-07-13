use std::fmt::Debug;

use common_enums::{Currency, PayoutStatus};
use common_utils::{collect_missing_value_keys, id_type, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{PayoutCreate, PayoutGet, PayoutStage, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, ResponseTransformationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutCreateRequest, PayoutCreateResponse, PayoutFlowData, PayoutGetRequest,
        PayoutGetResponse, PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest,
        PayoutTransferResponse,
    },
    router_data_v2::RouterDataV2,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use super::GigadatPayoutsRouterData;
use crate::types::ResponseRouterData;

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
        let ResponseRouterData {
            response,
            mut router_data,
            http_code,
        } = item;

        router_data.response = Ok(PayoutTransferResponse {
            merchant_payout_id: None,
            payout_status: PayoutStatus::from(response.status),
            connector_payout_id: Some(response.data.transaction_id),
            status_code: http_code,
        });
        Ok(router_data)
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
        let ResponseRouterData {
            response,
            mut router_data,
            http_code,
        } = item;

        router_data.response = Ok(PayoutGetResponse {
            merchant_payout_id: None,
            payout_status: PayoutStatus::from(response.status),
            connector_payout_id: router_data.request.connector_payout_id.clone(),
            status_code: http_code,
        });
        Ok(router_data)
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
        let ResponseRouterData {
            response,
            mut router_data,
            http_code,
        } = item;

        router_data.response = Ok(PayoutCreateResponse {
            merchant_payout_id: None,
            payout_status: PayoutStatus::from(response.status),
            connector_payout_id: Some(response.data.transaction_id),
            status_code: http_code,
        });
        Ok(router_data)
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
        let request = &item.router_data.request;
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
            .convert(request.amount, request.destination_currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let missing_fields = collect_missing_value_keys!(
            ("customer.id", request.customer_id.as_ref()),
            ("email", request.email.as_ref()),
            ("name", request.name.as_ref()),
            ("mobile", request.mobile.as_ref()),
            ("user_ip", request.user_ip.as_ref())
        );

        let (Some(customer_id), Some(email), Some(name), Some(mobile), Some(user_ip)) = (
            request.customer_id.clone(),
            request.email.clone(),
            request.name.clone(),
            request.mobile.clone(),
            request.user_ip.clone(),
        ) else {
            return Err(IntegrationError::MissingRequiredFields {
                field_names: missing_fields,
                context: Default::default(),
            }
            .into());
        };

        let sandbox = item
            .router_data
            .resource_common_data
            .test_mode
            .unwrap_or(false);

        Ok(Self {
            amount,
            campaign: auth.campaign_id,
            currency: request.destination_currency,
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
        let ResponseRouterData {
            response,
            mut router_data,
            http_code,
        } = item;

        let connector_metadata = serde_json::to_string(&GigadatPayoutMeta {
            token: response.token.clone(),
        })
        .change_context(ConnectorError::ResponseHandlingFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(http_code),
                additional_context: Some(
                    "Failed to serialize Gigadat staged-payout token metadata".to_owned(),
                ),
            },
        })?;

        router_data.resource_common_data.raw_connector_response =
            Some(Secret::new(connector_metadata.clone()));
        router_data.response = Ok(PayoutStageResponse {
            merchant_payout_id: None,
            payout_status: None,
            connector_payout_id: Some(response.data.transaction_id),
            status_code: http_code,
            connector_metadata: Some(Secret::new(connector_metadata)),
        });
        Ok(router_data)
    }
}
