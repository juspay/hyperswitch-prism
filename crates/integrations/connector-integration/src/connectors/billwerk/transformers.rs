pub type RefundsResponseRouterData<F, T> =
    ResponseRouterData<T, RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>>;

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::MinorUnit,
};

use crate::{connectors::billwerk::BillwerkRouterData, types::ResponseRouterData, utils};

use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PaymentMethodToken, RSync, RepeatPayment,
        SetupMandate,
    },
    connector_types::{
        BillwerkClientAuthenticationResponse as BillwerkClientAuthenticationResponseDomain,
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};

use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    recurring: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BillwerkRepeatPaymentRequest {
    handle: String,
    amount: MinorUnit,
    source: Secret<String>,
    currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_handle: Option<String>,
    settle: bool,
}

#[derive(Debug, Serialize)]
pub struct BillwerkSetupMandateRequest {
    handle: String,
    amount: MinorUnit,
    source: Secret<String>,
    currency: common_enums::Currency,
    customer: BillwerkCustomerObject,
    settle: bool,
    recurring: bool,
}

pub type BillwerkSetupMandateResponse = BillwerkPaymentsResponse;

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
    recurring_payment_method: Option<String>,
}

pub type BillwerkRepeatPaymentResponse = BillwerkPaymentsResponse;

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
    type Error = error_stack::Report<IntegrationError>;

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
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("billwerk"),
                    Default::default(),
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
    type Error = error_stack::Report<IntegrationError>;

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
            return Err(IntegrationError::NotImplemented(
                "Three_ds payments through Billwerk".to_string(),
                Default::default(),
            )
            .into());
        };
        let source = match &item.router_data.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => t.token.clone(),
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Billwerk authorize only accepts a tokenized payment source (ct_ or ca_ prefixed token). Raw card data is not accepted.".to_string(),
                    connector: "billwerk",
                    context: IntegrationErrorContext {
                        suggested_action: Some("Ensure a payment method token is obtained via PaymentMethodService.Tokenize before initiating a Billwerk payment.".to_string()),
                        doc_url: Some("https://optimize.billwerk.com/reference/create-session".to_string()),
                        additional_context: Some("Billwerk requires a tokenized payment source (ct_ or ca_ prefixed token) in the source field. Raw card data is not accepted.".to_string()),
                    },
                }
                .into())
            }
        };
        let recurring = if item.router_data.request.setup_future_usage.is_some() {
            Some(true)
        } else {
            None
        };
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
            settle: item.router_data.request.is_auto_capture(),
            recurring,
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
        let mandate_reference = response.recurring_payment_method.as_ref().map(|rpm| {
            Box::new(MandateReference {
                connector_mandate_id: Some(rpm.clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })
        });
        let payments_response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.handle.clone()),
            redirection_data: None,
            mandate_reference,
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
    type Error = error_stack::Report<IntegrationError>;
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
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
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
    type Error = error_stack::Report<IntegrationError>;

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
    type Error = error_stack::Report<IntegrationError>;
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

// SetupMandate (CIT) request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BillwerkRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BillwerkSetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BillwerkRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let source = match &item.router_data.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => t.token.clone(),
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Billwerk authorize only accepts a tokenized payment source (ct_ or ca_ prefixed token). Raw card data is not accepted.".to_string(),
                    connector: "billwerk",
                    context: IntegrationErrorContext {
                        suggested_action: Some("Ensure a payment method token is obtained via PaymentMethodService.Tokenize before initiating a Billwerk payment.".to_string()),
                        doc_url: Some("https://optimize.billwerk.com/reference/create-session".to_string()),
                        additional_context: Some("Billwerk requires a tokenized payment source (ct_ or ca_ prefixed token) in the source field. Raw card data is not accepted.".to_string()),
                    },
                }
                .into())
            }
        };
        let amount = item
            .router_data
            .request
            .minor_amount
            .unwrap_or(MinorUnit::new(0));
        Ok(Self {
            handle: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
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
            settle: false,
            recurring: true,
        })
    }
}

// RepeatPayment (MIT) request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BillwerkRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BillwerkRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BillwerkRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract the stored card reference (ca_...) from mandate
        let source = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ids) => {
                let connector_mandate_id = connector_mandate_ids.get_connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    },
                )?;
                Secret::new(connector_mandate_id)
            }
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotImplemented(
                    ("Network mandate ID is not supported for Billwerk").into(),
                    Default::default(),
                )
                .into())
            }
        };

        Ok(Self {
            handle: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: router_data.request.minor_amount,
            source,
            currency: router_data.request.currency,
            customer_handle: router_data
                .resource_common_data
                .customer_id
                .as_ref()
                .map(|id| id.get_string_repr().to_owned()),
            settle: router_data.request.is_auto_capture(),
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Billwerk checkout session for client-side SDK initialization.
/// The session id is returned to the frontend for Billwerk Checkout window initialization.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct BillwerkClientAuthRequest {
    pub order: BillwerkSessionOrder,
    pub accept_url: Option<String>,
    pub cancel_url: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct BillwerkSessionOrder {
    pub handle: String,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub customer: BillwerkSessionCustomer,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct BillwerkSessionCustomer {
    pub handle: String,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BillwerkRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BillwerkClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: BillwerkRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let handle = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let return_url = router_data
            .resource_common_data
            .return_url
            .clone()
            .unwrap_or_else(|| "https://hyperswitch.io".to_string());

        let customer_handle = router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_owned())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "customer_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Provide a `customer_id` when creating the client authentication \
                             token. Billwerk uses it as the customer handle for the checkout \
                             session."
                                .to_owned(),
                        ),
                        doc_url: Some(
                            "https://optimize.billwerk.com/reference/create-session".to_owned(),
                        ),
                        additional_context: Some(
                            "Billwerk checkout sessions require a customer handle to associate \
                             the session with a customer record."
                                .to_owned(),
                        ),
                    },
                })
            })?;

        let customer = BillwerkSessionCustomer {
            handle: customer_handle,
            email: router_data
                .request
                .email
                .as_ref()
                .map(|e| e.peek().to_string()),
            first_name: router_data
                .request
                .customer_name
                .as_ref()
                .map(|n| n.peek().to_string()),
            last_name: None,
        };

        Ok(Self {
            order: BillwerkSessionOrder {
                handle,
                amount: router_data.request.amount,
                currency: router_data.request.currency,
                customer,
            },
            accept_url: Some(return_url.clone()),
            cancel_url: Some(return_url),
        })
    }
}

/// Billwerk checkout session response containing the session id for SDK initialization.
#[derive(Debug, Deserialize, Serialize)]
pub struct BillwerkClientAuthResponse {
    pub id: String,
    pub url: Option<String>,
}

impl TryFrom<ResponseRouterData<BillwerkClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BillwerkClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Billwerk(
                BillwerkClientAuthenticationResponseDomain {
                    session_id: response.id,
                },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
