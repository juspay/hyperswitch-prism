use crate::{connectors::getnet::GetnetRouterData, types::ResponseRouterData};
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{id_type::CustomerId, types::MinorUnit};
use domain_types::errors::{ConnectorResponseTransformationError, IntegrationError};
use domain_types::{
    connector_flow::{Authorize, Capture, CreateAccessToken, PSync, RSync, Refund, Void},
    connector_types::{
        AccessTokenRequestData, AccessTokenResponseData, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
use std::fmt;

const TRANSACTION_TYPE_FULL: &str = "FULL";
const DEFAULT_INSTALLMENTS: i32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GetnetPaymentMethod {
    #[serde(rename = "CREDIT")]
    DirectCredit,
    #[serde(rename = "CREDIT_AUTHORIZATION")]
    DirectCreditAuthorization,
}

impl fmt::Display for GetnetPaymentMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectCredit => write!(f, "CREDIT"),
            Self::DirectCreditAuthorization => write!(f, "CREDIT_AUTHORIZATION"),
        }
    }
}

impl GetnetPaymentMethod {
    /// Determine payment method based on capture method
    fn from_capture_method(capture_method: Option<common_enums::CaptureMethod>) -> Self {
        match capture_method {
            Some(common_enums::CaptureMethod::Manual) => Self::DirectCreditAuthorization,
            _ => Self::DirectCredit,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum GetnetCardBrand {
    Mastercard,
    Visa,
    Amex,
    Elo,
    Hipercard,
}

#[derive(Debug, Clone)]
pub struct GetnetAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
    pub seller_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GetnetAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Getnet {
                api_key,
                api_secret,
                seller_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
                seller_id: seller_id.to_owned(),
            }),
            _other => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetnetErrorResponse {
    #[serde(rename = "error_code")]
    pub code: Option<String>,
    pub message: String,
    pub details: Option<Vec<GetnetErrorDetail>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetnetErrorDetail {
    pub field: Option<String>,
    pub message: Option<String>,
}

// ===== STATUS ENUMS =====
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum GetnetPaymentStatus {
    Approved,
    Captured,
    Pending,
    Waiting,
    Authorized,
    Denied,
    Failed,
    Error,
    Canceled,
    Cancelled,
    #[serde(other)]
    Unknown,
}

impl From<&GetnetPaymentStatus> for AttemptStatus {
    fn from(status: &GetnetPaymentStatus) -> Self {
        match status {
            GetnetPaymentStatus::Approved | GetnetPaymentStatus::Captured => Self::Charged,
            GetnetPaymentStatus::Authorized => Self::Authorized,
            GetnetPaymentStatus::Pending | GetnetPaymentStatus::Waiting => Self::Pending,
            GetnetPaymentStatus::Denied
            | GetnetPaymentStatus::Failed
            | GetnetPaymentStatus::Error => Self::Failure,
            GetnetPaymentStatus::Canceled | GetnetPaymentStatus::Cancelled => Self::Voided,
            GetnetPaymentStatus::Unknown => Self::Pending,
        }
    }
}

impl From<&GetnetPaymentStatus> for RefundStatus {
    fn from(status: &GetnetPaymentStatus) -> Self {
        match status {
            GetnetPaymentStatus::Canceled | GetnetPaymentStatus::Cancelled => Self::Success,
            GetnetPaymentStatus::Pending | GetnetPaymentStatus::Waiting => Self::Pending,
            GetnetPaymentStatus::Denied
            | GetnetPaymentStatus::Failed
            | GetnetPaymentStatus::Error => Self::Failure,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GetnetAuthorizeRequest<T: PaymentMethodDataTypes> {
    pub request_id: String,
    pub idempotency_key: String,
    pub order_id: String,
    pub data: GetnetPaymentData<T>,
}

#[derive(Debug, Serialize)]
pub struct GetnetPaymentData<T: PaymentMethodDataTypes> {
    pub customer_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    pub payment: GetnetPayment<T>,
}

#[derive(Debug, Serialize)]
pub struct GetnetPayment<T: PaymentMethodDataTypes> {
    pub payment_id: String,
    pub payment_method: String,
    pub transaction_type: String,
    pub number_installments: i32,
    pub card: GetnetCard<T>,
}

#[derive(Debug, Serialize)]
pub struct GetnetCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub cardholder_name: Secret<String>,
    pub security_code: Secret<String>,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GetnetAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GetnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let card_data = match &item.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Payment method ".to_string(),
                    connector: "Getnet",
                    context: Default::default(),
                }
                .into())
            }
        };

        // Convert 4-digit year to 2-digit year (e.g., "2025" -> "25")
        let expiration_year = card_data.get_card_expiry_year_2_digit()?;

        let cardholder_name = card_data
            .card_holder_name
            .clone()
            .or_else(|| item.resource_common_data.get_optional_billing_full_name())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "payment_method.card.card_holder_name",
                context: Default::default(),
            })?;

        let card = GetnetCard {
            number: card_data.card_number.clone(),
            expiration_month: card_data.card_exp_month.clone(),
            expiration_year,
            cardholder_name,
            security_code: card_data.card_cvc.clone(),
        };

        let request_ref_id = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Determine payment method based on capture method
        let payment_method = GetnetPaymentMethod::from_capture_method(item.request.capture_method);

        let payment = GetnetPayment {
            payment_id: request_ref_id.clone(),
            payment_method: payment_method.to_string(),
            transaction_type: TRANSACTION_TYPE_FULL.to_string(),
            number_installments: DEFAULT_INSTALLMENTS,
            card,
        };

        let customer_id = item
            .resource_common_data
            .get_customer_id()
            .unwrap_or_else(|_| CustomerId::default())
            .get_string_repr()
            .to_string();

        let data = GetnetPaymentData {
            customer_id,
            amount: item.request.minor_amount,
            currency: item.request.currency,
            payment,
        };

        Ok(Self {
            request_id: request_ref_id.clone(),
            idempotency_key: request_ref_id.clone(),
            order_id: request_ref_id,
            data,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetAuthorizeResponse {
    pub payment_id: String,
    pub order_id: Option<String>,
    pub amount: MinorUnit,
    pub currency: Option<Currency>,
    pub status: GetnetPaymentStatus,
    pub payment_method: Option<String>,
    pub received_at: Option<String>,
    pub transaction_id: Option<String>,
    pub authorization_code: Option<String>,
    pub brand: Option<GetnetCardBrand>,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GetnetAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<GetnetAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Getnet returns status in the response - use it directly
        // The status mapping already handles both manual and automatic capture correctly:
        // - Authorized/Captured/Approved -> Charged (for automatic capture)
        // - Authorized/Pending -> Pending (for manual capture)
        // - Denied/Failed/Error -> Failure
        let status = AttemptStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: item.response.transaction_id.clone(),
                connector_response_reference_id: item.response.order_id.clone(),
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

#[derive(Debug, Serialize)]
pub struct GetnetCaptureRequest {
    pub idempotency_key: String,
    pub payment_id: String,
    pub amount: MinorUnit,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for GetnetCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GetnetRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;

        let capture_amount = router_data.request.amount_to_capture;

        let capture_amount_minor = MinorUnit::new(capture_amount);

        Ok(Self {
            idempotency_key: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_id,
            amount: capture_amount_minor,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetCaptureResponse {
    pub idempotency_key: Option<String>,
    pub seller_id: Option<String>,
    pub payment_id: String,
    pub order_id: Option<String>,
    pub amount: MinorUnit,
    pub currency: Option<Currency>,
    pub status: GetnetPaymentStatus,
    pub reason_code: Option<String>,
    pub reason_message: Option<String>,
    pub captured_at: Option<String>,
}

impl TryFrom<ResponseRouterData<GetnetCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<GetnetCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
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

// ===== PSYNC RESPONSE =====
#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncResponse {
    pub payment_id: String,
    pub order_id: Option<String>,
    pub status: GetnetPaymentStatus,
    pub payment: Option<GetnetSyncPaymentDetails>,
    pub records: Option<Vec<GetnetSyncRecord>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncPaymentDetails {
    pub payment_method: String,
    pub transaction_type: String,
    pub card: GetnetSyncCardDetails,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncCardDetails {
    pub number: Secret<String>,
    pub brand: GetnetCardBrand,
    pub expiration_year: Secret<String>,
    pub expiration_month: Secret<String>,
    pub cardholder_name: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncRecord {
    pub rel: Option<String>,
    pub registered_at: Option<String>,
    pub idempotency_key: Option<String>,
    pub href: Option<String>,
}

impl TryFrom<ResponseRouterData<GetnetSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(item: ResponseRouterData<GetnetSyncResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
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

// ===== REFUND REQUEST =====
#[derive(Debug, Serialize)]
pub struct GetnetRefundRequest {
    pub idempotency_key: String,
    pub payment_id: String,
    pub amount: MinorUnit,
    pub payment_method: String,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for GetnetRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GetnetRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_id = router_data.request.connector_transaction_id.clone();

        // Determine payment method based on capture method
        let payment_method =
            GetnetPaymentMethod::from_capture_method(router_data.request.capture_method);

        Ok(Self {
            idempotency_key: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_id,
            amount: router_data.request.minor_refund_amount,
            payment_method: payment_method.to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetRefundResponse {
    pub idempotency_key: Option<String>,
    pub seller_id: Option<String>,
    pub payment_id: String,
    pub order_id: Option<String>,
    pub amount: MinorUnit,
    pub status: GetnetPaymentStatus,
    pub reason_code: Option<String>,
    pub reason_message: Option<String>,
    pub canceled_at: Option<String>,
}

impl TryFrom<ResponseRouterData<GetnetRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(item: ResponseRouterData<GetnetRefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.payment_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== RSYNC RESPONSE =====
pub type GetnetRefundSyncResponse = GetnetSyncResponse;

impl TryFrom<ResponseRouterData<GetnetRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<GetnetRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.payment_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
pub struct GetnetAccessTokenRequest {
    pub grant_type: String,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<
                CreateAccessToken,
                PaymentFlowData,
                AccessTokenRequestData,
                AccessTokenResponseData,
            >,
            T,
        >,
    > for GetnetAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GetnetRouterData<
            RouterDataV2<
                CreateAccessToken,
                PaymentFlowData,
                AccessTokenRequestData,
                AccessTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: item.router_data.request.grant_type,
        })
    }
}

// ===== ACCESS TOKEN RESPONSE =====
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetnetAccessTokenResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
    pub scope: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<GetnetAccessTokenResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, AccessTokenResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<GetnetAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(AccessTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in),
                token_type: Some(item.response.token_type),
            }),
            ..item.router_data
        })
    }
}

// ===== VOID REQUEST =====
// Getnet uses the same endpoint for both void and refund
pub type GetnetVoidRequest = GetnetRefundRequest;

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for GetnetVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GetnetRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_id = router_data.request.connector_transaction_id.clone();

        let void_amount =
            router_data
                .request
                .amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: Default::default(),
                })?;

        Ok(Self {
            idempotency_key: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_id,
            amount: void_amount,
            payment_method: GetnetPaymentMethod::DirectCreditAuthorization.to_string(),
        })
    }
}

// ===== VOID RESPONSE =====
// Getnet uses the same endpoint for both void and refund
pub type GetnetVoidResponse = GetnetRefundResponse;

impl TryFrom<ResponseRouterData<GetnetVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(item: ResponseRouterData<GetnetVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
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
