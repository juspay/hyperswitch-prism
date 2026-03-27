use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{id_type, request::Method, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::ConnectorError,
    payment_method_data::{BankRedirectData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::gigadat::GigadatRouterData, types::ResponseRouterData};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

// ===== CONNECTOR METADATA =====
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GigadatConnectorMetadataObject {
    pub site: String,
}

impl TryFrom<&Option<common_utils::pii::SecretSerdeValue>> for GigadatConnectorMetadataObject {
    type Error = Report<ConnectorError>;

    fn try_from(
        meta_data: &Option<common_utils::pii::SecretSerdeValue>,
    ) -> Result<Self, Self::Error> {
        let metadata: Self = meta_data
            .clone()
            .map(|data| {
                serde_json::from_value::<Self>(data.expose()).change_context(
                    ConnectorError::InvalidConnectorConfig {
                        config: "merchant_connector_account.metadata",
                    },
                )
            })
            .transpose()?
            .unwrap_or_default();
        Ok(metadata)
    }
}

// ===== AUTHENTICATION =====
#[derive(Debug, Clone)]
pub struct GigadatAuthType {
    pub campaign_id: Secret<String>,
    pub access_token: Secret<String>,
    pub security_token: Secret<String>,
    pub site: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GigadatAuthType {
    type Error = Report<ConnectorError>;

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
            _ => Err(Report::new(ConnectorError::FailedToObtainAuthType)),
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct GigadatErrorResponse {
    pub err: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct GigadatRefundErrorResponse {
    pub error: Vec<GigadatRefundError>,
    pub message: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct GigadatRefundError {
    pub code: Option<String>,
    pub detail: String,
}

// ===== TRANSACTION TYPE =====
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum GigadatTransactionType {
    Cpi,
    Eto,
}

// ===== TRANSACTION STATUS =====
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GigadatTransactionStatus {
    StatusInited,
    StatusSuccess,
    StatusRejected,
    StatusRejected1,
    StatusExpired,
    StatusAborted1,
    StatusPending,
    StatusFailed,
}

impl TryFrom<String> for GigadatTransactionStatus {
    type Error = Report<ConnectorError>;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "STATUS_INITED" => Ok(Self::StatusInited),
            "STATUS_SUCCESS" => Ok(Self::StatusSuccess),
            "STATUS_REJECTED" => Ok(Self::StatusRejected),
            "STATUS_REJECTED1" => Ok(Self::StatusRejected1),
            "STATUS_EXPIRED" => Ok(Self::StatusExpired),
            "STATUS_ABORTED1" => Ok(Self::StatusAborted1),
            "STATUS_PENDING" => Ok(Self::StatusPending),
            "STATUS_FAILED" => Ok(Self::StatusFailed),
            _ => Err(ConnectorError::WebhookBodyDecodingFailed.into()),
        }
    }
}

impl From<GigadatTransactionStatus> for AttemptStatus {
    fn from(item: GigadatTransactionStatus) -> Self {
        match item {
            GigadatTransactionStatus::StatusSuccess => Self::Charged,
            GigadatTransactionStatus::StatusInited | GigadatTransactionStatus::StatusPending => {
                Self::Pending
            }
            GigadatTransactionStatus::StatusRejected
            | GigadatTransactionStatus::StatusExpired
            | GigadatTransactionStatus::StatusRejected1
            | GigadatTransactionStatus::StatusAborted1
            | GigadatTransactionStatus::StatusFailed => Self::Failure,
        }
    }
}

// ===== PAYMENT REQUEST (CPI) =====
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GigadatPaymentsRequest {
    pub user_id: id_type::CustomerId,
    pub site: String,
    pub user_ip: Secret<String>,
    pub currency: Currency,
    pub amount: FloatMajorUnit,
    pub transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: GigadatTransactionType,
    pub sandbox: bool,
    pub name: Secret<String>,
    pub email: common_utils::pii::Email,
    pub mobile: Secret<String>,
}

// ===== PAYMENT RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatPaymentsResponse {
    pub token: Secret<String>,
    pub data: GigadatPaymentData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GigadatPaymentData {
    pub transaction_id: String,
}

// ===== SYNC RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatSyncResponse {
    pub status: GigadatTransactionStatus,
    pub interac_bank_name: Option<Secret<String>>,
    pub data: Option<GigadatSyncData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatSyncData {
    pub name: Option<Secret<String>>,
    pub email: Option<common_utils::pii::Email>,
    pub mobile: Option<Secret<String>>,
}

// ===== REFUND REQUEST =====
#[derive(Default, Debug, Serialize)]
pub struct GigadatRefundRequest {
    pub amount: FloatMajorUnit,
    pub transaction_id: String,
    pub campaign_id: Secret<String>,
}

// ===== REFUND RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigadatRefundResponse {
    pub success: bool,
    pub data: GigadatPaymentData,
}

// ===== REQUEST TRANSFORMER (AUTHORIZE) =====
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GigadatRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GigadatPaymentsRequest
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: GigadatRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Get site from connector config, then fallback to request metadata
        let auth = GigadatAuthType::try_from(&item.router_data.connector_config)?;
        let metadata = match auth.site {
            Some(site) => GigadatConnectorMetadataObject { site },
            None => {
                // Try to get site from request metadata
                item.router_data
                    .request
                    .metadata
                    .as_ref()
                    .and_then(|m| m.peek().get("site"))
                    .and_then(|v| v.as_str())
                    .map(|site| GigadatConnectorMetadataObject {
                        site: site.to_string(),
                    })
                    .ok_or_else(|| {
                        Report::from(ConnectorError::InvalidConnectorConfig {
                            config: "missing 'site' in connector config or metadata",
                        })
                    })?
            }
        };

        // Validate payment method is Interac bank redirect
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(BankRedirectData::Interac { .. }) => {
                // Get billing details
                let billing = item
                    .router_data
                    .resource_common_data
                    .address
                    .get_payment_billing()
                    .ok_or(ConnectorError::MissingRequiredField {
                        field_name: "billing_address",
                    })?;

                let billing_address =
                    billing
                        .clone()
                        .address
                        .ok_or(ConnectorError::MissingRequiredField {
                            field_name: "billing_address.address",
                        })?;

                let name = billing_address.get_optional_full_name().ok_or(
                    ConnectorError::MissingRequiredField {
                        field_name: "billing_address.first_name or billing_address.last_name",
                    },
                )?;

                let email = billing
                    .email
                    .clone()
                    .or(item.router_data.request.email.clone())
                    .ok_or(ConnectorError::MissingRequiredField {
                        field_name: "billing_address.email or email",
                    })?;

                let mobile = billing.get_phone_with_country_code().map_err(|_| {
                    ConnectorError::MissingRequiredField {
                        field_name: "billing_address.phone",
                    }
                })?;

                // Get customer ID
                let customer_id = item
                    .router_data
                    .resource_common_data
                    .customer_id
                    .clone()
                    .ok_or(ConnectorError::MissingRequiredField {
                        field_name: "customer_id",
                    })?;

                // Get browser IP
                let browser_info = item.router_data.request.browser_info.clone().ok_or(
                    ConnectorError::MissingRequiredField {
                        field_name: "browser_info",
                    },
                )?;
                let user_ip = Secret::new(
                    browser_info
                        .ip_address
                        .ok_or(ConnectorError::MissingRequiredField {
                            field_name: "browser_info.ip_address",
                        })?
                        .to_string(),
                );

                // Determine sandbox mode
                let sandbox = item
                    .router_data
                    .resource_common_data
                    .test_mode
                    .unwrap_or(false);

                let amount = item
                    .connector
                    .amount_converter
                    .convert(
                        item.router_data.request.minor_amount,
                        item.router_data.request.currency,
                    )
                    .change_context(ConnectorError::AmountConversionFailed)?;

                Ok(Self {
                    user_id: customer_id,
                    site: metadata.site,
                    user_ip,
                    currency: item.router_data.request.currency,
                    amount,
                    transaction_id: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    transaction_type: GigadatTransactionType::Cpi,
                    sandbox,
                    name,
                    email,
                    mobile,
                })
            }
            PaymentMethodData::BankRedirect(_) => Err(Report::new(ConnectorError::NotImplemented(
                "Only Interac bank redirect is supported for Gigadat".to_string(),
            ))),
            _ => Err(Report::new(ConnectorError::NotImplemented(
                "Only Interac bank redirect is supported for Gigadat".to_string(),
            ))),
        }
    }
}

