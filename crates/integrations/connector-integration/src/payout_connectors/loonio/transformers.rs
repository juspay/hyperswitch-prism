use crate::types::ResponseRouterData;
use common_enums::PayoutStatus;
use common_utils::{id_type::CustomerId, pii::Email, types::FloatMajorUnitForConnector};
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError},
    payouts::{
        payout_method_data::{BankRedirect, Interac, PayoutMethodData},
        payouts_types::{
            PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
            PayoutTransferResponse,
        },
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

// ===== AUTH TYPE =====

#[derive(Debug, Clone)]
pub struct LoonioAuthType {
    pub merchant_id: Secret<String>,
    pub merchant_token: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for LoonioAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Loonio {
                merchant_id,
                merchant_token,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                merchant_token: merchant_token.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoonioErrorResponse {
    pub status: Option<u16>,
    pub error_code: Option<String>,
    pub message: String,
}

// ===== CUSTOMER PROFILE (used in payout transfer request) =====

#[derive(Debug, Serialize)]
pub struct LoonioCustomerProfile {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub email: Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_a: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub province: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

// ===== PAYOUT STATUS =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoonioPayoutStatus {
    Created,
    Prepared,
    Pending,
    Settled,
    Available,
    Rejected,
    Abandoned,
    ConnectedAbandoned,
    ConnectedInsufficientFunds,
    Failed,
    Nsf,
    Returned,
    Rollback,
}

impl From<LoonioPayoutStatus> for PayoutStatus {
    fn from(item: LoonioPayoutStatus) -> Self {
        match item {
            LoonioPayoutStatus::Created | LoonioPayoutStatus::Prepared => Self::Initiated,
            LoonioPayoutStatus::Pending => Self::Pending,
            LoonioPayoutStatus::Settled | LoonioPayoutStatus::Available => Self::Success,
            LoonioPayoutStatus::Rejected
            | LoonioPayoutStatus::Abandoned
            | LoonioPayoutStatus::ConnectedAbandoned
            | LoonioPayoutStatus::ConnectedInsufficientFunds
            | LoonioPayoutStatus::Failed
            | LoonioPayoutStatus::Nsf
            | LoonioPayoutStatus::Returned
            | LoonioPayoutStatus::Rollback => Self::Failure,
        }
    }
}

// ===== PAYOUT GET FLOW =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoonioPayoutGetResponse {
    pub transaction_id: String,
    pub state: LoonioPayoutStatus,
}

impl TryFrom<ResponseRouterData<LoonioPayoutGetResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<LoonioPayoutGetResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutGetResponse {
                merchant_payout_id: None,
                payout_status: PayoutStatus::from(item.response.state),
                connector_payout_id: Some(item.response.transaction_id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== PAYOUT TRANSFER FLOW =====

#[derive(Debug, Serialize)]
pub struct LoonioPayoutTransferRequest {
    pub currency_code: common_enums::Currency,
    pub customer_profile: LoonioCustomerProfile,
    pub amount: common_utils::types::FloatMajorUnit,
    pub customer_id: CustomerId,
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for LoonioPayoutTransferRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> Result<Self, Self::Error> {
        match req.request.payout_method_data.clone() {
            Some(PayoutMethodData::BankRedirect(BankRedirect::Interac(Interac { email }))) => {
                let transaction_id = req
                    .resource_common_data
                    .connector_request_reference_id
                    .clone();

                let customer_profile = LoonioCustomerProfile {
                    first_name: req.request.get_billing_first_name()?,
                    last_name: req.request.get_billing_last_name()?,
                    email,
                    phone: req.request.get_optional_billing_phone(),
                    address_a: req.request.get_optional_billing_line1(),
                    city: req.request.get_optional_billing_city(),
                    province: req.request.get_optional_billing_state(),
                    postal_code: req.request.get_optional_billing_zip(),
                    country: req
                        .request
                        .get_optional_billing_country()
                        .map(|c| c.to_string()),
                };

                let amount = utils::convert_amount(
                    &FloatMajorUnitForConnector,
                    req.request.amount,
                    req.request.source_currency,
                )?;

                let customer_id = req.request.get_customer_id()?;

                Ok(Self {
                    currency_code: req.request.source_currency,
                    customer_profile,
                    amount,
                    customer_id,
                    transaction_id,
                    webhook_url: req.request.webhook_url.clone(),
                })
            }
            Some(PayoutMethodData::Card(_))
            | Some(PayoutMethodData::Bank(_))
            | Some(PayoutMethodData::Wallet(_))
            | Some(PayoutMethodData::BankRedirect(_))
            | Some(PayoutMethodData::Passthrough(_))
            | None => Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Payment Method Not Supported".to_string(),
                connector: "Loonio",
                context: Default::default(),
            })),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoonioPayoutTransferResponse {
    pub id: i64,
    pub api_transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub state: LoonioPayoutStatus,
}

impl TryFrom<ResponseRouterData<LoonioPayoutTransferResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<LoonioPayoutTransferResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: None,
                payout_status: PayoutStatus::from(item.response.state),
                connector_payout_id: Some(item.response.api_transaction_id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
