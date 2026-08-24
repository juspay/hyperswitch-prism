use crate::types::ResponseRouterData;
use common_enums::RefundStatus;
use common_utils::{types::MinorUnit, Email};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        ServerAuthenticationToken, Void,
    },
    connector_types::{
        MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, ServerAuthenticationTokenResponseData,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::MonerisRouterData;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const CLIENT_CREDENTIALS: &str = "client_credentials";

pub mod auth_headers {
    pub const X_MERCHANT_ID: &str = "X-Merchant-Id";
    pub const API_VERSION: &str = "Api-Version";
    pub const API_VERSION_VALUE: &str = "2026-08-14";
}

pub struct MonerisAuthType {
    pub(super) client_id: Secret<String>,
    pub(super) client_secret: Secret<String>,
    pub(super) merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for MonerisAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Moneris {
                client_secret,
                merchant_id,
                client_id,
                ..
            } => Ok(Self {
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure this merchant account's Moneris connector with a \
                             client_id, client_secret and merchant_id (ConnectorSpecificConfig::Moneris)."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "The connector_config passed to MonerisAuthType::try_from was not \
                             the ConnectorSpecificConfig::Moneris variant — either a different \
                             connector's config was routed to Moneris, or Moneris's client_id/ \
                             client_secret/merchant_id were never configured for this merchant account."
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MonerisAuthRequest {
    client_id: Secret<String>,
    client_secret: Secret<String>,
    grant_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for MonerisAuthRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = MonerisAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            client_id: auth.client_id.clone(),
            client_secret: auth.client_secret.clone(),
            grant_type: CLIENT_CREDENTIALS.to_string(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MonerisAuthResponse {
    access_token: Secret<String>,
    token_type: String,
    expires_in: String,
}

impl<F, T> TryFrom<ResponseRouterData<MonerisAuthResponse, Self>>
    for RouterDataV2<F, MerchantAuthenticationFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<MonerisAuthResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in.parse::<i64>().change_context(
                    ConnectorError::ResponseDeserializationFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: None,
                            additional_context: Some(format!(
                                "Failed to parse Moneris `expires_in` value as i64: {:?}",
                                item.response.expires_in
                            )),
                        },
                    },
                )?),
                token_type: Some(item.response.token_type),
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    idempotency_key: String,
    amount: Amount,
    payment_method: PaymentMethod<T>,
    automatic_capture: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecommerce_indicator: Option<MonerisEcommerceIndicator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    three_d_secure_data: Option<MonerisThreeDSecureData>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MonerisEcommerceIndicator {
    AuthenticatedEcommerce,
    NonAuthenticatedEcommerce,
    MailTelephoneOrderRecurring,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisThreeDSecureCryptogramData {
    three_d_secure_cryptogram: Secret<String>,
    three_d_secure_version: String,
    three_d_secure_server_transaction_id: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum MonerisThreeDSecureData {
    Cryptogram(MonerisThreeDSecureCryptogramData),
}

#[derive(Default, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    currency: common_enums::Currency,
    amount: MinorUnit,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum PaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(PaymentMethodCard<T>),
    PaymentMethodId(PaymentMethodId),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    payment_method_source: PaymentMethodSource,
    card: MonerisCard<T>,
    store_payment_method: StorePaymentMethod,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodId {
    payment_method_source: PaymentMethodSource,
    payment_method_id: Secret<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentMethodSource {
    Card,
    PaymentMethodId,
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card_number: RawCardNumber<T>,
    expiry_month: Secret<i64>,
    expiry_year: Secret<i64>,
    card_security_code: Secret<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorePaymentMethod {
    DoNotStore,
    CardholderInitiated,
    MerchantInitiated,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ref req_card) => {
                let idempotency_key = format!(
                    "auth_{}",
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                );
                let amount = Amount {
                    currency: item.router_data.request.currency,
                    amount: item.router_data.request.amount,
                };
                let payment_method = PaymentMethod::Card(PaymentMethodCard {
                    payment_method_source: PaymentMethodSource::Card,
                    card: MonerisCard {
                        card_number: req_card.card_number.clone(),
                        expiry_month: Secret::new(
                            req_card
                                .card_exp_month
                                .peek()
                                .parse::<i64>()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "card_exp_month",
                                    context: IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Pass card_exp_month as a 1- or 2-digit numeric \
                                             string (e.g. \"1\" or \"12\")."
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: Some(format!(
                                            "Failed to parse card expiry month as i64; \
                                             received value: {:?}.",
                                            req_card.card_exp_month.peek()
                                        )),
                                    },
                                })?,
                        ),
                        expiry_year: Secret::new(
                            req_card
                                .get_expiry_year_4_digit()
                                .peek()
                                .parse::<i64>()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "card_exp_year",
                                    context: IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Pass card_exp_year as a 4-digit numeric string \
                                             (e.g. \"2030\")."
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: Some(format!(
                                            "Failed to parse card expiry year as i64; \
                                             received value: {:?}.",
                                            req_card.get_expiry_year_4_digit().peek()
                                        )),
                                    },
                                })?,
                        ),
                        card_security_code: req_card.card_cvc.clone(),
                    },
                    store_payment_method: if item
                        .router_data
                        .request
                        .is_customer_initiated_mandate_payment()
                    {
                        StorePaymentMethod::CardholderInitiated
                    } else {
                        StorePaymentMethod::DoNotStore
                    },
                });
                let automatic_capture = item.router_data.request.is_auto_capture();

                let (three_d_secure_data, ecommerce_indicator) =
                    build_three_d_secure_data(&item.router_data.request.authentication_data);

                Ok(Self {
                    idempotency_key,
                    amount,
                    payment_method,
                    automatic_capture,
                    three_d_secure_data,
                    ecommerce_indicator,
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only card payments are currently implemented for Moneris".to_string(),
                IntegrationErrorContext {
                    suggested_action: Some(
                        "Use a card payment method, the only method currently implemented \
                         for Moneris authorize."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Moneris authorize received a non-card payment method: {:?}.",
                        item.router_data.request.payment_method_data
                    )),
                },
            )
            .into()),
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisAuthorizeResponse {
    payment_status: MonerisPaymentStatus,
    payment_id: String,
    payment_method: MonerisPaymentMethodData,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MonerisAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mandate_reference = if item
            .router_data
            .request
            .is_customer_initiated_mandate_payment()
        {
            Some(Box::new(MandateReference {
                connector_mandate_id: Some(
                    item.response
                        .payment_method
                        .payment_method_id
                        .peek()
                        .to_string(),
                ),
                payment_method_id: None,
                mandate_metadata: None,
                connector_mandate_request_reference_id: None,
            }))
        } else {
            None
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.payment_status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPaymentsResponse {
    payment_status: MonerisPaymentStatus,
    payment_id: String,
    payment_method: MonerisPaymentMethodData,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MonerisPaymentStatus {
    Succeeded,
    #[default]
    Processing,
    Canceled,
    Declined,
    DeclinedRetry,
    Authorized,
}

impl From<MonerisPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: MonerisPaymentStatus) -> Self {
        match item {
            MonerisPaymentStatus::Succeeded => Self::Charged,
            MonerisPaymentStatus::Authorized => Self::Authorized,
            MonerisPaymentStatus::Canceled => Self::Voided,
            MonerisPaymentStatus::Declined | MonerisPaymentStatus::DeclinedRetry => Self::Failure,
            MonerisPaymentStatus::Processing => Self::Pending,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPaymentMethodData {
    payment_method_id: Secret<String>,
}

impl<F, T> TryFrom<ResponseRouterData<MonerisPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.payment_status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Moneris does not return a distinct status for partial capture — a successful completion
// always comes back as SUCCEEDED regardless of whether the captured amount is less than
// the authorized amount. This dedicated response type carries out the amount comparison
// that the generic MonerisPaymentsResponse impl cannot perform.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisCaptureResponse {
    payment_status: MonerisPaymentStatus,
    payment_id: String,
    payment_method: MonerisPaymentMethodData,
}

impl TryFrom<ResponseRouterData<MonerisCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.payment_status {
            MonerisPaymentStatus::Succeeded => {
                let captured = item.router_data.request.minor_amount_to_capture;
                match item
                    .router_data
                    .resource_common_data
                    .minor_amount_capturable
                {
                    Some(authorized) if captured < authorized => {
                        common_enums::AttemptStatus::PartialCharged
                    }
                    _ => common_enums::AttemptStatus::Charged,
                }
            }
            other => common_enums::AttemptStatus::from(other),
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisRepeatPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    idempotency_key: String,
    amount: Amount,
    payment_method: PaymentMethod<T>,
    automatic_capture: bool,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisRepeatPaymentRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::MandatePayment => {
                let idempotency_key = format!(
                    "repeat_{}",
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                );
                let amount = Amount {
                    currency: item.router_data.request.currency,
                    amount: item.router_data.request.minor_amount,
                };
                let automatic_capture = item.router_data.request.is_auto_capture();
                let payment_method = PaymentMethod::PaymentMethodId(PaymentMethodId {
                    payment_method_source: PaymentMethodSource::PaymentMethodId,
                    payment_method_id: item
                        .router_data
                        .request
                        .connector_mandate_id()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Provide `connector_mandate_id` in the RepeatPayment \
                                     request — this is the mandate reference returned by \
                                     Moneris during the initial customer-initiated mandate \
                                     setup (Authorize with `setup_future_usage`)."
                                        .to_string(),
                                ),
                                doc_url: None,
                                additional_context: Some(
                                    "RepeatPayment calls Moneris's stored-credential \
                                     endpoint which requires a `payment_method_id` derived \
                                     from `connector_mandate_id`, but the field was None."
                                        .to_string(),
                                ),
                            },
                        })?
                        .into(),
                });
                Ok(Self {
                    idempotency_key,
                    amount,
                    payment_method,
                    automatic_capture,
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only mandate (stored-credential) payments are currently implemented for \
                 Moneris repeat_payment"
                    .to_string(),
                IntegrationErrorContext {
                    suggested_action: Some(
                        "Use `payment_method_data = MandatePayment` together with a valid \
                         `connector_mandate_id`; Moneris repeat_payment is a merchant-initiated \
                         transaction that charges a previously stored credential."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Moneris repeat_payment received a non-mandate payment method \
                         variant: {:?}.",
                        item.router_data.request.payment_method_data
                    )),
                },
            )
            .into()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPaymentsCaptureRequest {
    amount: Amount,
    idempotency_key: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for MonerisPaymentsCaptureRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = Amount {
            currency: item.router_data.request.currency,
            amount: item.router_data.request.minor_amount_to_capture,
        };
        let idempotency_key = format!(
            "capture_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );
        Ok(Self {
            amount,
            idempotency_key,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisCancelRequest {
    idempotency_key: String,
    reason: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for MonerisCancelRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let idempotency_key = format!(
            "void_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );
        let reason = item.router_data.request.cancellation_reason.clone();
        Ok(Self {
            idempotency_key,
            reason,
        })
    }
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisRefundRequest {
    pub refund_amount: Amount,
    pub idempotency_key: String,
    pub reason: Option<String>,
    pub payment_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for MonerisRefundRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let refund_amount = Amount {
            currency: item.router_data.request.currency,
            amount: item.router_data.request.minor_refund_amount,
        };
        let idempotency_key = format!(
            "refund_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );
        let reason = item.router_data.request.reason.clone();
        let payment_id = item.router_data.request.connector_transaction_id.clone();
        Ok(Self {
            refund_amount,
            idempotency_key,
            reason,
            payment_id,
        })
    }
}

#[derive(Debug, Serialize, Default, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MonerisRefundStatus {
    Succeeded,
    #[default]
    Processing,
    Declined,
    DeclinedRetry,
}

impl From<MonerisRefundStatus> for RefundStatus {
    fn from(item: MonerisRefundStatus) -> Self {
        match item {
            MonerisRefundStatus::Succeeded => Self::Success,
            MonerisRefundStatus::Declined | MonerisRefundStatus::DeclinedRetry => Self::Failure,
            MonerisRefundStatus::Processing => Self::Pending,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisRefundResponse {
    refund_id: String,
    refund_status: MonerisRefundStatus,
}

impl TryFrom<ResponseRouterData<MonerisRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.refund_id.to_string(),
                refund_status: RefundStatus::from(item.response.refund_status),
                acquirer_reference_number: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<MonerisRefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MonerisRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.refund_id.to_string(),
                refund_status: RefundStatus::from(item.response.refund_status),
                acquirer_reference_number: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisErrorResponse {
    pub status: u16,
    pub category: String,
    pub title: String,
    pub errors: Option<Vec<MonerisError>>,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisError {
    pub reason_code: String,
    pub parameter_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MonerisAuthErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
}

// ===== 3DS FLOW TYPES =====

pub fn build_three_d_secure_data(
    authentication_data: &Option<AuthenticationData>,
) -> (
    Option<MonerisThreeDSecureData>,
    Option<MonerisEcommerceIndicator>,
) {
    let Some(auth) = authentication_data.as_ref() else {
        return (None, None);
    };
    let Some(cavv) = auth.cavv.clone() else {
        return (None, None);
    };
    let three_d_secure_data = Some(MonerisThreeDSecureData::Cryptogram(
        MonerisThreeDSecureCryptogramData {
            three_d_secure_cryptogram: cavv,
            three_d_secure_version: "V2".to_string(),
            three_d_secure_server_transaction_id: auth
                .threeds_server_transaction_id
                .clone()
                .unwrap_or_default(),
        },
    ));
    (
        three_d_secure_data,
        Some(MonerisEcommerceIndicator::AuthenticatedEcommerce),
    )
}

// ---- 3DS Enums ----

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureMessageCategory {
    Payment,
    NonPayment,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureDeviceChannel {
    Browser,
    ThreeDSecureRequestorInitiated,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureRequestType {
    Cardholder,
    Recurring,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureCompletionIndicator {
    Success,
    Failure,
    Unavailable,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureChallengeRequested {
    NoPreference,
    NoChallengeRequested,
    ChallengeRequestedMandate,
    ChallengeRequestedPreference,
    NoChallengeRequestedRiskAnalysisPerformed,
    NoChallengeRequestedDataOnly,
    NoChallengeRequestedAuthenticationPerformed,
    NoChallengeRequestedWhitelistExemption,
    ChallengeRequestedWhitelistPrompt,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureChallengeWindowSize {
    XSmall,
    Small,
    Medium,
    Large,
    FullScreen,
}

// ---- 3DS Billing Address ----

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonerisBillingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub province: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
}

// ---- 3DS Transaction Status ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MonerisThreeDSecureTransactionStatus {
    Authenticated,
    AuthenticatedAttempted,
    ChallengeAuthenticationRequired,
    NotAuthenticated,
    Rejected,
    InformationOnly,
    Decoupled,
}

// ---- 3DS Payment Method (no store_payment_method for auth requests) ----

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Moneris3DSCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card_number: RawCardNumber<T>,
    expiry_month: Secret<i64>,
    expiry_year: Secret<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    card_security_code: Option<Secret<String>>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Moneris3DSPaymentMethodCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    payment_method_source: PaymentMethodSource,
    card: Moneris3DSCard<T>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Moneris3DSPaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(Moneris3DSPaymentMethodCard<T>),
}

// ---- PreAuthenticate Request ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPreAuthenticateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub idempotency_key: String,
    pub amount: Amount,
    pub cardholder_name: Secret<String>,
    pub cardholder_email: Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_phone_number: Option<Secret<String>>,
    pub billing_address: MonerisBillingAddress,
    pub payment_method: Moneris3DSPaymentMethod<T>,
    pub three_d_secure_message_category: ThreeDSecureMessageCategory,
    pub three_d_secure_device_channel: ThreeDSecureDeviceChannel,
    pub three_d_secure_request_type: ThreeDSecureRequestType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_notification_url: Option<String>,
    pub three_d_secure_completion_indicator: ThreeDSecureCompletionIndicator,
    pub three_d_secure_challenge_requested: ThreeDSecureChallengeRequested,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_challenge_window_size: Option<ThreeDSecureChallengeWindowSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_ip_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_javascript_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_screen_height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_screen_width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_language: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisPreAuthenticateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let resource = &item.router_data.resource_common_data;

        let payment_method_data =
            req.payment_method_data
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_data",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Provide card data in the PreAuthenticate request.".to_string(),
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                })?;

        let payment_method = match payment_method_data {
            PaymentMethodData::Card(ref req_card) => {
                let expiry_month = Secret::new(
                    req_card
                        .card_exp_month
                        .peek()
                        .parse::<i64>()
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "card_exp_month",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Pass card_exp_month as a 1- or 2-digit numeric string."
                                        .to_string(),
                                ),
                                doc_url: None,
                                additional_context: Some(format!(
                                    "Failed to parse card expiry month as i64; received: {:?}.",
                                    req_card.card_exp_month.peek()
                                )),
                            },
                        })?,
                );
                let expiry_year = Secret::new(
                    req_card
                        .get_expiry_year_4_digit()
                        .peek()
                        .parse::<i64>()
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "card_exp_year",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Pass card_exp_year as a 4-digit numeric string.".to_string(),
                                ),
                                doc_url: None,
                                additional_context: Some(format!(
                                    "Failed to parse card expiry year as i64; received: {:?}.",
                                    req_card.get_expiry_year_4_digit().peek()
                                )),
                            },
                        })?,
                );
                Moneris3DSPaymentMethod::Card(Moneris3DSPaymentMethodCard {
                    payment_method_source: PaymentMethodSource::Card,
                    card: Moneris3DSCard {
                        card_number: req_card.card_number.clone(),
                        expiry_month,
                        expiry_year,
                        card_security_code: Some(req_card.card_cvc.clone()),
                    },
                })
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Moneris 3DS PreAuthenticate only supports card payments".to_string(),
                    IntegrationErrorContext {
                        suggested_action: Some(
                            "Use a card payment method for 3DS authentication.".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(format!(
                            "Received non-card payment method: {:?}.",
                            payment_method_data
                        )),
                    },
                )
                .into())
            }
        };

        let cardholder_email = req
            .email
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "email",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide cardholder email for Moneris 3DS authentication.".to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            })?;

        let cardholder_name = resource.get_billing_full_name().map_err(|_| {
            IntegrationError::MissingRequiredField {
                field_name: "billing.address.first_name",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide cardholder name in billing address for Moneris 3DS.".to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            }
        })?;

        let billing_address = MonerisBillingAddress {
            street_name: resource.get_optional_billing_line1(),
            unit_number: resource.get_optional_billing_line2(),
            street_number: None,
            city: resource.get_optional_billing_city(),
            province: resource.get_optional_billing_state(),
            postal_code: resource.get_optional_billing_zip(),
            country: resource.get_optional_billing_country(),
        };

        let amount = Amount {
            currency: req.currency.unwrap_or_default(),
            amount: req.amount,
        };

        let idempotency_key = format!("3ds_{}", resource.connector_request_reference_id);

        let three_d_secure_notification_url = req
            .router_return_url
            .as_ref()
            .map(|url| urlencoding::encode(url.as_str()).into_owned());

        let (
            device_channel,
            browser_ip_address,
            browser_screen_height,
            browser_screen_width,
            browser_user_agent,
            browser_java_enabled,
            browser_javascript_enabled,
            browser_language,
        ) = match &req.browser_info {
            Some(info) => (
                ThreeDSecureDeviceChannel::Browser,
                info.ip_address.map(|ip| Secret::new(ip.to_string())),
                info.screen_height.and_then(|h| i32::try_from(h).ok()),
                info.screen_width.and_then(|w| i32::try_from(w).ok()),
                info.user_agent.clone(),
                info.java_enabled,
                info.java_script_enabled,
                info.language.clone(),
            ),
            // Without browser context this is a server-initiated (3RI) flow.
            None => (
                ThreeDSecureDeviceChannel::ThreeDSecureRequestorInitiated,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
        };

        let cardholder_phone_number = resource.get_optional_billing_phone_number();

        Ok(Self {
            idempotency_key,
            amount,
            cardholder_name,
            cardholder_email,
            cardholder_phone_number,
            billing_address,
            payment_method,
            three_d_secure_message_category: ThreeDSecureMessageCategory::Payment,
            three_d_secure_device_channel: device_channel,
            three_d_secure_request_type: ThreeDSecureRequestType::Cardholder,
            three_d_secure_notification_url,
            three_d_secure_completion_indicator: ThreeDSecureCompletionIndicator::Unavailable,
            three_d_secure_challenge_requested: ThreeDSecureChallengeRequested::NoPreference,
            three_d_secure_challenge_window_size: Some(ThreeDSecureChallengeWindowSize::FullScreen),
            browser_ip_address,
            browser_user_agent,
            browser_java_enabled,
            browser_javascript_enabled,
            browser_screen_height,
            browser_screen_width,
            browser_language,
        })
    }
}

// ---- PreAuthenticate Response ----

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPreAuthenticateResponse {
    pub authentication_id: String,
    pub three_d_secure_transaction_status: MonerisThreeDSecureTransactionStatus,
    pub three_d_secure_authentication_value: Option<Secret<String>>,
    pub ecommerce_indicator: Option<String>,
    pub three_d_secure_server_transaction_id: Option<String>,
    pub three_d_secure_version: Option<String>,
    pub challenge_url: Option<String>,
    pub challenge_data: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MonerisPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MonerisPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        let (status, response_data) = match response.three_d_secure_transaction_status {
            MonerisThreeDSecureTransactionStatus::Authenticated
            | MonerisThreeDSecureTransactionStatus::AuthenticatedAttempted
            | MonerisThreeDSecureTransactionStatus::InformationOnly => {
                let authentication_data = Some(AuthenticationData {
                    trans_status: Some(common_enums::TransactionStatus::Success),
                    eci: response.ecommerce_indicator.clone(),
                    cavv: response.three_d_secure_authentication_value.clone(),
                    threeds_server_transaction_id: response
                        .three_d_secure_server_transaction_id
                        .clone(),
                    transaction_id: Some(response.authentication_id.clone()),
                    ucaf_collection_indicator: None,
                    message_version: None,
                    ds_trans_id: None,
                    acs_transaction_id: None,
                    network_params: None,
                    exemption_indicator: None,
                    created_at: None,
                    challenge_code: None,
                    challenge_cancel: None,
                    challenge_code_reason: None,
                    message_extension: None,
                    authentication_type: None,
                });
                let response_data = PaymentsResponseData::PreAuthenticateResponse {
                    resource_id: Some(ResponseId::ConnectorTransactionId(
                        response.authentication_id.clone(),
                    )),
                    authentication_data,
                    redirection_data: None,
                    connector_response_reference_id: Some(response.authentication_id.clone()),
                    status_code: item.http_code,
                };
                (
                    common_enums::AttemptStatus::AuthenticationSuccessful,
                    response_data,
                )
            }
            MonerisThreeDSecureTransactionStatus::ChallengeAuthenticationRequired => {
                let challenge_url = response.challenge_url.clone().ok_or_else(|| {
                    error_stack::report!(ConnectorError::ResponseDeserializationFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: Some(item.http_code),
                            additional_context: Some(
                                "Moneris returned CHALLENGE_AUTHENTICATION_REQUIRED but \
                                 challenge_url was missing in the response."
                                    .to_string(),
                            ),
                        },
                    })
                })?;
                let challenge_data = response.challenge_data.clone().ok_or_else(|| {
                    error_stack::report!(ConnectorError::ResponseDeserializationFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: Some(item.http_code),
                            additional_context: Some(
                                "Moneris returned CHALLENGE_AUTHENTICATION_REQUIRED but \
                                 challenge_data (creq) was missing in the response."
                                    .to_string(),
                            ),
                        },
                    })
                })?;
                let mut form_fields = HashMap::new();
                form_fields.insert("creq".to_string(), challenge_data);
                let redirection_data = Some(Box::new(
                    domain_types::router_response_types::RedirectForm::Form {
                        endpoint: challenge_url,
                        method: common_utils::request::Method::Post,
                        form_fields,
                    },
                ));
                let response_data = PaymentsResponseData::PreAuthenticateResponse {
                    resource_id: Some(ResponseId::ConnectorTransactionId(
                        response.authentication_id.clone(),
                    )),
                    authentication_data: None,
                    redirection_data,
                    connector_response_reference_id: Some(response.authentication_id.clone()),
                    status_code: item.http_code,
                };
                (
                    common_enums::AttemptStatus::AuthenticationPending,
                    response_data,
                )
            }
            MonerisThreeDSecureTransactionStatus::NotAuthenticated
            | MonerisThreeDSecureTransactionStatus::Rejected
            | MonerisThreeDSecureTransactionStatus::Decoupled => {
                let response_data = PaymentsResponseData::PreAuthenticateResponse {
                    resource_id: Some(ResponseId::ConnectorTransactionId(
                        response.authentication_id.clone(),
                    )),
                    authentication_data: None,
                    redirection_data: None,
                    connector_response_reference_id: Some(response.authentication_id.clone()),
                    status_code: item.http_code,
                };
                (
                    common_enums::AttemptStatus::AuthenticationFailed,
                    response_data,
                )
            }
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(response_data),
            ..item.router_data
        })
    }
}

