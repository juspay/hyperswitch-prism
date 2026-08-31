use std::fmt::Debug;

use common_enums::{Currency, PayoutStatus};
use common_utils::{id_type, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{PayoutCreate, PayoutGet, PayoutStage, PayoutTransfer},
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutCreateRequest, PayoutCreateResponse, PayoutFlowData, PayoutGetRequest,
        PayoutGetResponse, PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest,
        PayoutTransferResponse,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils::missing_field_err,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::GigadatPayoutsRouterData;
use crate::types::ResponseRouterData;

/// Auth, error and transaction-type shapes for Gigadat payouts. Payouts are decoupled
/// from the payin connector, so these are defined here rather than imported from it.
#[derive(Debug, Clone)]
pub struct GigadatAuthType {
    pub campaign_id: Secret<String>,
    pub access_token: Secret<String>,
    pub security_token: Secret<String>,
    pub site: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GigadatAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Gigadat {
                campaign_id,
                access_token,
                security_token,
                site,
                ..
            } => Ok(Self {
                security_token: security_token.to_owned(),
                access_token: access_token.to_owned(),
                campaign_id: campaign_id.to_owned(),
                site: site.clone(),
            }),
            _ => Err(Report::new(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Gigadat payouts requires a Gigadat connector auth type".to_string(),
                    ),
                    suggested_action: Some(
                        "Configure the merchant connector account with Gigadat credentials"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })),
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct GigadatErrorResponse {
    pub err: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum GigadatTransactionType {
    Cpi,
    Eto,
}

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
            merchant_payout_id: router_data.request.merchant_payout_id.clone(),
            payout_status: PayoutStatus::from(response.status),
            connector_payout_id: Some(response.data.transaction_id),
            status_code: http_code,
        });
        Ok(router_data)
    }
}

// ===== PAYOUT SYNC RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatPayoutGetResponse {
    pub status: GigadatPayoutStatus,
}

impl TryFrom<ResponseRouterData<GigadatPayoutGetResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GigadatPayoutGetResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            mut router_data,
            http_code,
        } = item;

        router_data.response = Ok(PayoutGetResponse {
            merchant_payout_id: router_data.request.merchant_payout_id.clone(),
            payout_status: PayoutStatus::from(response.status),
            // Gigadat's sync response body carries only `status`, so echo back the id
            // the sync was issued against.
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
            merchant_payout_id: router_data.request.merchant_payout_id.clone(),
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
    pub user_ip: Secret<String, common_utils::pii::IpAddress>,
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
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Gigadat scopes each payout to a configured site".to_string(),
                    ),
                    suggested_action: Some(
                        "Set `site` on the Gigadat connector config".to_string(),
                    ),
                    doc_url: None,
                },
            })
        })?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.amount, request.destination_currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Gigadat expects the payout amount as a major-unit float".to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            })
            .attach_printable(
                "Failed to convert payout amount to Gigadat's major-unit float representation",
            )?;

        let customer = request.get_customer()?;
        let billing = request
            .address
            .as_ref()
            .and_then(|address| address.billing_address.as_ref())
            .ok_or_else(missing_field_err("address.billing_address"))?;
        let customer_id = customer.get_merchant_customer_id()?;
        let email = customer.get_email()?;
        let name = billing
            .get_optional_full_name()
            .ok_or_else(missing_field_err("address.billing_address.full_name"))?;
        let mobile = Secret::new(
            billing
                .get_phone_with_country_code()?
                .peek()
                .trim_start_matches('+')
                .to_string(),
        );
        let user_ip = request.get_browser_info()?.get_ip_address()?;

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

        router_data.response = Ok(PayoutStageResponse {
            merchant_payout_id: router_data.request.merchant_quote_id.clone(),
            payout_status: PayoutStatus::RequiresCreation,
            connector_payout_id: Some(response.data.transaction_id),
            status_code: http_code,
            connector_metadata: Some(Secret::new(connector_metadata)),
        });
        Ok(router_data)
    }
}
