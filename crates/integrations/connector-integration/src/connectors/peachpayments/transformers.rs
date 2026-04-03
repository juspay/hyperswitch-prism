use super::{requests, responses, PeachpaymentsRouterData};

use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::ext_traits::StringExt;
use common_utils::{consts, errors::CustomResult, types::MinorUnit, SecretSerdeValue};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorResponseTransformationError, IntegrationError, WebhookError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::Serialize;
use std::fmt::Debug;
use time::format_description::well_known::Iso8601;
use time::OffsetDateTime;

pub fn get_error_code(response_code: Option<&responses::PeachpaymentsResponseCode>) -> String {
    match response_code {
        Some(responses::PeachpaymentsResponseCode::Text(code)) => code.clone(),
        Some(responses::PeachpaymentsResponseCode::Structured { value, .. }) => value.clone(),
        None => consts::NO_ERROR_CODE.to_string(),
    }
}

pub fn get_error_message(response_code: Option<&responses::PeachpaymentsResponseCode>) -> String {
    match response_code {
        Some(responses::PeachpaymentsResponseCode::Text(msg)) => msg.clone(),
        Some(responses::PeachpaymentsResponseCode::Structured { description, .. }) => {
            description.clone()
        }
        None => consts::NO_ERROR_CODE.to_string(),
    }
}

pub fn get_webhook_object_from_body(
    body: &[u8],
) -> CustomResult<responses::PeachpaymentsIncomingWebhook, WebhookError> {
    let body_string =
        String::from_utf8(body.to_vec()).change_context(WebhookError::WebhookBodyDecodingFailed)?;
    body_string
        .parse_struct("PeachpaymentsIncomingWebhook")
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

fn get_webhook_response(
    response: responses::PeachpaymentsIncomingWebhook,
    status_code: u16,
) -> CustomResult<
    (AttemptStatus, Result<PaymentsResponseData, ErrorResponse>),
    ConnectorResponseTransformationError,
> {
    let transaction =
        response
            .transaction
            .ok_or(crate::utils::response_handling_fail_for_connector(
                status_code,
                "peachpayments",
            ))?;

    let status: AttemptStatus = transaction.transaction_result.clone().into();

    let response_data = if status == AttemptStatus::Failure {
        Err(ErrorResponse {
            code: get_error_code(transaction.response_code.as_ref()),
            message: transaction
                .error_message
                .clone()
                .unwrap_or_else(|| get_error_message(transaction.response_code.as_ref())),
            reason: transaction.error_message.clone(),
            status_code,
            attempt_status: Some(status),
            connector_transaction_id: Some(transaction.transaction_id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    } else {
        Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction.transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(transaction.reference_id.clone()),
            incremental_authorization_allowed: None,
            status_code,
        })
    };

    Ok((status, response_data))
}

#[derive(Debug, Clone)]
pub struct PeachpaymentsAuthType {
    pub api_key: Secret<String>,
    pub tenant_id: Secret<String>,
    pub client_merchant_reference_id: Option<Secret<String>>,
    pub merchant_payment_method_route_id: Option<Secret<String>>,
}

#[derive(Debug, Clone)]
pub struct PeachpaymentsConnectorMetadataObject {
    pub client_merchant_reference_id: Secret<String>,
    pub merchant_payment_method_route_id: Secret<String>,
}

impl TryFrom<&Option<SecretSerdeValue>> for PeachpaymentsConnectorMetadataObject {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(meta_data: &Option<SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata = meta_data
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_meta_data",
                context: Default::default(),
            })?;

        let metadata_obj =
            metadata
                .peek()
                .as_object()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_meta_data",
                    context: Default::default(),
                })?;

        let client_merchant_reference_id = metadata_obj
            .get("client_merchant_reference_id")
            .and_then(|v: &serde_json::Value| v.as_str())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_meta_data.client_merchant_reference_id",
                context: Default::default(),
            })?;

        let merchant_payment_method_route_id = metadata_obj
            .get("merchant_payment_method_route_id")
            .and_then(|v: &serde_json::Value| v.as_str())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_meta_data.merchant_payment_method_route_id",
                context: Default::default(),
            })?;

        Ok(Self {
            client_merchant_reference_id: Secret::new(client_merchant_reference_id.to_string()),
            merchant_payment_method_route_id: Secret::new(
                merchant_payment_method_route_id.to_string(),
            ),
        })
    }
}

impl TryFrom<&ConnectorSpecificConfig> for PeachpaymentsAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Peachpayments {
                api_key,
                tenant_id,
                client_merchant_reference_id,
                merchant_payment_method_route_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                tenant_id: tenant_id.to_owned(),
                client_merchant_reference_id: client_merchant_reference_id.clone(),
                merchant_payment_method_route_id: merchant_payment_method_route_id.clone(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PeachpaymentsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::PeachpaymentsAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PeachpaymentsRouterData<
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
            return Err(IntegrationError::NotSupported {
                message: "3DS payments".to_string(),
                connector: "peachpayments",
                context: Default::default(),
            }
            .into());
        }