// ---- PostAuthenticate Request ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPostAuthenticateRequest {
    pub idempotency_key: String,
    pub three_d_secure_challenge_response_data: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MonerisRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MonerisPostAuthenticateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MonerisRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let cres_payload = item.router_data.request.get_redirect_response_payload()?;

        let cres_value = cres_payload
            .peek()
            .as_str()
            .map(|s| s.to_owned())
            .or_else(|| {
                cres_payload
                    .peek()
                    .get("cres")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
            })
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "redirect_response.payload (cres)",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the cres value from the ACS callback is present in \
                         redirect_response.payload for PostAuthenticate."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            })?;

        let idempotency_key = format!(
            "postauth_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            idempotency_key,
            three_d_secure_challenge_response_data: cres_value,
        })
    }
}

// ---- PostAuthenticate Response ----

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonerisPostAuthenticateResponse {
    pub three_d_secure_authentication_value: Option<Secret<String>>,
    pub ecommerce_indicator: Option<String>,
    pub three_d_secure_server_transaction_id: Option<String>,
    pub three_d_secure_transaction_status: Option<MonerisThreeDSecureTransactionStatus>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MonerisPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MonerisPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        let status = match &response.three_d_secure_transaction_status {
            Some(MonerisThreeDSecureTransactionStatus::Authenticated)
            | Some(MonerisThreeDSecureTransactionStatus::AuthenticatedAttempted)
            | Some(MonerisThreeDSecureTransactionStatus::InformationOnly) => {
                common_enums::AttemptStatus::AuthenticationSuccessful
            }
            Some(MonerisThreeDSecureTransactionStatus::NotAuthenticated)
            | Some(MonerisThreeDSecureTransactionStatus::Rejected)
            | Some(MonerisThreeDSecureTransactionStatus::Decoupled) => {
                common_enums::AttemptStatus::AuthenticationFailed
            }
            _ => common_enums::AttemptStatus::AuthenticationSuccessful,
        };

        let authentication_data = Some(AuthenticationData {
            trans_status: None,
            eci: response.ecommerce_indicator.clone(),
            cavv: response.three_d_secure_authentication_value.clone(),
            threeds_server_transaction_id: response.three_d_secure_server_transaction_id.clone(),
            transaction_id: None,
            ucaf_collection_indicator: None,
            message_version: None,
            ds_trans_id: None,
            acs_transaction_id: None,
            network_params: None,
            exemption_indicator: None,
            created_at: None,
            challenge_code: None,
            challenge_cancel: None,
            challenge_code_reason: None,
            message_extension: None,
            authentication_type: None,
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                connector_response_reference_id: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
