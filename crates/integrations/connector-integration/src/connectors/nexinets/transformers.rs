use base64::Engine;
use common_enums::{enums, AttemptStatus};
use common_utils::{errors::CustomResult, request::Method};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference, MandateReferenceId,
        NexinetsClientAuthenticationResponse as NexinetsClientAuthenticationResponseDomain,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        ApplePayWalletData, BankRedirectData, Card, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber, WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{connectors::nexinets::NexinetsRouterData, types::ResponseRouterData};
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    initial_amount: i64,
    currency: enums::Currency,
    channel: NexinetsChannel,
    product: NexinetsProduct,
    payment: Option<NexinetsPaymentDetails<T>>,
    #[serde(rename = "async")]
    nexinets_async: NexinetsAsyncDetails,
    merchant_order_id: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexinetsChannel {
    #[default]
    Ecom,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NexinetsProduct {
    #[default]
    Creditcard,
    Paypal,
    Giropay,
    Sofort,
    Eps,
    Ideal,
    Applepay,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum NexinetsPaymentDetails<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(Box<NexiCardDetails<T>>),
    Wallet(Box<NexinetsWalletDetails>),
    BankRedirects(Box<NexinetsBankRedirects>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexiCardDetails<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(flatten)]
    card_data: CardDataDetails<T>,
    cof_contract: Option<CofContract>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CardDataDetails<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    CardDetails(Box<CardDetails<T>>),
    PaymentInstrument(Box<PaymentInstrument>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDetails<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card_number: RawCardNumber<T>,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    verification: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInstrument {
    payment_instrument_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct CofContract {
    #[serde(rename = "type")]
    recurring_type: RecurringType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecurringType {
    Unscheduled,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsBankRedirects {
    bic: Option<NexinetsBIC>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsAsyncDetails {
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
    pub failure_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum NexinetsBIC {
    #[serde(rename = "ABNANL2A")]
    AbnAmro,
    #[serde(rename = "ASNBNL21")]
    AsnBank,
    #[serde(rename = "BUNQNL2A")]
    Bunq,
    #[serde(rename = "INGBNL2A")]
    Ing,
    #[serde(rename = "KNABNL2H")]
    Knab,
    #[serde(rename = "RABONL2U")]
    Rabobank,
    #[serde(rename = "RBRBNL21")]
    Regiobank,
    #[serde(rename = "SNSBNL2A")]
    SnsBank,
    #[serde(rename = "TRIONL2U")]
    TriodosBank,
    #[serde(rename = "FVLBNL22")]
    VanLanschot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NexinetsWalletDetails {
    ApplePayToken(Box<ApplePayDetails>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplePayDetails {
    payment_data: serde_json::Value,
    payment_method: ApplepayPaymentMethod,
    transaction_identifier: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplepayPaymentMethod {
    display_name: String,
    network: String,
    #[serde(rename = "type")]
    token_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexinetsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexinetsPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NexinetsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let return_url = item.router_data.resource_common_data.return_url.clone();
        let nexinets_async = NexinetsAsyncDetails {
            success_url: return_url.clone(),
            cancel_url: return_url.clone(),
            failure_url: return_url,
        };
        let (payment, product) = get_payment_details_and_product(&item.router_data)?;
        let merchant_order_id = match item.router_data.resource_common_data.payment_method {
            // Merchant order id is sent only in case of card payment
            enums::PaymentMethod::Card => Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            _ => None,
        };
        Ok(Self {
            initial_amount: item.router_data.request.amount.get_amount_as_i64(),
            currency: item.router_data.request.currency,
            channel: NexinetsChannel::Ecom,
            product,
            payment,
            nexinets_async,
            merchant_order_id,
        })
    }
}

// Auth Struct
pub struct NexinetsAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NexinetsAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nexinets {
                merchant_id,
                api_key,
                ..
            } => {
                let auth_key = format!("{}:{}", merchant_id.peek(), api_key.peek());
                let auth_header = format!("Basic {}", BASE64_ENGINE.encode(auth_key));
                Ok(Self {
                    api_key: Secret::new(auth_header),
                })
            }
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?,
        }
    }
}
// PaymentsResponse
#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexinetsPaymentStatus {
    Success,
    Pending,
    Ok,
    Failure,
    Declined,
    InProgress,
    Expired,
    Aborted,
}

fn get_status(status: NexinetsPaymentStatus, method: NexinetsTransactionType) -> AttemptStatus {
    match status {
        NexinetsPaymentStatus::Success => match method {
            NexinetsTransactionType::Preauth => AttemptStatus::Authorized,
            NexinetsTransactionType::Debit | NexinetsTransactionType::Capture => {
                AttemptStatus::Charged
            }
            NexinetsTransactionType::Cancel => AttemptStatus::Voided,
        },
        NexinetsPaymentStatus::Declined
        | NexinetsPaymentStatus::Failure
        | NexinetsPaymentStatus::Expired
        | NexinetsPaymentStatus::Aborted => match method {
            NexinetsTransactionType::Preauth => AttemptStatus::AuthorizationFailed,
            NexinetsTransactionType::Debit | NexinetsTransactionType::Capture => {
                AttemptStatus::CaptureFailed
            }
            NexinetsTransactionType::Cancel => AttemptStatus::VoidFailed,
        },
        NexinetsPaymentStatus::Ok => match method {
            NexinetsTransactionType::Preauth => AttemptStatus::Authorized,
            _ => AttemptStatus::Pending,
        },
        NexinetsPaymentStatus::Pending => AttemptStatus::AuthenticationPending,
        NexinetsPaymentStatus::InProgress => AttemptStatus::Pending,
    }
}

impl TryFrom<&enums::BankNames> for NexinetsBIC {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(bank: &enums::BankNames) -> Result<Self, Self::Error> {
        match bank {
            enums::BankNames::AbnAmro => Ok(Self::AbnAmro),
            enums::BankNames::AsnBank => Ok(Self::AsnBank),
            enums::BankNames::Bunq => Ok(Self::Bunq),
            enums::BankNames::Ing => Ok(Self::Ing),
            enums::BankNames::Knab => Ok(Self::Knab),
            enums::BankNames::Rabobank => Ok(Self::Rabobank),
            enums::BankNames::Regiobank => Ok(Self::Regiobank),
            enums::BankNames::SnsBank => Ok(Self::SnsBank),
            enums::BankNames::TriodosBank => Ok(Self::TriodosBank),
            enums::BankNames::VanLanschot => Ok(Self::VanLanschot),
            _ => Err(IntegrationError::FlowNotSupported {
                flow: bank.to_string(),
                connector: "Nexinets".to_string(),
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsPreAuthOrDebitResponse {
    order_id: String,
    transaction_type: NexinetsTransactionType,
    transactions: Vec<NexinetsTransaction>,
    payment_instrument: PaymentInstrument,
    redirect_url: Option<Url>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsTransaction {
    pub transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: NexinetsTransactionType,
    pub currency: enums::Currency,
    pub status: NexinetsPaymentStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexinetsTransactionType {
    Preauth,
    Debit,
    Capture,
    Cancel,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NexinetsPaymentsMetadata {
    pub transaction_id: Option<String>,
    pub order_id: Option<String>,
    pub psync_flow: NexinetsTransactionType,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NexinetsPreAuthOrDebitResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NexinetsPreAuthOrDebitResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let transaction = match item.response.transactions.first() {
            Some(order) => order,
            _ => Err(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "nexinets",
            ))?,
        };
        let nexinets_metadata = NexinetsPaymentsMetadata {
            transaction_id: Some(transaction.transaction_id.clone()),
            order_id: Some(item.response.order_id.clone()),
            psync_flow: item.response.transaction_type.clone(),
        };
        let connector_metadata = serde_json::to_value(&nexinets_metadata).change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "nexinets"),
        )?;

        let redirection_data = item
            .response
            .redirect_url
            .map(|url| RedirectForm::from((url, Method::Get)));
        let resource_id = match item.response.transaction_type.clone() {
            NexinetsTransactionType::Preauth
            | NexinetsTransactionType::Debit
            | NexinetsTransactionType::Capture => {
                ResponseId::ConnectorTransactionId(transaction.transaction_id.clone())
            }
            _ => Err(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "nexinets",
            ))?,
        };
        let mandate_reference = item
            .response
            .payment_instrument
            .payment_instrument_id
            .map(|id| MandateReference {
                connector_mandate_id: Some(id.expose()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            });
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: get_status(transaction.status.clone(), item.response.transaction_type),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: Some(connector_metadata),
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsCaptureOrVoidRequest {
    pub initial_amount: i64,
    pub currency: enums::Currency,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsOrder {
    pub order_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexinetsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NexinetsCaptureOrVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NexinetsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            initial_amount: item.router_data.request.amount_to_capture,
            currency: item.router_data.request.currency,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexinetsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NexinetsCaptureOrVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NexinetsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount =
            item.router_data
                .request
                .amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: Default::default(),
                })?;
        let currency =
            item.router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;
        Ok(Self {
            initial_amount: amount.get_amount_as_i64(),
            currency,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsPaymentResponse {
    pub transaction_id: String,
    pub status: NexinetsPaymentStatus,
    pub order: NexinetsOrder,
    #[serde(rename = "type")]
    pub transaction_type: NexinetsTransactionType,
}

impl<F, T> TryFrom<ResponseRouterData<NexinetsPaymentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NexinetsPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let transaction_id = Some(item.response.transaction_id.clone());

        let connector_metadata = serde_json::to_value(NexinetsPaymentsMetadata {
            transaction_id,
            order_id: Some(item.response.order.order_id.clone()),
            psync_flow: item.response.transaction_type.clone(),
        })
        .change_context(crate::utils::response_handling_fail_for_connector(
            item.http_code,
            "nexinets",
        ))?;
        let resource_id = match item.response.transaction_type.clone() {
            NexinetsTransactionType::Preauth
            | NexinetsTransactionType::Debit
            | NexinetsTransactionType::Capture => {
                ResponseId::ConnectorTransactionId(item.response.transaction_id)
            }
            _ => ResponseId::NoResponseId,
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: get_status(item.response.status, item.response.transaction_type),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(connector_metadata),
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order.order_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// REFUND :
// Type definition for RefundRequest
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsRefundRequest {
    pub initial_amount: i64,
    pub currency: enums::Currency,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexinetsRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for NexinetsRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NexinetsRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            initial_amount: item.router_data.request.refund_amount,
            currency: item.router_data.request.currency,
        })
    }
}

// Type definition for Refund Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsRefundResponse {
    pub transaction_id: String,
    pub status: RefundStatus,
    pub order: NexinetsOrder,
    #[serde(rename = "type")]
    pub transaction_type: RefundType,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefundStatus {
    Success,
    Ok,
    Failure,
    Declined,
    InProgress,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefundType {
    Refund,
}

impl From<RefundStatus> for enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Success => Self::Success,
            RefundStatus::Failure | RefundStatus::Declined => Self::Failure,
            RefundStatus::InProgress | RefundStatus::Ok => Self::Pending,
        }
    }
}

impl<F> TryFrom<ResponseRouterData<NexinetsRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NexinetsRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id,
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<NexinetsRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NexinetsRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id,
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NexinetsErrorResponse {
    pub status: u16,
    pub code: u16,
    pub message: String,
    pub errors: Vec<OrderErrorDetails>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct OrderErrorDetails {
    pub code: u16,
    pub message: String,
    pub field: Option<String>,
}

fn get_payment_details_and_product<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<
    (Option<NexinetsPaymentDetails<T>>, NexinetsProduct),
    error_stack::Report<IntegrationError>,
> {
    match &item.request.payment_method_data {
        PaymentMethodData::Card(card) => Ok((
            Some(get_card_data(item, card)?),
            NexinetsProduct::Creditcard,
        )),
        PaymentMethodData::Wallet(wallet) => Ok(get_wallet_details(wallet)?),
        PaymentMethodData::BankRedirect(bank_redirect) => match bank_redirect {
            BankRedirectData::Eps { .. } => Ok((None, NexinetsProduct::Eps)),
            BankRedirectData::Giropay { .. } => Ok((None, NexinetsProduct::Giropay)),
            BankRedirectData::Ideal { bank_name, .. } => Ok((
                Some(NexinetsPaymentDetails::BankRedirects(Box::new(
                    NexinetsBankRedirects {
                        bic: bank_name
                            .map(|bank_name| NexinetsBIC::try_from(&bank_name))
                            .transpose()?,
                    },
                ))),
                NexinetsProduct::Ideal,
            )),
            BankRedirectData::Sofort { .. } => Ok((None, NexinetsProduct::Sofort)),
            BankRedirectData::BancontactCard { .. }
            | BankRedirectData::Blik { .. }
            | BankRedirectData::Bizum { .. }
            | BankRedirectData::Eft { .. }
            | BankRedirectData::Interac { .. }
            | BankRedirectData::OnlineBankingCzechRepublic { .. }
            | BankRedirectData::OnlineBankingFinland { .. }
            | BankRedirectData::OnlineBankingPoland { .. }
            | BankRedirectData::OnlineBankingSlovakia { .. }
            | BankRedirectData::OpenBankingUk { .. }
            | BankRedirectData::Przelewy24 { .. }
            | BankRedirectData::Trustly { .. }
            | BankRedirectData::OnlineBankingFpx { .. }
            | BankRedirectData::OnlineBankingThailand { .. }
            | BankRedirectData::LocalBankRedirect {}
            | BankRedirectData::OpenBanking {}
            | BankRedirectData::Netbanking { .. } => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("nexinets"),
            ))?,
        },
        PaymentMethodData::CardRedirect(_)
        | PaymentMethodData::PayLater(_)
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
            Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("nexinets"),
            ))?
        }
    }
}

fn get_card_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    card: &Card<T>,
) -> Result<NexinetsPaymentDetails<T>, IntegrationError> {
    let (card_data, cof_contract) = match is_mandate_payment(&item.request) {
        true => {
            let card_data = match item.request.off_session {
                Some(true) => CardDataDetails::PaymentInstrument(Box::new(PaymentInstrument {
                    payment_instrument_id: item.request.connector_mandate_id().map(Secret::new),
                })),
                _ => CardDataDetails::CardDetails(Box::new(get_card_details(card)?)),
            };
            let cof_contract = Some(CofContract {
                recurring_type: RecurringType::Unscheduled,
            });
            (card_data, cof_contract)
        }
        false => (
            CardDataDetails::CardDetails(Box::new(get_card_details(card)?)),
            None,
        ),
    };
    Ok(NexinetsPaymentDetails::Card(Box::new(NexiCardDetails {
        card_data,
        cof_contract,
    })))
}

fn get_applepay_details(
    wallet_data: &WalletData,
    applepay_data: &ApplePayWalletData,
) -> CustomResult<ApplePayDetails, IntegrationError> {
    let payment_data = WalletData::get_wallet_token_as_json(wallet_data, "Apple Pay".to_string())?;
    Ok(ApplePayDetails {
        payment_data,
        payment_method: ApplepayPaymentMethod {
            display_name: applepay_data.payment_method.display_name.to_owned(),
            network: applepay_data.payment_method.network.to_owned(),
            token_type: applepay_data.payment_method.pm_type.to_owned(),
        },
        transaction_identifier: applepay_data.transaction_identifier.to_owned(),
    })
}

fn get_card_details<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    req_card: &Card<T>,
) -> Result<CardDetails<T>, IntegrationError> {
    Ok(CardDetails {
        card_number: req_card.card_number.clone(),
        expiry_month: req_card.card_exp_month.clone(),
        expiry_year: req_card.card_exp_year.clone(),
        verification: req_card.card_cvc.clone(),
    })
}

fn get_wallet_details<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    wallet: &WalletData,
) -> Result<
    (Option<NexinetsPaymentDetails<T>>, NexinetsProduct),
    error_stack::Report<IntegrationError>,
> {
    match wallet {
        WalletData::PaypalRedirect(_) => Ok((None, NexinetsProduct::Paypal)),
        WalletData::ApplePay(applepay_data) => Ok((
            Some(NexinetsPaymentDetails::Wallet(Box::new(
                NexinetsWalletDetails::ApplePayToken(Box::new(get_applepay_details(
                    wallet,
                    applepay_data,
                )?)),
            ))),
            NexinetsProduct::Applepay,
        )),
        WalletData::AliPayQr(_)
        | WalletData::BluecodeRedirect { .. }
        | WalletData::AliPayRedirect(_)
        | WalletData::AliPayHkRedirect(_)
        | WalletData::AmazonPayRedirect(_)
        | WalletData::MomoRedirect(_)
        | WalletData::KakaoPayRedirect(_)
        | WalletData::GoPayRedirect(_)
        | WalletData::GcashRedirect(_)
        | WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_)
        | WalletData::DanaRedirect { .. }
        | WalletData::GooglePay(_)
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_)
        | WalletData::MbWayRedirect(_)
        | WalletData::MobilePayRedirect(_)
        | WalletData::PaypalSdk(_)
        | WalletData::Paze(_)
        | WalletData::SamsungPay(_)
        | WalletData::TwintRedirect { .. }
        | WalletData::VippsRedirect { .. }
        | WalletData::TouchNGoRedirect(_)
        | WalletData::WeChatPayRedirect(_)
        | WalletData::WeChatPayQr(_)
        | WalletData::CashappQr(_)
        | WalletData::SwishQr(_)
        | WalletData::Mifinity(_)
        | WalletData::RevolutPay(_)
        | WalletData::MbWay(_)
        | WalletData::Satispay(_)
        | WalletData::Wero(_)
        | WalletData::LazyPayRedirect(_)
        | WalletData::PhonePeRedirect(_)
        | WalletData::BillDeskRedirect(_)
        | WalletData::CashfreeRedirect(_)
        | WalletData::PayURedirect(_)
        | WalletData::EaseBuzzRedirect(_) => Err(IntegrationError::not_implemented(
            utils::get_unimplemented_payment_method_error_message("nexinets"),
        ))?,
    }
}

pub fn get_order_id(
    meta: &NexinetsPaymentsMetadata,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let order_id =
        meta.order_id
            .clone()
            .ok_or(IntegrationError::MissingConnectorRelatedTransactionID {
                id: "order_id".to_string(),
                context: Default::default(),
            })?;
    Ok(order_id)
}

pub fn get_transaction_id(
    meta: &NexinetsPaymentsMetadata,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let transaction_id = meta.transaction_id.clone().ok_or(
        IntegrationError::MissingConnectorRelatedTransactionID {
            id: "transaction_id".to_string(),
            context: Default::default(),
        },
    )?;
    Ok(transaction_id)
}

fn is_mandate_payment<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &PaymentsAuthorizeData<T>,
) -> bool {
    (item.setup_future_usage == Some(enums::FutureUsage::OffSession))
        || item
            .mandate_id
            .as_ref()
            .and_then(|mandate_ids| mandate_ids.mandate_reference_id.as_ref())
            .is_some()
}

// ===== CLIENT AUTHENTICATION TOKEN FLOW STRUCTURES =====

/// Request to create a Nexinets order for client-side hosted payment page initialization.
/// Returns an orderId that serves as a client authentication token.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsClientAuthRequest {
    pub initial_amount: i64,
    pub currency: enums::Currency,
    pub channel: NexinetsChannel,
    pub transaction_type: NexinetsTransactionType,
    #[serde(rename = "async")]
    pub nexinets_async: NexinetsAsyncDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexinetsRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexinetsClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NexinetsRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let return_url = router_data.resource_common_data.return_url.clone();
        let nexinets_async = NexinetsAsyncDetails {
            success_url: return_url.clone(),
            cancel_url: return_url.clone(),
            failure_url: return_url,
        };

        Ok(Self {
            initial_amount: router_data.request.amount.get_amount_as_i64(),
            currency: router_data.request.currency,
            channel: NexinetsChannel::Ecom,
            transaction_type: NexinetsTransactionType::Preauth,
            nexinets_async,
        })
    }
}