// ===== RESPONSE TRANSFORMER (AUTHORIZE) =====
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<GigadatPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GigadatPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Build redirect URL
        let redirect_url = format!(
            "{}webflow?transaction={}&token={}",
            router_data.resource_common_data.connectors.gigadat.base_url,
            router_data
                .resource_common_data
                .connector_request_reference_id,
            response.token.peek()
        );

        let redirection_data = Some(Box::new(RedirectForm::Form {
            endpoint: redirect_url,
            method: Method::Get,
            form_fields: std::collections::HashMap::new(),
        }));

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    response.data.transaction_id.clone(),
                ),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// ===== RESPONSE TRANSFORMER (PSYNC) =====
impl TryFrom<ResponseRouterData<GigadatSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<GigadatSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let status = AttemptStatus::from(response.status.clone());

        // Build customer metadata if data is present
        let connector_metadata = response.data.as_ref().map(|data| {
            serde_json::json!({
                "interac_customer_info": {
                    "customer_name": data.name,
                    "customer_email": data.email,
                    "customer_phone_number": data.mobile,
                    "customer_bank_name": response.interac_bank_name,
                }
            })
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// ===== REQUEST TRANSFORMER (REFUND) =====
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GigadatRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for GigadatRefundRequest
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: GigadatRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = GigadatAuthType::try_from(&item.router_data.connector_config)?;

        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(ConnectorError::AmountConversionFailed)?;

        Ok(Self {
            amount,
            transaction_id: item.router_data.request.connector_transaction_id.clone(),
            campaign_id: auth.campaign_id,
        })
    }
}

// ===== RESPONSE TRANSFORMER (REFUND) =====
impl TryFrom<ResponseRouterData<GigadatRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GigadatRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let mut router_data = item.router_data;

        // Determine refund status based on HTTP code
        let refund_status = match item.http_code {
            200 => RefundStatus::Success,
            400 | 401 | 422 => RefundStatus::Failure,
            _ => RefundStatus::Pending,
        };

        router_data.response = Ok(RefundsResponseData {
            connector_refund_id: response.data.transaction_id,
            refund_status,
            status_code: item.http_code,
        });

        Ok(router_data)
    }
}
