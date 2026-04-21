use crate::{types::ResponseRouterData, utils::is_refund_failure};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{pii::Email, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        BillingDescriptor, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        Card, CardDetailsForNetworkTransactionId, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Iso8601, PrimitiveDateTime};

#[derive(Debug, Clone)]
pub struct Revolv3AuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for Revolv3AuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Revolv3 { api_key, .. } => Ok(Self {
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

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Revolv3PaymentsRequest<T: PaymentMethodDataTypes> {
    Sale(Revolv3SaleRequest<T>),
    Authorize(Revolv3AuthorizeRequest<T>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3AuthorizeRequest<T: PaymentMethodDataTypes> {
    pub payment_method: Revolv3PaymentMethodData<T>,
    pub amount: Revolv3AmountData,
    pub three_ds: Option<Revolv3ThreeDSData>,
    pub network_processing: Option<NetworkProcessingData>,
    pub order_processing_channel: Option<OrderProcessingChannelType>,
    pub dynamic_descriptor: Option<Revolv3DynamicDescriptor>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3SaleRequest<T: PaymentMethodDataTypes> {
    pub payment_method: Revolv3PaymentMethodData<T>,
    pub invoice: Revolv3InvoiceData,
    pub three_ds: Option<Revolv3ThreeDSData>,
    pub network_processing: Option<NetworkProcessingData>,
    pub dynamic_descriptor: Option<Revolv3DynamicDescriptor>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3DynamicDescriptor {
    pub sub_merchant_id: Option<String>,
    pub sub_merchant_name: Option<Secret<String>>,
    pub sub_merchant_phone: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3ThreeDSData {
    pub cavv: Secret<String>,
    pub xid: Option<String>,
    pub ds_transaction_id: Option<String>,
    pub three_ds_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkProcessingData {
    pub processing_type: Option<PaymentProcessingType>,
    pub original_network_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PaymentProcessingType {
    InitialRecurring,
    Recurring,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3InvoiceData {
    pub merchant_invoice_ref_id: Option<String>,
    pub amount: Revolv3AmountData,
    pub order_processing_channel: Option<OrderProcessingChannelType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum OrderProcessingChannelType {
    Ecommerce,
    Moto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3AmountData {
    pub value: FloatMajorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Revolv3PaymentMethodData<T: PaymentMethodDataTypes> {
    CreditCard(CreditCardPaymentMethodData<T>),
    Ntid(NtidCreditCardPaymentMethodData),
    MandatePayment,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3BillingAddress {
    address_line1: Option<Secret<String>>,
    address_line2: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    postal_code: Option<Secret<String>>,
    phone_number: Option<Secret<String>>,
    email: Option<Email>,
    country: Option<common_enums::CountryAlpha2>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NtidCreditCardPaymentMethodData {
    billing_address: Option<Revolv3BillingAddress>,
    billing_first_name: Option<Secret<String>>,
    billing_last_name: Option<Secret<String>>,
    billing_full_name: Secret<String>,
    credit_card: Revolv3NtidCreditCardData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3NtidCreditCardData {
    payment_account_number: cards::CardNumber,
    expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardPaymentMethodData<T: PaymentMethodDataTypes> {
    billing_address: Option<Revolv3BillingAddress>,
    billing_first_name: Option<Secret<String>>,
    billing_last_name: Option<Secret<String>>,
    billing_full_name: Secret<String>,
    credit_card: Revolv3CreditCardData<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3CreditCardData<T: PaymentMethodDataTypes> {
    payment_account_number: RawCardNumber<T>,
    expiration_date: Secret<String>,
    security_code: Secret<String>,
}

impl Revolv3BillingAddress {
    fn try_from_payment_flow_data(common_data: &PaymentFlowData) -> Option<Self> {
        let email = common_data.get_optional_billing_email();
        let phone_number = common_data.get_optional_billing_phone_number();

        if common_data.get_optional_billing().is_some() || email.is_some() || phone_number.is_some()
        {
            Some(Self {
                address_line1: common_data.get_optional_billing_line1(),
                address_line2: common_data.get_optional_billing_line2(),
                city: common_data.get_optional_billing_city(),
                state: common_data.get_optional_billing_state(),
                postal_code: common_data.get_optional_billing_zip(),
                phone_number: common_data.get_optional_billing_phone_number(),
                email: common_data.get_optional_billing_email(),
                country: common_data.get_optional_billing_country(),
            })
        } else {
            None
        }
    }
}

pub struct PaymentMethodSpecificRequest<T: PaymentMethodDataTypes> {
    pub payment_method_data: Revolv3PaymentMethodData<T>,
    pub network_data: Option<NetworkProcessingData>,
}

impl<T: PaymentMethodDataTypes> PaymentMethodSpecificRequest<T> {
    pub fn set_credit_card_data(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        card: Card<T>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let common_data = &item.resource_common_data;
        let credit_card_data = CreditCardPaymentMethodData {
            billing_address: Revolv3BillingAddress::try_from_payment_flow_data(common_data),
            billing_first_name: common_data.get_optional_billing_first_name(),
            billing_last_name: common_data.get_optional_billing_last_name(),
            billing_full_name: common_data
                .get_billing_full_name()
                .ok()
                .or(card.card_holder_name.clone())
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_data.billing.address.first_name",
                    context: Default::default(),
                })?,
            credit_card: Revolv3CreditCardData {
                payment_account_number: card.card_number.clone(),
                expiration_date: card.get_expiry_date_as_mmyy()?,
                security_code: card.card_cvc.clone(),
            },
        };
        let network_data = item
            .request
            .is_mandate_payment()
            .then_some(NetworkProcessingData {
                processing_type: Some(PaymentProcessingType::InitialRecurring),
                original_network_transaction_id: None,
            });

        Ok(Self {
            payment_method_data: Revolv3PaymentMethodData::CreditCard(credit_card_data),
            network_data,
        })
    }
}

impl From<common_enums::PaymentChannel> for OrderProcessingChannelType {
    fn from(item: common_enums::PaymentChannel) -> Self {
        match item {
            common_enums::PaymentChannel::Ecommerce => Self::Ecommerce,
            common_enums::PaymentChannel::MailOrder
            | common_enums::PaymentChannel::TelephoneOrder => Self::Moto,
        }
    }
}

impl From<BillingDescriptor> for Revolv3DynamicDescriptor {
    fn from(item: BillingDescriptor) -> Self {
        Self {
            sub_merchant_id: item.reference.clone(),
            sub_merchant_name: item.name.clone(),
            sub_merchant_phone: item.phone.clone(),
            city: item.city.clone(),
        }
    }
}

impl TryFrom<&AuthenticationData> for Revolv3ThreeDSData {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: &AuthenticationData) -> Result<Self, Self::Error> {
        Ok(Self {
            cavv: item
                .cavv
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "authentication_data.cavv",
                    context: Default::default(),
                })?,
            xid: item.acs_transaction_id.clone(),
            ds_transaction_id: item.ds_trans_id.clone(),
            three_ds_version: item
                .message_version
                .clone()
                .map(|version| version.to_string()),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::Revolv3RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Revolv3PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::Revolv3RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method_specific_response = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref card_data) => {
                PaymentMethodSpecificRequest::set_credit_card_data(
                    &item.router_data,
                    card_data.clone(),
                )?
            }
            _ => Err(IntegrationError::NotImplemented(
                (domain_types::utils::get_unimplemented_payment_method_error_message("revolv3"))
                    .into(),
                Default::default(),
            ))?,
        };

        let three_ds = item
            .router_data
            .request
            .authentication_data
            .as_ref()
            .map(Revolv3ThreeDSData::try_from)
            .transpose()?;

        let amount = Revolv3AmountData {
            value: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.minor_amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
            currency: item.router_data.request.currency,
        };

        let dynamic_descriptor = item
            .router_data
            .request
            .billing_descriptor
            .clone()
            .map(Revolv3DynamicDescriptor::from);

        let order_processing_channel = item
            .router_data
            .request
            .payment_channel
            .clone()
            .map(OrderProcessingChannelType::from);

        if item.router_data.request.is_auto_capture() {
            let invoice = Revolv3InvoiceData {
                merchant_invoice_ref_id: item.router_data.request.merchant_order_id.clone(),
                amount,
                order_processing_channel,
            };

            Ok(Self::Sale(Revolv3SaleRequest {
                payment_method: payment_method_specific_response.payment_method_data,
                invoice,
                three_ds,
                network_processing: payment_method_specific_response.network_data,
                dynamic_descriptor,
            }))
        } else {
            Ok(Self::Authorize(Revolv3AuthorizeRequest {
                payment_method: payment_method_specific_response.payment_method_data,
                amount,
                three_ds,
                network_processing: payment_method_specific_response.network_data,
                dynamic_descriptor,
                order_processing_channel,
            }))
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Revolv3PaymentsResponse {
    Sale(Revolv3SaleResponse),
    Authorize(Revolv3AuthorizeResponse),
}

// Note: An authorization request does not create an invoice
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3AuthorizeResponse {
    pub network_transaction_id: Option<String>,
    pub payment_method_authorization_id: Option<i64>,
    pub payment_method: Option<Revolv3PaymentMethodResponse>,
    pub payment_processor: Option<String>,
    pub response_message: Option<String>,
    pub response_code: Option<String>,
    pub processor_transaction_id: Option<String>,
    pub auth_code: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3SaleResponse {
    pub invoice_id: i64,
    pub merchant_invoice_ref_id: Option<String>,
    pub network_transaction_id: Option<String>,
    pub invoice_status: InvoiceStatus,
    pub payment_method_id: Option<i64>,
    pub payment_processor: Option<String>,
    pub response_message: Option<String>,
    pub response_code: Option<String>,
    pub processor_transaction_id: Option<String>,
    pub auth_code: Option<String>,
}

pub struct DerivedPaymentResponse {
    pub status: AttemptStatus,
    pub response: Result<PaymentsResponseData, domain_types::router_data::ErrorResponse>,
}

impl Revolv3SaleResponse {
    pub fn get_transaction_response(
        &self,
        status_code: u16,
    ) -> Result<DerivedPaymentResponse, error_stack::Report<ConnectorError>> {
        let status = AttemptStatus::from(&self.invoice_status);
        let response = if domain_types::utils::is_payment_failure(status) {
            Err(domain_types::router_data::ErrorResponse {
                code: self
                    .response_code
                    .clone()
                    .unwrap_or(common_utils::consts::NO_ERROR_CODE.to_string()),
                message: self
                    .response_message
                    .clone()
                    .unwrap_or(common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                reason: self.response_message.clone(),
                status_code,
                attempt_status: None,
                connector_transaction_id: Some(self.invoice_id.to_string()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            let mandate_reference = self.payment_method_id.as_ref().map(|connector_mandate_id| {
                domain_types::connector_types::MandateReference {
                    connector_mandate_id: Some(connector_mandate_id.to_string()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                }
            });

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(self.invoice_id.to_string()),
                redirection_data: None,
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: Some(serde_json::json!(Revolv3OperationMetadata::PsyncAllowed)),
                network_txn_id: self.network_transaction_id.clone(),
                connector_response_reference_id: self.merchant_invoice_ref_id.clone(),
                incremental_authorization_allowed: None,
                status_code,
            })
        };

        Ok(DerivedPaymentResponse { status, response })
    }
}

impl Revolv3AuthorizeResponse {
    pub fn get_transaction_response(
        &self,
        status_code: u16,
        is_setup_mandate: bool,
    ) -> Result<DerivedPaymentResponse, error_stack::Report<ConnectorError>> {
        let mandate_reference = self.payment_method.as_ref().and_then(|pm| {
            pm.payment_method_id.map(|connector_mandate_id| {
                domain_types::connector_types::MandateReference {
                    connector_mandate_id: Some(connector_mandate_id.to_string()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                }
            })
        });
        // Synchronous flow — PSync is not applicable
        match self.payment_method_authorization_id {
            Some(ref payment_method_authorization_id) => Ok(DerivedPaymentResponse {
                status: if is_setup_mandate {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        payment_method_authorization_id.to_string(),
                    ),
                    redirection_data: None,
                    mandate_reference: mandate_reference.map(Box::new),
                    connector_metadata: None,
                    network_txn_id: self.network_transaction_id.clone(),
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code,
                }),
            }),
            _ => Ok(DerivedPaymentResponse {
                status: AttemptStatus::Failure,
                response: Err(domain_types::router_data::ErrorResponse {
                    code: self
                        .response_code
                        .clone()
                        .unwrap_or(common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: self
                        .response_message
                        .clone()
                        .unwrap_or(common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: self.response_message.clone(),
                    status_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
            }),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum InvoiceStatus {
    Paid,
    Pending,
    Noncollectable,
    Failed,
    OneTimePaymentPending,
    RetryPending,
}

impl From<&InvoiceStatus> for AttemptStatus {
    fn from(status: &InvoiceStatus) -> Self {
        match status {
            InvoiceStatus::Paid => Self::Charged,
            InvoiceStatus::Pending
            | InvoiceStatus::OneTimePaymentPending
            | InvoiceStatus::RetryPending => Self::Pending,
            InvoiceStatus::Noncollectable | InvoiceStatus::Failed => Self::Failure,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<Revolv3PaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Revolv3PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let derived_response = match item.response {
            Revolv3PaymentsResponse::Authorize(ref auth_response) => {
                auth_response.get_transaction_response(item.http_code, false)
            }
            Revolv3PaymentsResponse::Sale(ref sale_response) => {
                sale_response.get_transaction_response(item.http_code)
            }
        }?;

        Ok(Self {
            response: derived_response.response,
            resource_common_data: PaymentFlowData {
                status: derived_response.status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3PaymentSyncResponse {
    pub invoice_id: i64,
    pub merchant_invoice_ref_id: Option<String>,
    pub network_transaction_id: Option<String>,
    pub invoice_status: InvoiceStatus,
    pub payment_method: Option<Revolv3PaymentMethodResponse>,
    pub invoice_attempts: Option<Vec<Revolv3InvoiceAttempt>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3PaymentMethodResponse {
    pub payment_method_id: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3InvoiceAttempt {
    pub invoice_attempt_date: String,
    pub response_code: Option<String>,
    pub response_message: Option<String>,
}

fn get_latest_attempt(
    attempts: &Option<Vec<Revolv3InvoiceAttempt>>,
) -> Option<&Revolv3InvoiceAttempt> {
    attempts
        .as_ref()?
        .iter()
        .filter_map(|attempt| {
            PrimitiveDateTime::parse(&attempt.invoice_attempt_date, &Iso8601::DEFAULT)
                .ok()
                .map(|dt| (dt, attempt))
        })
        .max_by_key(|(dt, _)| *dt)
        .map(|(_, attempt)| attempt)
}

impl TryFrom<ResponseRouterData<Revolv3PaymentSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Revolv3PaymentSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.invoice_status);
        let response = if domain_types::utils::is_payment_failure(status) {
            let latest_attempt = get_latest_attempt(&item.response.invoice_attempts);
            let error_message = latest_attempt.and_then(|attempt| attempt.response_message.clone());

            Err(domain_types::router_data::ErrorResponse {
                code: latest_attempt
                    .and_then(|attempt| attempt.response_code.clone())
                    .unwrap_or(common_utils::consts::NO_ERROR_CODE.to_string()),
                message: error_message
                    .clone()
                    .unwrap_or(common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                reason: error_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.invoice_id.to_string()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            let mandate_reference = item.response.payment_method.and_then(|payment_method| {
                payment_method
                    .payment_method_id
                    .map(
                        |connector_mandate_id| domain_types::connector_types::MandateReference {
                            connector_mandate_id: Some(connector_mandate_id.to_string()),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                        },
                    )
            });

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.invoice_id.to_string(),
                ),
                redirection_data: None,
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: None,
                network_txn_id: item.response.network_transaction_id.clone(),
                connector_response_reference_id: item.response.merchant_invoice_ref_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3RefundRequest {
    pub amount: FloatMajorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::Revolv3RouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for Revolv3RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::Revolv3RouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.minor_refund_amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3RefundResponse {
    pub invoice: RefundInvoice,
    pub refunds: Option<Vec<Revolv3InvoiceAttempt>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundInvoice {
    pub invoice_id: i64,
    pub parent_invoice_id: i64,
    pub invoice_status: RefundInvoiceStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum RefundInvoiceStatus {
    Refund,
    PartialRefund,
    RefundPending,
    RefundDeclined,
    RefundFailed,
}

impl From<&RefundInvoiceStatus> for RefundStatus {
    fn from(status: &RefundInvoiceStatus) -> Self {
        match status {
            RefundInvoiceStatus::Refund | RefundInvoiceStatus::PartialRefund => Self::Success,
            RefundInvoiceStatus::RefundPending => Self::Pending,
            RefundInvoiceStatus::RefundDeclined | RefundInvoiceStatus::RefundFailed => {
                Self::Failure
            }
        }
    }
}

impl TryFrom<ResponseRouterData<Revolv3RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Revolv3RefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.invoice.invoice_status);
        let response = if is_refund_failure(refund_status) {
            let latest_attempt = get_latest_attempt(&item.response.refunds);
            let error_message = latest_attempt
                .as_ref()
                .and_then(|attempt| attempt.response_message.clone());
            Err(domain_types::router_data::ErrorResponse {
                code: latest_attempt
                    .as_ref()
                    .and_then(|attempt| attempt.response_code.clone())
                    .unwrap_or(common_utils::consts::NO_ERROR_CODE.to_string()),
                message: error_message
                    .clone()
                    .unwrap_or(common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                reason: error_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.invoice.invoice_id.to_string()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.invoice.invoice_id.to_string(),
                refund_status,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3RefundSyncResponse {
    pub invoice_id: i64,
    pub merchant_invoice_ref_id: Option<String>,
    pub network_transaction_id: Option<String>,
    pub invoice_status: RefundInvoiceStatus,
    pub payment_method: Option<Revolv3PaymentMethodResponse>,
    pub invoice_attempts: Option<Vec<Revolv3InvoiceAttempt>>,
}

impl TryFrom<ResponseRouterData<Revolv3RefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Revolv3RefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.invoice_status);
        let response = if is_refund_failure(refund_status) {
            let latest_attempt = get_latest_attempt(&item.response.invoice_attempts);
            let error_message = latest_attempt
                .as_ref()
                .and_then(|attempt| attempt.response_message.clone());
            Err(domain_types::router_data::ErrorResponse {
                code: latest_attempt
                    .as_ref()
                    .and_then(|attempt| attempt.response_code.clone())
                    .unwrap_or(common_utils::consts::NO_ERROR_CODE.to_string()),
                message: error_message
                    .clone()
                    .unwrap_or(common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                reason: error_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.invoice_id.to_string()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.invoice_id.to_string(),
                refund_status,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3CaptureRequest {
    pub invoice: Revolv3InvoiceData,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::Revolv3RouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for Revolv3CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::Revolv3RouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let invoice = Revolv3InvoiceData {
            merchant_invoice_ref_id: item.router_data.request.merchant_order_id.clone(),
            amount: Revolv3AmountData {
                value: item
                    .connector
                    .amount_converter
                    .convert(
                        item.router_data.request.minor_amount_to_capture,
                        item.router_data.request.currency,
                    )
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: Default::default(),
                    })?,
                currency: item.router_data.request.currency,
            },
            order_processing_channel: None,
        };

        Ok(Self { invoice })
    }
}

impl<F> TryFrom<ResponseRouterData<Revolv3SaleResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(value: ResponseRouterData<Revolv3SaleResponse, Self>) -> Result<Self, Self::Error> {
        let derived_response = value.response.get_transaction_response(value.http_code)?;
        Ok(Self {
            response: derived_response.response,
            resource_common_data: PaymentFlowData {
                status: derived_response.status,
                ..value.router_data.resource_common_data
            },
            ..value.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3AuthReversalRequest {
    pub payment_method_authorization_id: String,
    pub reason: Option<String>,
    pub amount: Option<FloatMajorUnit>,
}

impl<T>
    TryFrom<
        super::Revolv3RouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for Revolv3AuthReversalRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: super::Revolv3RouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method_authorization_id =
            item.router_data.request.connector_transaction_id.clone();
        let reason = item.router_data.request.cancellation_reason.clone();
        let amount = item
            .router_data
            .request
            .amount
            .zip(item.router_data.request.currency)
            .map(|(minor_amount, currency)| {
                item.connector
                    .amount_converter
                    .convert(minor_amount, currency)
            })
            .transpose()
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            payment_method_authorization_id,
            reason,
            amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3AuthReversalResponse {
    pub payment_processor: i32,
    pub reference_number: Option<String>,
    pub message: Option<String>,
}

impl TryFrom<ResponseRouterData<Revolv3AuthReversalResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Revolv3AuthReversalResponse, Self>,
    ) -> Result<Self, Self::Error> {
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
                status: AttemptStatus::Voided,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Revolv3RepeatPaymentRequest<T: PaymentMethodDataTypes> {
    RepeatSale(Revolv3RepeatSaleRequest<T>),
    RepeatAuthorize(Revolv3RepeatAuthorizeRequest<T>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3RepeatSaleRequest<T: PaymentMethodDataTypes> {
    pub payment_method: Revolv3PaymentMethodData<T>,
    pub network_processing: NetworkProcessingData,
    pub invoice: Revolv3InvoiceData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3RepeatAuthorizeRequest<T: PaymentMethodDataTypes> {
    pub payment_method: Revolv3PaymentMethodData<T>,
    pub network_processing: NetworkProcessingData,
    pub amount: Revolv3AmountData,
}

impl<T: PaymentMethodDataTypes> Revolv3PaymentMethodData<T> {
    pub fn set_credit_card_data_for_ntid(
        card: CardDetailsForNetworkTransactionId,
        common_data: &PaymentFlowData,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let credit_card_data = NtidCreditCardPaymentMethodData {
            billing_address: Revolv3BillingAddress::try_from_payment_flow_data(common_data),
            billing_first_name: common_data.get_optional_billing_first_name(),
            billing_last_name: common_data.get_optional_billing_last_name(),
            billing_full_name: common_data
                .get_billing_full_name()
                .ok()
                .or(card.card_holder_name.clone())
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_data.billing.address.first_name",
                    context: Default::default(),
                })?,
            credit_card: Revolv3NtidCreditCardData {
                payment_account_number: card.card_number.clone(),
                expiration_date: card.get_expiry_date_as_mmyy()?,
            },
        };
        Ok(Self::Ntid(credit_card_data))
    }

    pub fn set_mandate_data() -> Result<Self, error_stack::Report<IntegrationError>> {
        Ok(Self::MandatePayment)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::Revolv3RouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Revolv3RepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: super::Revolv3RouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let network_processing = NetworkProcessingData {
            processing_type: Some(PaymentProcessingType::Recurring),
            original_network_transaction_id: item.router_data.request.get_network_mandate_id(),
        };

        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::CardDetailsForNetworkTransactionId(ref card_data) => {
                if item.router_data.resource_common_data.is_three_ds() {
                    Err(IntegrationError::NotSupported {
                        message: "Cards No3DS".to_string(),
                        connector: "revolv3",
                        context: Default::default(),
                    })?
                };
                Revolv3PaymentMethodData::set_credit_card_data_for_ntid(
                    card_data.clone(),
                    &item.router_data.resource_common_data,
                )?
            }
            PaymentMethodData::MandatePayment => Revolv3PaymentMethodData::set_mandate_data()?,
            _ => Err(IntegrationError::NotImplemented(
                (domain_types::utils::get_unimplemented_payment_method_error_message("revolv3"))
                    .into(),
                Default::default(),
            ))?,
        };

        let amount = Revolv3AmountData {
            value: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.minor_amount,
                    item.router_data.request.currency,
                )
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
            currency: item.router_data.request.currency,
        };

        if item.router_data.request.is_auto_capture() {
            Ok(Self::RepeatSale(Revolv3RepeatSaleRequest {
                payment_method,
                network_processing,
                invoice: Revolv3InvoiceData {
                    merchant_invoice_ref_id: item.router_data.request.merchant_order_id.clone(),
                    amount,
                    order_processing_channel: None,
                },
            }))
        } else {
            Ok(Self::RepeatAuthorize(Revolv3RepeatAuthorizeRequest {
                payment_method,
                network_processing,
                amount,
            }))
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Revolv3RepeatPaymentResponse {
    Sale(Revolv3SaleResponse),
    Authorize(Revolv3AuthorizeResponse),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<Revolv3RepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Revolv3RepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let derived_response = match item.response {
            Revolv3RepeatPaymentResponse::Authorize(ref auth_response) => {
                auth_response.get_transaction_response(item.http_code, false)
            }
            Revolv3RepeatPaymentResponse::Sale(ref sale_response) => {
                sale_response.get_transaction_response(item.http_code)
            }
        }?;

        Ok(Self {
            response: derived_response.response,
            resource_common_data: PaymentFlowData {
                status: derived_response.status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3SetupMandateRequest<T: PaymentMethodDataTypes> {
    pub payment_method: Revolv3PaymentMethodData<T>,
    pub network_processing: NetworkProcessingData,
    pub amount: Revolv3AmountData,
    pub order_processing_channel: Option<OrderProcessingChannelType>,
    pub dynamic_descriptor: Option<Revolv3DynamicDescriptor>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::Revolv3RouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Revolv3SetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::Revolv3RouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref card_data) => {
                if item.router_data.resource_common_data.is_three_ds() {
                    Err(IntegrationError::NotSupported {
                        message: "Cards No3DS".to_string(),
                        connector: "revolv3",
                        context: Default::default(),
                    })?
                };
                let common_data = &item.router_data.resource_common_data;
                Revolv3PaymentMethodData::CreditCard(CreditCardPaymentMethodData {
                    billing_address: Revolv3BillingAddress::try_from_payment_flow_data(common_data),
                    billing_first_name: common_data.get_optional_billing_first_name(),
                    billing_last_name: common_data.get_optional_billing_last_name(),
                    billing_full_name: common_data
                        .get_billing_full_name()
                        .ok()
                        .or(card_data.card_holder_name.clone())
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "payment_method_data.billing.address.first_name",
                            context: Default::default(),
                        })?,
                    credit_card: Revolv3CreditCardData {
                        payment_account_number: card_data.card_number.clone(),
                        expiration_date: card_data.get_expiry_date_as_mmyy()?,
                        security_code: card_data.card_cvc.clone(),
                    },
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                (domain_types::utils::get_unimplemented_payment_method_error_message("revolv3"))
                    .into(),
                Default::default(),
            ))?,
        };

        let network_processing = NetworkProcessingData {
            processing_type: Some(PaymentProcessingType::InitialRecurring),
            original_network_transaction_id: None,
        };

        let amount = Revolv3AmountData {
            value: FloatMajorUnit::zero(),
            currency: item.router_data.request.currency,
        };

        let order_processing_channel = item
            .router_data
            .request
            .payment_channel
            .map(OrderProcessingChannelType::from);

        let dynamic_descriptor = item
            .router_data
            .request
            .billing_descriptor
            .clone()
            .map(Revolv3DynamicDescriptor::from);

        Ok(Self {
            payment_method,
            network_processing,
            amount,
            order_processing_channel,
            dynamic_descriptor,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<Revolv3AuthorizeResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Revolv3AuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let derived_response = item
            .response
            .get_transaction_response(item.http_code, true)?;

        Ok(Self {
            response: derived_response.response,
            resource_common_data: PaymentFlowData {
                status: derived_response.status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revolv3ErrorResponse {
    pub message: String,
    pub errors: Option<Vec<String>>,
}

/// Authorize and void operations do not support psync.
/// This metadata acts as a flag to determine whether a psync
/// request should be triggered to the connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Revolv3OperationMetadata {
    PsyncAllowed,
}
pub fn validate_psync(
    connector_metadata: &Option<Secret<serde_json::Value>>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    let metadata = connector_metadata
        .clone()
        .map(|metadata| metadata.expose())
        .ok_or_else(|| IntegrationError::NotSupported {
            message: "PSync for authorization/void operations".to_string(),
            connector: "revolv3",
            context: Default::default(),
        })?;

    let operation_metadata: Revolv3OperationMetadata = serde_json::from_value(metadata.clone())
        .map_err(|_| IntegrationError::NotSupported {
            message: "Invalid connector metadata for PSync validation".to_string(),
            connector: "revolv3",
            context: Default::default(),
        })?;

    match operation_metadata {
        Revolv3OperationMetadata::PsyncAllowed => Ok(()),
    }
}