/// Nexinets order creation response — contains the orderId
/// used as a client authentication token for hosted checkout.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsClientAuthResponse {
    pub order_id: String,
}

impl TryFrom<ResponseRouterData<NexinetsClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NexinetsClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Nexinets(
                NexinetsClientAuthenticationResponseDomain {
                    order_id: response.order_id,
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

// ============================================================================
// SetupMandate Flow
// ============================================================================
//
// Nexinets does not expose a dedicated mandate-setup endpoint. The idiomatic
// card-on-file pattern is to issue a PREAUTH against `/orders/preauth` with a
// `cofContract` block of type `UNSCHEDULED`, which instructs Nexinets to
// persist the payment instrument and return a reusable
// `paymentInstrument.paymentInstrumentId` used as the `connector_mandate_id`
// for subsequent RepeatPayment (MIT) calls. Zero-amount preauths are not
// universally accepted, so a caller-supplied amount is preferred and we fall
// back to a minimum unit amount when none is provided.

/// SetupMandate request — reuses the same wire shape as the standard Nexinets
/// payments request. We always send card + cofContract(UNSCHEDULED) so the
/// resulting preauth returns a persistable `paymentInstrumentId`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    initial_amount: i64,
    currency: enums::Currency,
    channel: NexinetsChannel,
    product: NexinetsProduct,
    payment: NexinetsPaymentDetails<T>,
    #[serde(rename = "async")]
    nexinets_async: NexinetsAsyncDetails,
    merchant_order_id: Option<String>,
}

/// SetupMandate response — reuses the standard Nexinets preauth/debit
/// response which already carries `paymentInstrument.paymentInstrumentId`.
pub type NexinetsSetupMandateResponse = NexinetsPreAuthOrDebitResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexinetsRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexinetsSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: NexinetsRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        // Prefer request-level return URLs (SetupMandateRequestData.return_url
        // / router_return_url) over the flow-data return_url, which is often
        // None for SetupMandate. Nexinets rejects the preauth call when the
        // async URLs are null, so fall back to a benign placeholder.
        let return_url = request
            .return_url
            .clone()
            .or_else(|| request.router_return_url.clone())
            .or_else(|| router_data.resource_common_data.return_url.clone())
            .or_else(|| Some("https://hyperswitch.io/return".to_string()));
        let nexinets_async = NexinetsAsyncDetails {
            success_url: return_url.clone(),
            cancel_url: return_url.clone(),
            failure_url: return_url,
        };

        let (payment, product) = match &request.payment_method_data {
            PaymentMethodData::Card(card) => {
                let card_details = get_card_details(card)?;
                let nexi_card = NexiCardDetails {
                    card_data: CardDataDetails::CardDetails(Box::new(card_details)),
                    cof_contract: Some(CofContract {
                        recurring_type: RecurringType::Unscheduled,
                    }),
                };
                (
                    NexinetsPaymentDetails::Card(Box::new(nexi_card)),
                    NexinetsProduct::Creditcard,
                )
            }
            _ => {
                return Err(IntegrationError::not_implemented(
                    utils::get_unimplemented_payment_method_error_message("nexinets"),
                ))?;
            }
        };

        // Prefer caller-supplied amount; fall back to a minimal unit amount
        // (1) since Nexinets does not universally accept zero-amount
        // preauths for card-on-file verification.
        let initial_amount = request
            .amount
            .or_else(|| request.minor_amount.map(|m| m.get_amount_as_i64()))
            .unwrap_or(1);

        let merchant_order_id = Some(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        Ok(Self {
            initial_amount,
            currency: request.currency,
            channel: NexinetsChannel::Ecom,
            product,
            payment,
            nexinets_async,
            merchant_order_id,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NexinetsSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NexinetsSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let transaction = response.transactions.first().ok_or_else(|| {
            crate::utils::response_handling_fail_for_connector(http_code, "nexinets")
        })?;

        let nexinets_metadata = NexinetsPaymentsMetadata {
            transaction_id: Some(transaction.transaction_id.clone()),
            order_id: Some(response.order_id.clone()),
            psync_flow: response.transaction_type.clone(),
        };
        let connector_metadata = serde_json::to_value(&nexinets_metadata).change_context(
            crate::utils::response_handling_fail_for_connector(http_code, "nexinets"),
        )?;

        let redirection_data = response
            .redirect_url
            .map(|url| RedirectForm::from((url, Method::Get)));

        // For SetupMandate, surface the connector `paymentInstrumentId` as
        // the connector_mandate_id for subsequent RepeatPayment (MIT) calls.
        let mandate_reference =
            response
                .payment_instrument
                .payment_instrument_id
                .map(|id| MandateReference {
                    connector_mandate_id: Some(id.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                });

        // Promote Authorized to Charged so the zero/low-amount mandate setup
        // reaches a terminal state for downstream consumers.
        let mut status = get_status(
            transaction.status.clone(),
            response.transaction_type.clone(),
        );
        if status == AttemptStatus::Authorized {
            status = AttemptStatus::Charged;
        }

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction.transaction_id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: Some(connector_metadata),
                network_txn_id: None,
                connector_response_reference_id: Some(response.order_id),
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

// ============================================================================
// RepeatPayment Flow (MIT / Merchant-Initiated Transaction)
// ============================================================================
//
// Nexinets MIT reuses the standard `/orders/debit` (auto-capture) or
// `/orders/preauth` (manual-capture) payment endpoints, replacing the card
// panel with a `paymentInstrument.paymentInstrumentId` reference that was
// returned by the prior SetupMandate (UNSCHEDULED COF) call. No 3DS is
// required because the transaction is off-session. The request still carries
// a `cofContract` of type UNSCHEDULED to flag the payment as a subsequent
// MIT in the stored-credential lifecycle.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsRepeatPaymentRequest {
    initial_amount: i64,
    currency: enums::Currency,
    channel: NexinetsChannel,
    product: NexinetsProduct,
    payment: NexinetsRepeatPaymentDetails,
    #[serde(rename = "async")]
    nexinets_async: NexinetsAsyncDetails,
    merchant_order_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexinetsRepeatPaymentDetails {
    payment_instrument_id: Secret<String>,
    cof_contract: CofContract,
}

pub type NexinetsRepeatPaymentResponse = NexinetsPreAuthOrDebitResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexinetsRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexinetsRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: NexinetsRouterData<
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
        let request = &router_data.request;

        let payment_instrument_id = match &request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => mandate_data
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                }
                .into());
            }
        };

        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| router_data.resource_common_data.return_url.clone())
            .or_else(|| Some("https://hyperswitch.io/return".to_string()));
        let nexinets_async = NexinetsAsyncDetails {
            success_url: return_url.clone(),
            cancel_url: return_url.clone(),
            failure_url: return_url,
        };

        let payment = NexinetsRepeatPaymentDetails {
            payment_instrument_id: Secret::new(payment_instrument_id),
            cof_contract: CofContract {
                recurring_type: RecurringType::Unscheduled,
            },
        };

        let merchant_order_id = Some(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        Ok(Self {
            initial_amount: request.amount,
            currency: request.currency,
            channel: NexinetsChannel::Ecom,
            product: NexinetsProduct::Creditcard,
            payment,
            nexinets_async,
            merchant_order_id,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NexinetsRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexinetsRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let transaction = response.transactions.first().ok_or_else(|| {
            crate::utils::response_handling_fail_for_connector(http_code, "nexinets")
        })?;

        let nexinets_metadata = NexinetsPaymentsMetadata {
            transaction_id: Some(transaction.transaction_id.clone()),
            order_id: Some(response.order_id.clone()),
            psync_flow: response.transaction_type.clone(),
        };
        let connector_metadata = serde_json::to_value(&nexinets_metadata).change_context(
            crate::utils::response_handling_fail_for_connector(http_code, "nexinets"),
        )?;

        let mandate_reference =
            response
                .payment_instrument
                .payment_instrument_id
                .map(|id| MandateReference {
                    connector_mandate_id: Some(id.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                });

        let status = get_status(
            transaction.status.clone(),
            response.transaction_type.clone(),
        );

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: Some(connector_metadata),
                network_txn_id: None,
                connector_response_reference_id: Some(response.order_id),
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            ..router_data
        })
    }
}
