use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::ResponseRouterData;
use base64::{engine::general_purpose, Engine};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    crypto::{self, RsaOaepSha256, SignMessage},
    FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund, ServerAuthenticationToken, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// Constants for encryption and token formatting
pub(crate) const ENCRYPTION_TYPE_RSA: &str = "RSA";
pub(crate) const ACCESS_TOKEN_SEPARATOR: &str = "|||";

#[derive(Debug, Clone)]
pub struct FiservcommercehubAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
    pub merchant_id: Secret<String>,
    pub terminal_id: Secret<String>,
}

impl FiservcommercehubAuthType {
    pub fn generate_hmac_signature(
        &self,
        api_key: &str,
        client_request_id: &str,
        timestamp: &str,
        request_body: &str,
    ) -> Result<String, error_stack::Report<errors::IntegrationError>> {
        let raw_signature = format!("{api_key}{client_request_id}{timestamp}{request_body}");
        let signature = crypto::HmacSha256
            .sign_message(self.api_secret.peek().as_bytes(), raw_signature.as_bytes())
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        Ok(general_purpose::STANDARD.encode(signature))
    }

    pub fn generate_client_request_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    pub fn generate_timestamp() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string()
    }
}

impl TryFrom<&ConnectorSpecificConfig> for FiservcommercehubAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Fiservcommercehub {
                api_key,
                secret: api_secret,
                merchant_id,
                terminal_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
                merchant_id: merchant_id.to_owned(),
                terminal_id: terminal_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorResponse {
    pub gateway_response: Option<FiservcommercehubErrorGatewayResponse>,
    pub error: Vec<FiservcommercehubErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorGatewayResponse {
    pub transaction_state: Option<String>,
    pub transaction_processing_details: Option<FiservcommercehubErrorTxnDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorTxnDetails {
    pub api_trace_id: Option<String>,
    pub transaction_id: Option<String>,
    pub order_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiservcommercehubErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
}

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAuthorizeRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub source: FiservcommercehubSourceData,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub transaction_details: FiservcommercehubTransactionDetailsReq,
    pub transaction_interaction: FiservcommercehubTransactionInteractionReq,
    /// Additional 3DS data for external 3DS authentication (when authentication_data is present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<FiservcommercehubAdditionalData3DS>,
}

#[derive(Debug, Serialize)]
pub struct FiservcommercehubAuthorizeAmount {
    pub currency: common_enums::Currency,
    pub total: FloatMajorUnit,
}

#[derive(Debug, Serialize)]
pub enum FiservcommercehubSourceType {
    PaymentCard,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSourceData {
    pub source_type: FiservcommercehubSourceType,
    pub encryption_data: FiservcommercehubEncryptionData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubEncryptionData {
    pub key_id: String,
    pub encryption_type: String,
    pub encryption_block: Secret<String>,
    pub encryption_block_fields: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionDetailsReq {
    pub capture_flag: bool,
    pub merchant_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubOrigin {
    Ecom,
    Moto,
    Pos,
}

impl From<Option<&common_enums::PaymentChannel>> for FiservcommercehubOrigin {
    fn from(channel: Option<&common_enums::PaymentChannel>) -> Self {
        match channel {
            Some(common_enums::PaymentChannel::MailOrder)
            | Some(common_enums::PaymentChannel::TelephoneOrder) => Self::Moto,
            Some(common_enums::PaymentChannel::Ecommerce) | None => Self::Ecom,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionInteractionReq {
    pub origin: FiservcommercehubOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubMpiData {
    pub cavv: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAdditionalData3DS {
    pub ds_transaction_id: String,
    pub mpi_data: FiservcommercehubMpiData,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let total = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        let access_token = router_data.resource_common_data.get_access_token()?;
        let parts: Vec<&str> = access_token.split(ACCESS_TOKEN_SEPARATOR).collect();

        let key_id = parts
            .first()
            .ok_or_else(|| {
                error_stack::report!(errors::IntegrationError::MissingRequiredField {
                    field_name: "key_id",
                    context: Default::default()
                })
            })?
            .to_string();

        let encoded_public_key = parts.get(1).ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "encoded_public_key",
                context: Default::default()
            })
        })?;

        let public_key_der = general_purpose::STANDARD
            .decode(encoded_public_key)
            .map_err(|_| {
                error_stack::report!(errors::IntegrationError::RequestEncodingFailed {
                    context: Default::default()
                })
            })
            .attach_printable("Failed to decode Base64 RSA public key")?;

        let auth_type = &router_data.connector_config;
        let auth = FiservcommercehubAuthType::try_from(auth_type)?;

        let source = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                let card_data = card.card_number.peek().to_string();
                let name_on_card = card
                    .card_holder_name
                    .as_ref()
                    .map(|n| n.peek().clone())
                    .ok_or(errors::IntegrationError::MissingRequiredField {
                        field_name: "card_holder_name",
                        context: Default::default(),
                    })?;
                let expiration_month = card.card_exp_month.peek().to_string();
                let expiration_year = card.get_expiry_year_4_digit().peek().to_string();

                let plain_block =
                    format!("{card_data}{name_on_card}{expiration_month}{expiration_year}");

                let card_data_len = card_data.len();
                let name_on_card_len = name_on_card.len();
                let expiration_month_len = expiration_month.len();
                let expiration_year_len = expiration_year.len();
                let encryption_block_fields = format!(
                    "card.cardData:{card_data_len},card.nameOnCard:{name_on_card_len},card.expirationMonth:{expiration_month_len},card.expirationYear:{expiration_year_len}"
                );

                let encrypted_bytes =
                    RsaOaepSha256::encrypt(&public_key_der, plain_block.as_bytes())
                        .change_context(errors::IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                        .attach_printable("RSA OAEP-SHA256 encryption of card data failed")?;

                let encryption_block =
                    Secret::new(general_purpose::STANDARD.encode(&encrypted_bytes));

                FiservcommercehubSourceData {
                    source_type: FiservcommercehubSourceType::PaymentCard,
                    encryption_data: FiservcommercehubEncryptionData {
                        key_id,
                        encryption_type: ENCRYPTION_TYPE_RSA.to_string(),
                        encryption_block,
                        encryption_block_fields,
                    },
                }
            }
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "This payment method is not implemented".to_string(),
                        Default::default()
                    )
                ))
            }
        };

        let origin = FiservcommercehubOrigin::from(router_data.request.payment_channel.as_ref());

        let eci_indicator = router_data
            .request
            .authentication_data
            .as_ref()
            .and_then(|auth_data| auth_data.eci.clone());

        let additional_data_3ds =
            if let Some(ref auth_data) = router_data.request.authentication_data {
                match (&auth_data.ds_trans_id, &auth_data.cavv) {
                    (Some(ds_trans_id), Some(cavv)) => {
                        let xid = auth_data
                            .threeds_server_transaction_id
                            .clone()
                            .or_else(|| auth_data.ds_trans_id.clone());

                        Some(FiservcommercehubAdditionalData3DS {
                            ds_transaction_id: ds_trans_id.clone(),
                            mpi_data: FiservcommercehubMpiData {
                                cavv: cavv.clone(),
                                xid,
                            },
                        })
                    }
                    _ => None,
                }
            } else {
                None
            };

        let request = Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            source,
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            transaction_details: FiservcommercehubTransactionDetailsReq {
                capture_flag: true,
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            transaction_interaction: FiservcommercehubTransactionInteractionReq {
                origin,
                eci_indicator,
            },
            additional_data_3ds,
        };
        Ok(request)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubTransactionState {
    Approved,
    Captured,
    Authorized,
    Pending,
    Declined,
    Rejected,
    Failed,
    Cancelled,
}

impl From<&FiservcommercehubTransactionState> for AttemptStatus {
    fn from(state: &FiservcommercehubTransactionState) -> Self {
        match state {
            FiservcommercehubTransactionState::Approved
            | FiservcommercehubTransactionState::Captured => Self::Charged,
            FiservcommercehubTransactionState::Authorized => Self::Authorized,
            FiservcommercehubTransactionState::Pending => Self::Pending,
            FiservcommercehubTransactionState::Declined
            | FiservcommercehubTransactionState::Rejected
            | FiservcommercehubTransactionState::Failed => Self::Failure,
            FiservcommercehubTransactionState::Cancelled => Self::Voided,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubRefundState {
    Approved,
    Captured,
    Authorized,
    Pending,
    Declined,
    Rejected,
    Failed,
    Cancelled,
}

impl From<&FiservcommercehubRefundState> for RefundStatus {
    fn from(state: &FiservcommercehubRefundState) -> Self {
        match state {
            FiservcommercehubRefundState::Approved | FiservcommercehubRefundState::Captured => {
                Self::Success
            }
            FiservcommercehubRefundState::Authorized | FiservcommercehubRefundState::Pending => {
                Self::Pending
            }
            FiservcommercehubRefundState::Declined
            | FiservcommercehubRefundState::Rejected
            | FiservcommercehubRefundState::Failed
            | FiservcommercehubRefundState::Cancelled => Self::Failure,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAuthorizeResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubGatewayResponseBody {
    pub transaction_state: FiservcommercehubTransactionState,
    pub transaction_processing_details: FiservcommercehubTxnDetails,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTxnDetails {
    pub order_id: Option<String>,
    pub transaction_id: String,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<FiservcommercehubAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        let status = AttemptStatus::from(&item.response.gateway_response.transaction_state);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: txn.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// PSYNC FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncMerchantDetails {
    pub merchant_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubReferenceTransactionDetails {
    pub reference_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncRequest {
    pub merchant_details: FiservcommercehubPSyncMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for FiservcommercehubPSyncRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;
        Ok(Self {
            merchant_details: FiservcommercehubPSyncMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: connector_transaction_id,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncGatewayResponse {
    pub transaction_state: FiservcommercehubTransactionState,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncItem {
    pub gateway_response: FiservcommercehubPSyncGatewayResponse,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FiservcommercehubPSyncResponse(pub Vec<FiservcommercehubPSyncItem>);

impl TryFrom<ResponseRouterData<FiservcommercehubPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let psync_item = item.response.0.into_iter().next().ok_or_else(|| {
            error_stack::report!(
                crate::utils::response_deserialization_fail(
                    item.http_code
                , "fiservcommercehub: response body did not match the expected format; confirm API version and connector documentation.")
            )
        })?;
        let status = AttemptStatus::from(&psync_item.gateway_response.transaction_state);
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REFUND FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundTransactionDetails {
    pub capture_flag: bool,
    pub merchant_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub transaction_details: FiservcommercehubRefundTransactionDetails,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for FiservcommercehubRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let total = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            transaction_details: FiservcommercehubRefundTransactionDetails {
                capture_flag: true,
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_transaction_id.clone(),
            },
        })
    }
}

/// Response body from `POST /payments/v1/refunds`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundResponse {
    pub gateway_response: FiservcommercehubRefundGatewayResponseBody,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundGatewayResponseBody {
    pub transaction_state: FiservcommercehubRefundState,
    pub transaction_processing_details: FiservcommercehubTxnDetails,
}

impl TryFrom<ResponseRouterData<FiservcommercehubRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.gateway_response.transaction_state);
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: txn.transaction_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// RSYNC FLOW (Refund Sync)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncRequest {
    pub merchant_details: FiservcommercehubPSyncMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for FiservcommercehubRSyncRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            merchant_details: FiservcommercehubPSyncMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_refund_id.clone(),
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncGatewayResponse {
    pub transaction_state: FiservcommercehubRefundState,
    pub transaction_processing_details: Option<FiservcommercehubRSyncTxnDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncTxnDetails {
    pub transaction_id: String,
    pub order_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncItem {
    pub gateway_response: FiservcommercehubRSyncGatewayResponse,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FiservcommercehubRSyncResponse(pub Vec<FiservcommercehubRSyncItem>);

impl TryFrom<ResponseRouterData<FiservcommercehubRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let rsync_item = item.response.0.into_iter().next().ok_or_else(|| {
            error_stack::report!(
                crate::utils::response_deserialization_fail(
                    item.http_code
                , "fiservcommercehub: response body did not match the expected format; confirm API version and connector documentation.")
            )
        })?;
        let refund_status = RefundStatus::from(&rsync_item.gateway_response.transaction_state);
        let connector_refund_id = rsync_item
            .gateway_response
            .transaction_processing_details
            .map(|d| d.transaction_id)
            .unwrap_or(item.router_data.request.connector_refund_id.clone());
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// VOID FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubVoidRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<FiservcommercehubAuthorizeAmount>,
    pub transaction_details: FiservcommercehubRefundTransactionDetails,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for FiservcommercehubVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;

        let amount = match (router_data.request.amount, router_data.request.currency) {
            (Some(minor_amount), Some(currency)) => {
                let total =
                    utils::convert_amount(item.connector.amount_converter, minor_amount, currency)?;
                Some(FiservcommercehubAuthorizeAmount { currency, total })
            }
            _ => None,
        };

        Ok(Self {
            amount,
            transaction_details: FiservcommercehubRefundTransactionDetails {
                capture_flag: true,
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_transaction_id.clone(),
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubVoidResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
}

impl TryFrom<ResponseRouterData<FiservcommercehubVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.gateway_response.transaction_state);
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: txn.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// ACCESS TOKEN FLOW
// =============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubMerchantDetails {
    pub merchant_id: Secret<String>,
    pub terminal_id: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAccessTokenRequest {
    pub merchant_details: FiservcommercehubMerchantDetails,
}

impl TryFrom<&ConnectorSpecificConfig> for FiservcommercehubAccessTokenRequest {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        let auth = FiservcommercehubAuthType::try_from(auth_type)?;
        Ok(Self {
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for FiservcommercehubAccessTokenRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data.connector_config)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionProcessingDetails {
    pub api_key: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubGatewayResponse {
    pub transaction_processing_details: FiservcommercehubTransactionProcessingDetails,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAsymmetricKeyDetails {
    pub key_id: String,
    pub encoded_public_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAccessTokenResponse {
    pub gateway_response: FiservcommercehubGatewayResponse,
    pub asymmetric_key_details: FiservcommercehubAsymmetricKeyDetails,
}

impl<F, T> TryFrom<ResponseRouterData<FiservcommercehubAccessTokenResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let key_id = &item.response.asymmetric_key_details.key_id;
        let encoded_public_key = &item.response.asymmetric_key_details.encoded_public_key;
        let combined_token = Secret::new(format!(
            "{key_id}{ACCESS_TOKEN_SEPARATOR}{encoded_public_key}"
        ));
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: combined_token,
                expires_in: Some(604_800), // 1 week in seconds
                token_type: None,
            }),
            ..item.router_data
        })
    }
}
