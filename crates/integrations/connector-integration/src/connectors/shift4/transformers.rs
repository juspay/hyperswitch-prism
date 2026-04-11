use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{pii, request::Method, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PSync, RSync, Refund, RepeatPayment,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateReferenceId, PaymentFlowData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId,
    },
    payment_method_data::{
        BankRedirectData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

// Import the connector's RouterData wrapper type created by the macro
use super::Shift4RouterData;
use domain_types::errors::{ConnectorError, IntegrationError};

#[derive(Debug, Clone)]
pub struct Shift4AuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for Shift4AuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Shift4 { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shift4ErrorResponse {
    pub error: ApiErrorResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub code: Option<String>,
    pub message: String,
}

// ===== CREATE CUSTOMER FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CreateCustomerRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4CreateCustomerResponse {
    pub id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for Shift4CreateCustomerRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            email: item.router_data.request.email.clone().expose_option(),
            description: item.router_data.request.description.clone(),
        })
    }
}

impl<F, T> TryFrom<ResponseRouterData<Shift4CreateCustomerResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ConnectorCustomerResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4CreateCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: item.response.id,
            }),
            ..item.router_data
        })
    }
}

// ===== AUTHORIZE FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4PaymentsRequest<T: PaymentMethodDataTypes> {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(flatten)]
    pub payment_method: Shift4PaymentMethod<T>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Shift4PaymentMethod<T: PaymentMethodDataTypes> {
    Card(Shift4CardPayment<T>),
    BankRedirect(Shift4BankRedirectPayment),
    Crypto(Shift4CryptoPayment),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CardPayment<T: PaymentMethodDataTypes> {
    pub card: Shift4CardData<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CardData<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub cardholder_name: Secret<String>,
}

// BankRedirect Payment Structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4BankRedirectPayment {
    pub payment_method: Shift4BankRedirectMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<Shift4FlowRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4FlowRequest {
    pub return_url: String,
}

#[derive(Debug, Serialize)]
pub struct Shift4BankRedirectMethod {
    #[serde(rename = "type")]
    pub payment_type: String,
    pub billing: Shift4Billing,
}

#[derive(Debug, Serialize)]
pub struct Shift4Billing {
    pub name: Option<Secret<String>>,
    pub email: Option<pii::Email>,
    pub address: Option<Shift4Address>,
}

#[derive(Debug, Serialize)]
pub struct Shift4Address {
    pub line1: Option<Secret<String>>,
    pub line2: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country: Option<String>,
}

// Crypto Payment Structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CryptoPayment {
    pub payment_method: Shift4CryptoMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<Shift4FlowRequest>,
}

#[derive(Debug, Serialize)]
pub struct Shift4CryptoMethod {
    #[serde(rename = "type")]
    pub payment_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<Shift4CryptoBilling>,
}

#[derive(Debug, Serialize)]
pub struct Shift4CryptoBilling {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<pii::Email>,
}

// Crypto Payment Structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CryptoPayment {
    pub payment_method: Shift4CryptoMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<Shift4FlowRequest>,
}

#[derive(Debug, Serialize)]
pub struct Shift4CryptoMethod {
    #[serde(rename = "type")]
    pub payment_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<Shift4Billing>,
}

// BankRedirect Data Transformation
impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for Shift4BankRedirectMethod
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_type = match &router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(bank_redirect_data) => match bank_redirect_data {
                BankRedirectData::Ideal { .. } => "ideal",
                BankRedirectData::Eps { .. } => "eps",
                _ => {
                    return Err(error_stack::report!(IntegrationError::NotSupported {
                        message: format!(
                            "BankRedirect type {:?} is not supported by Shift4",
                            bank_redirect_data
                        ),
                        connector: "Shift4",
                        context: Default::default()
                    }))
                }
            },
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Non-bank redirect payment method".to_string(),
                    connector: "Shift4",
                    context: Default::default()
                }))
            }
        };

        // Extract billing information
        let billing = router_data
            .resource_common_data
            .address
            .get_payment_method_billing();
        let name = billing.as_ref().and_then(|b| b.get_optional_full_name());

        // Extract email from request - prioritize from payment data, fallback to address
        let email = router_data
            .request
            .email
            .as_ref()
            .cloned()
            .or_else(|| billing.as_ref().and_then(|b| b.email.as_ref()).cloned());

        let address = billing
            .as_ref()
            .and_then(|b| b.address.as_ref())
            .map(|addr| Shift4Address {
                line1: addr.line1.clone(),
                line2: addr.line2.clone(),
                city: addr.city.clone(),
                state: addr.state.clone(),
                zip: addr.zip.clone(),
                country: addr.country.as_ref().map(|c| c.to_string()),
            });

        let billing_info = Shift4Billing {
            name,
            email,
            address,
        };

        Ok(Self {
            payment_type: payment_type.to_string(),
            billing: billing_info,
        })
    }
}