        let auth = PeachpaymentsAuthType::try_from(&item.router_data.connector_config)?;
        let connector_meta_data = PeachpaymentsConnectorMetadataObject {
            client_merchant_reference_id: auth.client_merchant_reference_id.ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "client_merchant_reference_id",
                    context: Default::default(),
                },
            )?,
            merchant_payment_method_route_id: auth.merchant_payment_method_route_id.ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "merchant_payment_method_route_id",
                    context: Default::default(),
                },
            )?,
        };

        let transaction_data = match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card_info) => {
                requests::PeachpaymentsTransactionData::Card(requests::PeachpaymentsCardData {
                    merchant_information: requests::PeachpaymentsMerchantInformation {
                        client_merchant_reference_id: connector_meta_data
                            .client_merchant_reference_id,
                    },
                    routing_reference: requests::PeachpaymentsRoutingReference {
                        merchant_payment_method_route_id: connector_meta_data
                            .merchant_payment_method_route_id,
                    },
                    card: requests::PeachpaymentsCardDetails {
                        pan: card_info.card_number.clone(),
                        cardholder_name: card_info.card_holder_name.clone(),
                        expiry_year: Some(
                            card_info.get_card_expiry_year_2_digit().change_context(
                                IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                },
                            )?,
                        ),
                        expiry_month: Some(card_info.card_exp_month),
                        cvv: Some(card_info.card_cvc),
                        eci: None,
                    },
                    amount: requests::PeachpaymentsAmount {
                        amount: item.router_data.request.minor_amount,
                        currency_code: item.router_data.request.currency,
                        display_amount: None,
                    },
                    rrn: item.router_data.request.merchant_order_id.clone(),
                    pre_auth_inc_ext_capture_flow: item
                        .router_data
                        .request
                        .capture_method
                        .and_then(|cm| {
                            if cm == common_enums::CaptureMethod::Manual {
                                Some(requests::PeachpaymentsPreAuthFlow {
                                    dcc_mode: requests::DccMode::NoDcc,
                                    txn_ref_nr: item
                                        .router_data
                                        .resource_common_data
                                        .connector_request_reference_id
                                        .clone(),
                                })
                            } else {
                                None
                            }
                        }),
                    cof_data: None,
                })
            }
            PaymentMethodData::NetworkToken(token_data) => {
                requests::PeachpaymentsTransactionData::NetworkToken(
                    requests::PeachpaymentsNetworkTokenData {
                        merchant_information: requests::PeachpaymentsMerchantInformation {
                            client_merchant_reference_id: connector_meta_data
                                .client_merchant_reference_id,
                        },
                        routing_reference: requests::PeachpaymentsRoutingReference {
                            merchant_payment_method_route_id: connector_meta_data
                                .merchant_payment_method_route_id,
                        },
                        network_token_data: requests::PeachpaymentsNetworkTokenDetails {
                            token: Secret::new(token_data.token_number.peek().clone()),
                            expiry_year: token_data
                                .get_token_expiry_year_2_digit()
                                .change_context(IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                })?,
                            expiry_month: token_data.token_exp_month,
                            cryptogram: token_data.token_cryptogram,
                            eci: token_data.eci,
                            scheme: token_data
                                .card_network
                                .map(requests::CardNetworkLowercase::try_from)
                                .transpose()
                                .change_context(IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                })?,
                        },
                        amount: requests::PeachpaymentsAmount {
                            amount: item.router_data.request.minor_amount,
                            currency_code: item.router_data.request.currency,
                            display_amount: None,
                        },
                        cof_data: requests::PeachpaymentsCofData::default(),
                        rrn: item.router_data.request.merchant_order_id.clone(),
                        pre_auth_inc_ext_capture_flow: None,
                    },
                )
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Payment method not supported".to_string(),
                    connector: "peachpayments",
                    context: Default::default(),
                }
                .into());
            }
        };

        Ok(Self {
            payment_method: requests::PeachpaymentsPaymentMethod::EcommerceCardPaymentOnly,
            reference_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            ecommerce_card_payment_only_transaction_data: transaction_data,
            pos_data: None,
            send_date_time: OffsetDateTime::now_utc()
                .format(&Iso8601::DEFAULT)
                .map_err(|_| IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::PeachpaymentsPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<responses::PeachpaymentsPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = match item.response {
            responses::PeachpaymentsPaymentsResponse::Response(data) => {
                let status: AttemptStatus = data.transaction_result.clone().into();
                let response = if status == AttemptStatus::Failure {
                    Err(ErrorResponse {
                        code: get_error_code(data.response_code.as_ref()),
                        message: get_error_message(data.response_code.as_ref()),
                        reason: Some(get_error_message(data.response_code.as_ref())),
                        status_code: item.http_code,
                        attempt_status: Some(status),
                        connector_transaction_id: Some(data.transaction_id.clone()),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    })
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(data.transaction_id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    })
                };
                (status, response)
            }
            responses::PeachpaymentsPaymentsResponse::WebhookResponse(webhook) => {
                get_webhook_response(*webhook, item.http_code)?
            }
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

impl TryFrom<ResponseRouterData<responses::PeachpaymentsSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<responses::PeachpaymentsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status: AttemptStatus = item.response.transaction_result.into();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.transaction_id),
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PeachpaymentsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for requests::PeachpaymentsCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PeachpaymentsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: requests::PeachpaymentsAmount {
                amount: item.router_data.request.minor_amount_to_capture,
                currency_code: item.router_data.request.currency,
                display_amount: None,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<responses::PeachpaymentsCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<responses::PeachpaymentsCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status: AttemptStatus = item.response.transaction_result.into();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.transaction_id),
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PeachpaymentsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for requests::PeachpaymentsVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PeachpaymentsRouterData<
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
            amount: requests::PeachpaymentsAmount {
                amount,
                currency_code: currency,
                display_amount: None,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<responses::PeachpaymentsVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<responses::PeachpaymentsVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status: AttemptStatus = item.response.transaction_result.into();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.transaction_id),
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PeachpaymentsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for requests::PeachpaymentsRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PeachpaymentsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            reference_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            ecommerce_card_payment_only_transaction_data:
                requests::PeachpaymentsRefundTransactionData {
                    amount: requests::PeachpaymentsAmount {
                        amount: MinorUnit::new(item.router_data.request.refund_amount),
                        currency_code: item.router_data.request.currency,
                        display_amount: None,
                    },
                },
            pos_data: None,
        })
    }
}

