use crate::types::ResponseRouterData;
use common_enums::{enums::Currency, AttemptStatus, CaptureMethod};
use common_utils::types::MinorUnit;
use domain_types::errors::{ConnectorResponseTransformationError, IntegrationError};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SilverflowAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
    pub merchant_acceptor_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for SilverflowAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Silverflow {
                api_key,
                api_secret,
                merchant_acceptor_key,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
                merchant_acceptor_key: merchant_acceptor_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// Error response structures matching Silverflow's nested error format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverflowErrorDetails {
    pub field: Option<String>,
    pub issue: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowError {
    pub code: String,
    pub message: String,
    pub trace_id: Option<String>,
    pub details: Option<SilverflowErrorDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverflowErrorResponse {
    pub error: SilverflowError,
}

impl Default for SilverflowErrorResponse {
    fn default() -> Self {
        Self {
            error: SilverflowError {
                code: "UNKNOWN_ERROR".to_string(),
                message: "An unknown error occurred".to_string(),
                trace_id: None,
                details: None,
            },
        }
    }
}

// Silverflow enums for type safety
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SilverflowPaymentIntent {
    Purchase,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SilverflowCardEntry {
    ECommerce,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SilverflowOrderType {
    Checkout,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SilverflowClearingMode {
    Manual,
    Auto,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum SilverflowAuthorizationStatus {
    Approved,
    Declined,
    Failed,
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum SilverflowClearingStatus {
    Cleared,
    Settled,
    Pending,
    Failed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SilverflowActionStatus {
    Completed,
    Success,
    Failed,
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowPaymentsRequest<T: PaymentMethodDataTypes> {
    pub merchant_acceptor_resolver: SilverflowMerchantAcceptorResolver,
    pub card: SilverflowCard<T>,
    #[serde(rename = "type")]
    pub payment_type: SilverflowPaymentType,
    pub amount: SilverflowAmount,
    pub clearing_mode: SilverflowClearingMode,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowMerchantAcceptorResolver {
    pub merchant_acceptor_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiry_year: Secret<u16>,
    pub expiry_month: Secret<u8>,
    pub cvc: Secret<String>,
    pub holder_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowPaymentType {
    pub intent: SilverflowPaymentIntent,
    pub card_entry: SilverflowCardEntry,
    pub order: SilverflowOrderType,
}

#[derive(Debug, Serialize)]
pub struct SilverflowAmount {
    pub value: MinorUnit,
    pub currency: Currency,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::SilverflowRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for SilverflowPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::SilverflowRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth credentials
        let auth = SilverflowAuthType::try_from(&router_data.connector_config)?;

        // Extract card data from payment method
        let card_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(IntegrationError::not_implemented(
                    "Only card payments are supported".to_string(),
                )
                .into())
            }
        };

        // Parse expiry year and month
        let expiry_year = card_data
            .card_exp_year
            .clone()
            .expose()
            .parse::<u16>()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let expiry_month = card_data
            .card_exp_month
            .clone()
            .expose()
            .parse::<u8>()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            merchant_acceptor_resolver: SilverflowMerchantAcceptorResolver {
                merchant_acceptor_key: auth.merchant_acceptor_key.expose(),
            },
            card: SilverflowCard {
                number: card_data.card_number.clone(),
                expiry_year: Secret::new(expiry_year),
                expiry_month: Secret::new(expiry_month),
                cvc: card_data.card_cvc.clone(),
                holder_name: router_data.request.customer_name.clone().map(Secret::new),
            },
            payment_type: SilverflowPaymentType {
                intent: SilverflowPaymentIntent::Purchase,
                card_entry: SilverflowCardEntry::ECommerce,
                order: SilverflowOrderType::Checkout,
            },
            amount: SilverflowAmount {
                value: router_data.request.minor_amount,
                currency: router_data.request.currency,
            },
            clearing_mode: match router_data.request.capture_method {
                Some(CaptureMethod::Manual) | Some(CaptureMethod::ManualMultiple) => {
                    SilverflowClearingMode::Manual
                }
                _ => SilverflowClearingMode::Auto,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowPaymentsResponse {
    pub key: String,
    pub merchant_acceptor_ref: Option<SilverflowMerchantAcceptorRef>,
    pub card: Option<SilverflowCardResponse>,
    pub amount: SilverflowAmountResponse,
    #[serde(rename = "type")]
    pub payment_type: SilverflowPaymentTypeResponse,
    pub clearing_mode: Option<String>,
    pub status: SilverflowStatus,
    pub authentication: Option<SilverflowAuthentication>,
    pub local_transaction_date_time: Option<String>,
    pub fraud_liability: Option<String>,
    pub authorization_iso_fields: Option<SilverflowAuthorizationIsoFields>,
    pub created: Option<String>,
    pub version: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SilverflowMerchantAcceptorRef {
    pub key: String,
    pub version: i32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowCardResponse {
    pub masked_number: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SilverflowAmountResponse {
    pub value: MinorUnit,
    pub currency: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowPaymentTypeResponse {
    pub intent: String,
    pub card_entry: String,
    pub order: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SilverflowStatus {
    pub authentication: String,
    pub authorization: SilverflowAuthorizationStatus,
    pub clearing: SilverflowClearingStatus,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SilverflowAuthentication {
    pub sca: Option<SilverflowSca>,
    pub cvc: Option<String>,
    pub avs: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowSca {
    pub compliance: String,
    pub compliance_reason: String,
    pub method: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowAuthorizationIsoFields {
    pub response_code: String,
    pub response_code_description: String,
    pub authorization_code: String,
    pub network_code: String,
    pub system_trace_audit_number: String,
    pub retrieval_reference_number: String,
    pub eci: String,
    pub network_specific_fields: Option<SilverflowNetworkSpecificFields>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowNetworkSpecificFields {
    pub transaction_identifier: Option<String>,
    pub cvv2_result_code: Option<String>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<SilverflowPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<SilverflowPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map status based on Silverflow's authorization and clearing status
        // This follows the multi-dimensional status mapping pattern as per best practices
        let status = match (
            &item.response.status.authorization,
            &item.response.status.clearing,
        ) {
            // Approved authorization - check clearing status for final determination
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Cleared) => {
                AttemptStatus::Charged
            }
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Settled) => {
                AttemptStatus::Charged
            }
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Pending) => {
                AttemptStatus::Authorized
            }
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Failed) => {
                AttemptStatus::Failure
            }
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Unknown) => {
                AttemptStatus::Authorized
            }
            // Failed or declined authorization
            (SilverflowAuthorizationStatus::Declined, _) => AttemptStatus::Failure,
            (SilverflowAuthorizationStatus::Failed, _) => AttemptStatus::Failure,
            // Pending authorization
            (SilverflowAuthorizationStatus::Pending, _) => AttemptStatus::Pending,
            // Unknown authorization status
            (SilverflowAuthorizationStatus::Unknown, _) => AttemptStatus::Pending,
        };

        // Extract network transaction ID from authorization ISO fields
        let network_txn_id = item
            .response
            .authorization_iso_fields
            .as_ref()
            .and_then(|iso| iso.network_specific_fields.as_ref())
            .and_then(|nsf| nsf.transaction_identifier.clone());

        // Extract authorization code for connector response reference
        let connector_response_reference_id = item
            .response
            .authorization_iso_fields
            .as_ref()
            .map(|iso| iso.authorization_code.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.key),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                connector_response_reference_id,
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

// PSync flow structures
// Reuse SilverflowPaymentsResponse for sync response since GET /charges/{chargeKey} returns the same structure
pub type SilverflowSyncResponse = SilverflowPaymentsResponse;

// PSync Response Transformation
impl TryFrom<ResponseRouterData<SilverflowSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<SilverflowSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map status based on Silverflow's status fields
        // This follows the multi-dimensional status mapping pattern as per best practices
        let status = match (
            &item.response.status.authorization,
            &item.response.status.clearing,
        ) {
            // Approved authorization - check clearing status for final determination
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Cleared) => {
                AttemptStatus::Charged
            }
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Settled) => {
                AttemptStatus::Charged
            }
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Pending) => {
                AttemptStatus::Authorized
            }
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Failed) => {
                AttemptStatus::Failure
            }
            (SilverflowAuthorizationStatus::Approved, SilverflowClearingStatus::Unknown) => {
                AttemptStatus::Authorized
            }
            // Failed or declined authorization
            (SilverflowAuthorizationStatus::Declined, _) => AttemptStatus::Failure,
            (SilverflowAuthorizationStatus::Failed, _) => AttemptStatus::Failure,
            // Pending authorization
            (SilverflowAuthorizationStatus::Pending, _) => AttemptStatus::Pending,
            // Unknown authorization status
            (SilverflowAuthorizationStatus::Unknown, _) => AttemptStatus::Pending,
        };

        // Extract network transaction ID from authorization ISO fields
        let network_txn_id = item
            .response
            .authorization_iso_fields
            .as_ref()
            .and_then(|iso| iso.network_specific_fields.as_ref())
            .and_then(|nsf| nsf.transaction_identifier.clone());

        // Extract authorization code for connector response reference
        let connector_response_reference_id = item
            .response
            .authorization_iso_fields
            .as_ref()
            .map(|iso| iso.authorization_code.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.key),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                connector_response_reference_id,
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
// Capture flow structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowCaptureRequest {
    pub amount: Option<MinorUnit>,
    pub close_charge: Option<bool>,
    pub reference: Option<String>,
}

// Capture response structure based on Silverflow clear API
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowCaptureResponse {
    #[serde(rename = "type")]
    pub action_type: String, // Should be "clearing"
    pub key: String, // Action key (act-...)
    pub charge_key: String,
    pub status: SilverflowActionStatus,
    pub reference: Option<String>,
    pub amount: SilverflowAmountResponse,
    pub close_charge: Option<bool>,
    pub clear_after: Option<String>,
    pub created: Option<String>,
    pub last_modified: Option<String>,
    pub version: Option<i32>,
}

// Capture Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::SilverflowRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for SilverflowCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::SilverflowRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Use the capture amount for partial capture, omit for full capture
        let amount = Some(router_data.request.minor_amount_to_capture);

        // Get connector transaction ID string for reference
        let reference = Some(
            router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?,
        );

        Ok(Self {
            amount,
            close_charge: Some(true), // Close the charge after capture
            reference,
        })
    }
}

// Capture Response Transformation
impl TryFrom<ResponseRouterData<SilverflowCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<SilverflowCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map status based on Silverflow's action status for capture flow
        let status = match item.response.status {
            SilverflowActionStatus::Completed | SilverflowActionStatus::Success => {
                AttemptStatus::Charged
            }
            SilverflowActionStatus::Pending => AttemptStatus::Pending,
            SilverflowActionStatus::Failed => AttemptStatus::Failure,
            SilverflowActionStatus::Unknown => AttemptStatus::Pending,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.charge_key),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.key),
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

// Refund flow structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowRefundRequest {
    pub refund_amount: Option<MinorUnit>,
    pub reference: Option<String>,
}

// Refund response structure based on Silverflow refund API
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowRefundResponse {
    #[serde(rename = "type")]
    pub action_type: String,
    pub key: String, // Action key (act-...)
    pub charge_key: String,
    pub refund_charge_key: Option<String>,
    pub reference: Option<String>,
    pub amount: SilverflowAmountResponse,
    pub status: SilverflowActionStatus,
    pub authorization_response: Option<SilverflowAuthorizationResponse>,
    pub created: Option<String>,
    pub last_modified: Option<String>,
    pub version: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowAuthorizationResponse {
    pub network: Option<String>,
    pub response_code: Option<String>,
    pub response_code_description: Option<String>,
}

// Void/Reversal status structure (simpler than charge status)
#[derive(Debug, Deserialize, Serialize)]
pub struct SilverflowVoidStatus {
    pub authorization: SilverflowAuthorizationStatus,
}

// Refund Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::SilverflowRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for SilverflowRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::SilverflowRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Use the refund amount for partial refund, omit for full refund
        let amount = Some(router_data.request.minor_refund_amount);

        // Get refund ID as reference
        let reference = Some(router_data.request.refund_id.clone());

        Ok(Self {
            refund_amount: amount,
            reference,
        })
    }
}

// Refund Response Transformation
impl TryFrom<ResponseRouterData<SilverflowRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<SilverflowRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map refund status based on Silverflow's status
        let refund_status = match item.response.status {
            SilverflowActionStatus::Success | SilverflowActionStatus::Completed => {
                common_enums::RefundStatus::Success
            }
            SilverflowActionStatus::Pending => common_enums::RefundStatus::Pending,
            SilverflowActionStatus::Failed => common_enums::RefundStatus::Failure,
            SilverflowActionStatus::Unknown => common_enums::RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.key,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Refund Sync flow structures
#[derive(Debug, Serialize)]
pub struct SilverflowRefundSyncRequest;

// Refund sync returns the refund action details, not the charge details
// The response structure is the same as the refund execute response
pub type SilverflowRefundSyncResponse = SilverflowRefundResponse;

// Refund Sync Request Transformation (empty for GET-based connector)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::SilverflowRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for SilverflowRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::SilverflowRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request for GET-based sync
        Ok(Self)
    }
}

// Refund Sync Response Transformation
impl TryFrom<ResponseRouterData<SilverflowRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<SilverflowRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map refund status based on Silverflow's refund action status
        // This is the CORRECT way - check the action status, not authorization status
        let refund_status = match item.response.status {
            SilverflowActionStatus::Success | SilverflowActionStatus::Completed => {
                common_enums::RefundStatus::Success
            }
            SilverflowActionStatus::Failed => common_enums::RefundStatus::Failure,
            SilverflowActionStatus::Pending => common_enums::RefundStatus::Pending,
            SilverflowActionStatus::Unknown => common_enums::RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.key,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
} // Void flow structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowVoidRequest {
    pub replacement_amount: Option<MinorUnit>,
    pub reference: Option<String>,
}

// Void response structure based on Silverflow reverse API
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilverflowVoidResponse {
    #[serde(rename = "type")]
    pub action_type: String, // Should be "reversal"
    pub key: String, // Action key (act-...)
    pub charge_key: String,
    pub reference: Option<String>,
    pub replacement_amount: Option<SilverflowAmountResponse>,
    pub status: SilverflowVoidStatus, // Reversal has different status structure
    pub authorization_response: Option<SilverflowAuthorizationResponse>,
    pub created: Option<String>,
    pub last_modified: Option<String>,
    pub version: Option<i32>,
}

// Void Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::SilverflowRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for SilverflowVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::SilverflowRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get connector transaction ID string for reference
        let reference = Some(router_data.request.connector_transaction_id.clone());

        Ok(Self {
            replacement_amount: Some(MinorUnit::zero()), // 0 means full reversal according to Silverflow docs
            reference,
        })
    }
}

// Void Response Transformation
impl TryFrom<ResponseRouterData<SilverflowVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<SilverflowVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map status based on Silverflow's authorization status for void operations
        let status = match item.response.status.authorization {
            SilverflowAuthorizationStatus::Approved => AttemptStatus::Voided,
            SilverflowAuthorizationStatus::Declined => AttemptStatus::VoidFailed,
            SilverflowAuthorizationStatus::Failed => AttemptStatus::VoidFailed,
            SilverflowAuthorizationStatus::Pending => AttemptStatus::Pending,
            SilverflowAuthorizationStatus::Unknown => AttemptStatus::Pending,
        };

        // Extract network transaction ID from authorization response (if available)
        let network_txn_id = item
            .response
            .authorization_response
            .as_ref()
            .and_then(|auth| auth.network.clone());

        // Extract authorization code for connector response reference
        let connector_response_reference_id = item
            .response
            .authorization_response
            .as_ref()
            .and_then(|auth| auth.response_code.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.key),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                connector_response_reference_id,
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