// Crypto Data Transformation
impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for Shift4CryptoMethod
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract billing information
        let billing = router_data
            .resource_common_data
            .address
            .get_payment_method_billing();
        let name = billing.as_ref().and_then(|b| b.get_optional_full_name());

        // Extract email from request - prioritize from payment data, fallback to address
        let email = router_data
            .request
            .email
            .as_ref()
            .cloned()
            .or_else(|| billing.as_ref().and_then(|b| b.email.as_ref()).cloned());

        let billing_info = if name.is_some() || email.is_some() {
            Some(Shift4Billing {
                name,
                email,
                address: None,
            })
        } else {
            None
        };

        Ok(Self {
            payment_type: "bitpay".to_string(),
            billing: billing_info,
        })
    }
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for Shift4PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let captured = item.request.is_auto_capture();

        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Get cardholder name from address/billing info if available
                let cardholder_name = item
                    .resource_common_data
                    .address
                    .get_payment_method_billing()
                    .and_then(|billing| billing.get_optional_full_name())
                    .or_else(|| {
                        item.request
                            .customer_name
                            .as_ref()
                            .map(|name| Secret::new(name.clone()))
                    })
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "billing_address.first_name",
                            context: Default::default()
                        })
                    })?;

                Shift4PaymentMethod::Card(Shift4CardPayment {
                    card: Shift4CardData {
                        number: card_data.card_number.clone(),
                        exp_month: card_data.card_exp_month.clone(),
                        exp_year: card_data.card_exp_year.clone(),
                        cardholder_name,
                    },
                })
            }
            PaymentMethodData::BankRedirect(_bank_redirect_data) => {
                let bank_redirect_method = Shift4BankRedirectMethod::try_from(item)?;
                let return_url = item.request.get_router_return_url().change_context(
                    IntegrationError::MissingRequiredField {
                        field_name: "return_url",
                        context: Default::default(),
                    },
                )?;

                Shift4PaymentMethod::BankRedirect(Shift4BankRedirectPayment {
                    payment_method: bank_redirect_method,
                    flow: Some(Shift4FlowRequest { return_url }),
                })
            }
            PaymentMethodData::Crypto(_crypto_data) => {
                let crypto_method = Shift4CryptoMethod::try_from(item)?;
                let return_url = item.request.get_router_return_url().change_context(
                    IntegrationError::MissingRequiredField {
                        field_name: "return_url",
                        context: Default::default(),
                    },
                )?;

                Shift4PaymentMethod::Crypto(Shift4CryptoPayment {
                    payment_method: crypto_method,
                    flow: Some(Shift4FlowRequest { return_url }),
                })
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Payment method".to_string(),
                    connector: "Shift4",
                    context: Default::default()
                }))
            }
        };

        Ok(Self {
            amount: item.request.minor_amount,
            currency: item.request.currency,
            captured,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            payment_method,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4PaymentsResponse {
    pub id: String,
    pub currency: Currency,
    pub amount: MinorUnit,
    pub status: Shift4PaymentStatus,
    pub captured: bool,
    pub refunded: bool,
    pub flow: Option<FlowResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowResponse {
    pub next_action: Option<NextAction>,
    pub redirect: Option<RedirectResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectResponse {
    pub redirect_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NextAction {
    Redirect,
    Wait,
    None,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shift4PaymentStatus {
    Successful,
    Pending,
    Failed,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Match Hyperswitch status mapping logic exactly
        let status = match item.response.status {
            Shift4PaymentStatus::Successful => {
                if item.response.captured {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            Shift4PaymentStatus::Failed => AttemptStatus::Failure,
            Shift4PaymentStatus::Pending => {
                match item
                    .response
                    .flow
                    .as_ref()
                    .and_then(|flow| flow.next_action.as_ref())
                {
                    Some(NextAction::Redirect) => AttemptStatus::AuthenticationPending,
                    Some(NextAction::Wait) | Some(NextAction::None) | None => {
                        AttemptStatus::Pending
                    }
                }
            }
        };

        // Extract redirect URL from flow if present
        let redirection_data = item
            .response
            .flow
            .as_ref()
            .and_then(|flow| flow.redirect.as_ref())
            .and_then(|redirect| {
                Url::parse(&redirect.redirect_url)
                    .ok()
                    .map(|url| Box::new(RedirectForm::from((url, Method::Get))))
            });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
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

// PSync response transformation - reuses Shift4PaymentsResponse and status mapping logic
impl TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Match Hyperswitch status mapping logic exactly
        let status = match item.response.status {
            Shift4PaymentStatus::Successful => {
                if item.response.captured {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            Shift4PaymentStatus::Failed => AttemptStatus::Failure,
            Shift4PaymentStatus::Pending => {
                match item
                    .response
                    .flow
                    .as_ref()
                    .and_then(|flow| flow.next_action.as_ref())
                {
                    Some(NextAction::Redirect) => AttemptStatus::AuthenticationPending,
                    Some(NextAction::Wait) | Some(NextAction::None) | None => {
                        AttemptStatus::Pending
                    }
                }
            }
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
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

// Capture response transformation - reuses Shift4PaymentsResponse
impl TryFrom<ResponseRouterData<Shift4PaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Match Hyperswitch status mapping logic exactly
        let status = match item.response.status {
            Shift4PaymentStatus::Successful => {
                if item.response.captured {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            Shift4PaymentStatus::Failed => AttemptStatus::Failure,
            Shift4PaymentStatus::Pending => {
                match item
                    .response
                    .flow
                    .as_ref()
                    .and_then(|flow| flow.next_action.as_ref())
                {
                    Some(NextAction::Redirect) => AttemptStatus::AuthenticationPending,
                    Some(NextAction::Wait) | Some(NextAction::None) | None => {
                        AttemptStatus::Pending
                    }
                }
            }
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
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

// ===== REFUND FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4RefundRequest {
    pub charge_id: String,
    pub amount: MinorUnit,
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for Shift4RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            charge_id: item.request.connector_transaction_id.clone(),
            amount: item.request.minor_refund_amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4RefundResponse {
    pub id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    pub charge: String,
    pub status: Shift4RefundStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shift4RefundStatus {
    Successful,
    Failed,
    Processing,
}

impl TryFrom<ResponseRouterData<Shift4RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<Shift4RefundResponse, Self>) -> Result<Self, Self::Error> {
        // CRITICAL: Explicitly check the status field from the response
        // Do NOT assume success based solely on HTTP 200 response
        let refund_status = match item.response.status {
            Shift4RefundStatus::Successful => RefundStatus::Success,
            Shift4RefundStatus::Failed => RefundStatus::Failure,
            Shift4RefundStatus::Processing => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// RSync (Refund Sync) response transformation - reuses Shift4RefundResponse
impl TryFrom<ResponseRouterData<Shift4RefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<Shift4RefundResponse, Self>) -> Result<Self, Self::Error> {
        // CRITICAL: Explicitly check the status field from the response
        // Do NOT assume success based solely on HTTP 200 response
        let refund_status = match item.response.status {
            Shift4RefundStatus::Successful => RefundStatus::Success,
            Shift4RefundStatus::Failed => RefundStatus::Failure,
            Shift4RefundStatus::Processing => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== SYNC REQUEST STRUCTURES =====
// Sync operations (GET requests) typically don't send a body, but we need these for the macro

#[derive(Debug, Serialize, Default)]
pub struct Shift4PSyncRequest {}

#[derive(Debug, Serialize, Default)]
pub struct Shift4RSyncRequest {}

// ===== MACRO-COMPATIBLE TRYFROM IMPLEMENTATIONS =====
// The macro creates a Shift4RouterData wrapper type. We need TryFrom implementations
// that work with this wrapper.

// PSync Request - converts from Shift4RouterData to empty request struct
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for Shift4PSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: Shift4RouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

// RSync Request - converts from Shift4RouterData to empty request struct
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for Shift4RSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: Shift4RouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

// Authorize Request - delegates to existing implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Shift4PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom<&RouterDataV2> implementation
        Self::try_from(&item.router_data)
    }
}

// Capture Request - we need a separate request type
#[derive(Debug, Serialize)]
pub struct Shift4CaptureRequest {
    // Shift4 capture is done via POST to /charges/{id}/capture with no body
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for Shift4CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: Shift4RouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

// Refund Request - delegates to existing implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for Shift4RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom<&RouterDataV2> implementation
        Self::try_from(&item.router_data)
    }
}

// ===== REPEAT PAYMENT (MIT) FLOW STRUCTURES =====

/// Shift4 MIT request - supports both stored card token and raw card details
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4RepeatPaymentRequest<T: PaymentMethodDataTypes> {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub captured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Card: either a token string ("card_xxx") or raw card details object
    pub card: Shift4RepeatPaymentCard<T>,
    /// Transaction type: "merchant_initiated", "subsequent_recurring", etc.
    #[serde(rename = "type")]
    pub transaction_type: Shift4TransactionType,
    /// Customer ID required when charging a stored card (not needed for raw card)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
}

/// Card field for MIT: either a stored card token or raw card details
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Shift4RepeatPaymentCard<T: PaymentMethodDataTypes> {
    /// Stored card identifier (e.g., "card_xxx")
    Token(String),
    /// Raw card details for approach 3 MIT
    RawCard(Shift4CardData<T>),
}

/// Shift4 transaction type for MIT/recurring classification
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Shift4TransactionType {
    MerchantInitiated,
    SubsequentRecurring,
}

/// MIT response reuses the standard payments response
pub type Shift4RepeatPaymentResponse = Shift4PaymentsResponse;

// ===== REPEAT PAYMENT (MIT) REQUEST TRANSFORMATION =====

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
    > for Shift4RepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // Determine card: use raw card data if available, otherwise use stored card token
        let (card, customer_id) =
            if let PaymentMethodData::Card(card_data) = &item.request.payment_method_data {
                // Approach 3: Raw card details for MIT (no customer needed)
                let cardholder_name = item
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .unwrap_or_else(|| Secret::new("".to_string()));

                (
                    Shift4RepeatPaymentCard::RawCard(Shift4CardData {
                        number: card_data.card_number.clone(),
                        exp_month: card_data.card_exp_month.clone(),
                        exp_year: card_data.card_exp_year.clone(),
                        cardholder_name,
                    }),
                    None, // No customer needed for raw card
                )
            } else {
                // Stored card token approach: extract from mandate_reference
                let token = match &item.request.mandate_reference {
                    MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
                        connector_mandate_ref
                            .get_connector_mandate_id()
                            .ok_or_else(|| {
                                error_stack::report!(IntegrationError::MissingRequiredField {
                                    field_name: "connector_mandate_id (card token)",
                                    context: Default::default(),
                                })
                            })?
                    }
                    MandateReferenceId::NetworkMandateId(_) => {
                        return Err(error_stack::report!(IntegrationError::NotImplemented(
                            "NetworkMandateId is not supported for Shift4 MIT".to_string(),
                            Default::default(),
                        )));
                    }
                    MandateReferenceId::NetworkTokenWithNTI(_) => {
                        return Err(error_stack::report!(IntegrationError::NotImplemented(
                            "NetworkTokenWithNTI is not supported for Shift4 MIT".to_string(),
                            Default::default(),
                        )));
                    }
                };
                (
                    Shift4RepeatPaymentCard::Token(token),
                    item.resource_common_data.connector_customer.clone(),
                )
            };

        // Determine Shift4 transaction type based on MIT category
        let transaction_type = match item.request.mit_category {
            Some(common_enums::MitCategory::Recurring) => {
                Shift4TransactionType::SubsequentRecurring
            }
            _ => Shift4TransactionType::MerchantInitiated,
        };

        let captured = item.request.is_auto_capture();

        Ok(Self {
            amount: item.request.minor_amount,
            currency: item.request.currency,
            captured,
            description: item.resource_common_data.description.clone(),
            metadata: item.request.metadata.clone().expose_option(),
            card,
            transaction_type,
            customer_id,
        })
    }
}

// RepeatPayment Request - converts from Shift4RouterData wrapper
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Shift4RepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom<&RouterDataV2> implementation
        Self::try_from(&item.router_data)
    }
}

// RepeatPayment Response transformation - reuses standard payments response mapping
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<Shift4RepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Shift4RepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Reuse the same status mapping logic as Authorize flow
        let status = match item.response.status {
            Shift4PaymentStatus::Successful => {
                if item.response.captured {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            Shift4PaymentStatus::Failed => AttemptStatus::Failure,
            Shift4PaymentStatus::Pending => {
                match item
                    .response
                    .flow
                    .as_ref()
                    .and_then(|flow| flow.next_action.as_ref())
                {
                    Some(NextAction::Redirect) => AttemptStatus::AuthenticationPending,
                    Some(NextAction::Wait) | Some(NextAction::None) | None => {
                        AttemptStatus::Pending
                    }
                }
            }
        };

        // Extract redirect URL from flow if present
        let redirection_data = item
            .response
            .flow
            .as_ref()
            .and_then(|flow| flow.redirect.as_ref())
            .and_then(|redirect| {
                Url::parse(&redirect.redirect_url)
                    .ok()
                    .map(|url| Box::new(RedirectForm::from((url, Method::Get))))
            });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
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
