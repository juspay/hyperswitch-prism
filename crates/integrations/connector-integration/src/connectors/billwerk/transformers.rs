pub type RefundsResponseRouterData<F, T> =
    ResponseRouterData<T, RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>>;

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::MinorUnit,
};

use crate::{connectors::billwerk::BillwerkRouterData, types::ResponseRouterData, utils};

use domain_types::{
    connector_flow::{Authorize, Capture, PaymentMethodToken, RSync},
    connector_types::{
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::ConnectorError,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{
        ConnectorSpecificConfig, ErrorResponse, PaymentMethodToken as PaymentMethodTokenFlow,
    },
    router_data_v2::RouterDataV2,
};

use hyperswitch_masking::{ExposeInterface, Secret};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct BillwerkAuthType {
    pub api_key: Secret<String>,
    pub public_api_key: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillwerkErrorResponse {
    pub code: Option<i32>,
    pub error: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BillwerkTokenRequestIntent {
    ChargeAndStore,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BillwerkStrongAuthRule {
    UseScaIfAvailableAuth,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillwerkTokenRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    number: RawCardNumber<T>,
    month: Secret<String>,
    year: Secret<String>,
    cvv: Secret<String>,
    pkey: Secret<String>,
    recurring: Option<bool>,
    intent: Option<BillwerkTokenRequestIntent>,
    strong_authentication_rule: Option<BillwerkStrongAuthRule>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BillwerkTokenResponse {
    pub id: Secret<String>,
    pub recurring: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BillwerkPaymentsRequest {
    handle: String,
    amount: MinorUnit,
    source: Secret<String>,
    currency: common_enums::Currency,
    customer: BillwerkCustomerObject,
    metadata: Option<common_utils::pii::SecretSerdeValue>,
    settle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BillwerkPaymentState {
    Created,
    Authorized,
    Pending,
    Settled,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize)]
pub struct BillwerkCustomerObject {
    handle: Option<common_utils::id_type::CustomerId>,
    email: Option<common_utils::pii::Email>,
    address: Option<Secret<String>>,
    address2: Option<Secret<String>>,
    city: Option<Secret<String>>,
    country: Option<common_enums::CountryAlpha2>,
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct BillwerkCaptureRequest {
    amount: MinorUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BillwerkPaymentsResponse {
    state: BillwerkPaymentState,
    handle: String,
    error: Option<String>,
    error_state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefundState {
    Refunded,
    Failed,
    Processing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefundResponse {
    id: String,
    state: RefundState,
}

#[derive(Debug, Serialize)]
pub struct BillwerkRefundRequest {
    pub invoice: String,
    pub amount: MinorUnit,
    pub text: Option<String>,
}

pub type BillwerkRefundResponse = RefundResponse;

pub type BillwerkRSyncResponse = RefundResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BillwerkRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for BillwerkTokenRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: BillwerkRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ccard) => {
                let connector_auth = &item.router_data.connector_config;
                let auth_type = BillwerkAuthType::try_from(connector_auth)?;
                Ok(Self {
                    number: ccard.card_number.clone(),
                    month: ccard.card_exp_month.clone(),
                    year: ccard.get_card_expiry_year_2_digit()?,
                    cvv: ccard.card_cvc,
                    pkey: auth_type.public_api_key,
                    recurring: None,
                    intent: None,
                    strong_authentication_rule: None,
                })
            }
            PaymentMethodData::Wallet(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::CardToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(ConnectorError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("billwerk"),
                )
                .into())
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BillwerkRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BillwerkPaymentsRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: BillwerkRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if item.router_data.resource_common_data.is_three_ds() {
            return Err(ConnectorError::NotImplemented(
                "Three_ds payments through Billwerk".to_string(),
            )
            .into());
        };
        let PaymentMethodTokenFlow::Token(source) = item
            .router_data
            .resource_common_data
            .get_payment_method_token()?;
        Ok(Self {
            handle: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.router_data.request.amount,
            source,
            currency: item.router_data.request.currency,
            customer: BillwerkCustomerObject {
                handle: item.router_data.resource_common_data.customer_id.clone(),
                email: item.router_data.request.email.clone(),
                address: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_line1(),
                address2: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_line2(),
                city: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_city(),
                country: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_country(),
                first_name: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_first_name(),
                last_name: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_last_name(),
            },
            metadata: item.router_data.request.metadata.clone(),
            settle: item.router_data.request.is_auto_capture()?,
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<BillwerkTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BillwerkTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.id.expose(),
            }),
            ..item.router_data
        })
    }
}

impl<F, T> TryFrom<ResponseRouterData<BillwerkPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BillwerkPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let error_response = if response.error.is_some() || response.error_state.is_some() {
            Some(ErrorResponse {
                code: response
                    .error_state
                    .clone()
                    .unwrap_or(NO_ERROR_CODE.to_string()),
                message: response
                    .error
                    .clone()
                    .unwrap_or(NO_ERROR_MESSAGE.to_string()),
                reason: response.error,
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: Some(response.handle.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            None
        };
        let payments_response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.handle.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.handle),
            incremental_authorization_allowed: None,
            status_code: http_code,
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(response.state),
                ..router_data.resource_common_data
            },
            response: error_response.map_or_else(|| Ok(payments_response), Err),
            ..router_data
        })
    }
}

impl From<BillwerkPaymentState> for common_enums::AttemptStatus {
    fn from(item: BillwerkPaymentState) -> Self {
        match item {
            BillwerkPaymentState::Created | BillwerkPaymentState::Pending => Self::Pending,
            BillwerkPaymentState::Authorized => Self::Authorized,
            BillwerkPaymentState::Settled => Self::Charged,
            BillwerkPaymentState::Failed => Self::Failure,
            BillwerkPaymentState::Cancelled => Self::Voided,
        }
    }
}

impl TryFrom<&ConnectorSpecificConfig> for BillwerkAuthType {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Billwerk {
                api_key,
                public_api_key,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                public_api_key: public_api_key.to_owned(),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BillwerkRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for BillwerkCaptureRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: BillwerkRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.minor_amount_to_capture,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BillwerkRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for BillwerkRefundRequest
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: BillwerkRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.minor_refund_amount,
            invoice: item.router_data.request.connector_transaction_id.clone(),
            text: item.router_data.request.reason.clone(),
        })
    }
}

impl From<RefundState> for common_enums::RefundStatus {
    fn from(item: RefundState) -> Self {
        match item {
            RefundState::Refunded => Self::Success,
            RefundState::Failed => Self::Failure,
            RefundState::Processing => Self::Pending,
        }
    }
}

impl<F> TryFrom<RefundsResponseRouterData<F, RefundResponse>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: RefundsResponseRouterData<F, RefundResponse>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.to_string(),
                refund_status: common_enums::RefundStatus::from(item.response.state),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.to_string(),
                refund_status: common_enums::RefundStatus::from(item.response.state),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