impl TryFrom<ResponseRouterData<responses::PeachpaymentsRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<responses::PeachpaymentsRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = item.response.transaction_result.into();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<responses::PeachpaymentsRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<responses::PeachpaymentsRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = item.response.transaction_result.into();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl From<responses::PeachpaymentsPaymentStatus> for AttemptStatus {
    fn from(item: responses::PeachpaymentsPaymentStatus) -> Self {
        match item {
            responses::PeachpaymentsPaymentStatus::Pending
            | responses::PeachpaymentsPaymentStatus::Authorized
            | responses::PeachpaymentsPaymentStatus::Approved => Self::Authorized,
            responses::PeachpaymentsPaymentStatus::Declined
            | responses::PeachpaymentsPaymentStatus::Failed => Self::Failure,
            responses::PeachpaymentsPaymentStatus::Voided
            | responses::PeachpaymentsPaymentStatus::Reversed => Self::Voided,
            responses::PeachpaymentsPaymentStatus::ThreedsRequired => Self::AuthenticationPending,
            responses::PeachpaymentsPaymentStatus::ApprovedConfirmed
            | responses::PeachpaymentsPaymentStatus::Successful => Self::Charged,
        }
    }
}

impl From<responses::PeachpaymentsRefundStatus> for RefundStatus {
    fn from(item: responses::PeachpaymentsRefundStatus) -> Self {
        match item {
            responses::PeachpaymentsRefundStatus::ApprovedConfirmed => Self::Success,
            responses::PeachpaymentsRefundStatus::Failed
            | responses::PeachpaymentsRefundStatus::Declined => Self::Failure,
        }
    }
}

impl TryFrom<common_enums::CardNetwork> for requests::CardNetworkLowercase {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(card_network: common_enums::CardNetwork) -> Result<Self, Self::Error> {
        match card_network {
            common_enums::CardNetwork::Visa => Ok(Self::Visa),
            common_enums::CardNetwork::Mastercard => Ok(Self::Mastercard),
            common_enums::CardNetwork::AmericanExpress => Ok(Self::Amex),
            common_enums::CardNetwork::Discover => Ok(Self::Discover),
            common_enums::CardNetwork::JCB => Ok(Self::Jcb),
            common_enums::CardNetwork::DinersClub => Ok(Self::Diners),
            common_enums::CardNetwork::CartesBancaires => Ok(Self::CartesBancaires),
            common_enums::CardNetwork::UnionPay => Ok(Self::UnionPay),
            common_enums::CardNetwork::Interac => Ok(Self::Interac),
            common_enums::CardNetwork::RuPay => Ok(Self::RuPay),
            common_enums::CardNetwork::Maestro => Ok(Self::Maestro),
            common_enums::CardNetwork::Star => Ok(Self::Star),
            common_enums::CardNetwork::Pulse => Ok(Self::Pulse),
            common_enums::CardNetwork::Accel => Ok(Self::Accel),
            common_enums::CardNetwork::Nyce => Ok(Self::Nyce),
        }
    }
}

impl Default for requests::CardOnFileData {
    fn default() -> Self {
        Self {
            _type: requests::CofType::Adhoc,
            source: requests::CofSource::Cit,
            mode: requests::CofMode::Initial,
        }
    }
}

impl Default for requests::PeachpaymentsCofData {
    fn default() -> Self {
        Self {
            cof_type: requests::CofType::Adhoc,
            source: requests::CofSource::Cit,
            mode: requests::CofMode::Initial,
        }
    }
}
