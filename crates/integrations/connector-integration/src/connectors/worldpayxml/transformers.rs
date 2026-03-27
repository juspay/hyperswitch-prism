use std::fmt::Debug;

use common_enums::{AttemptStatus, CaptureMethod, RefundStatus};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::Serialize;

use super::{
    requests::{self, WorldpayxmlAction},
    responses::{self, WorldpayxmlLastEvent},
    WorldpayxmlRouterData,
};
use crate::types::ResponseRouterData;

const API_VERSION: &str = "1.4";

#[derive(Debug, Clone)]
pub struct WorldpayxmlAuthType {
    pub api_username: Secret<String>,
    pub api_password: Secret<String>,
    pub merchant_code: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for WorldpayxmlAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Worldpayxml {
                api_username,
                api_password,
                merchant_code,
                ..
            } => Ok(Self {
                api_username: api_username.to_owned(),
                api_password: api_password.to_owned(),
                merchant_code: merchant_code.to_owned(),
            }),
            _ => Err(errors::ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

// Helper function to get currency exponent

const DEFAULT_CARD_HOLDER_NAME: &str = "Card Holder";
const DEFAULT_PAYMENT_DESCRIPTION: &str = "Payment";

// Helper function to get payment method XML element
fn get_worldpayxml_payment_method<T>(
    payment_method_data: &PaymentMethodData<T>,
    card: &Card<T>,
    billing_address: Option<&requests::WorldpayxmlBillingAddress>,
) -> Result<requests::WorldpayxmlPaymentMethod, error_stack::Report<errors::ConnectorError>>
where
    T: PaymentMethodDataTypes,
{
    match payment_method_data {
        PaymentMethodData::Card(_) => {
            // Convert 2-digit year to 4-digit year (e.g., "30" -> "2030")
            let year_str = card.card_exp_year.peek();
            let formatted_year = if year_str.len() == 2 {
                Secret::new(format!("20{}", year_str))
            } else {
                card.card_exp_year.clone()
            };

            // Use card_holder_name from card data, or construct from billing address, or use a default
            let card_holder_name = if let Some(ref holder_name) = card.card_holder_name {
                holder_name.clone()
            } else if let Some(billing_addr) = billing_address {
                // Construct from billing address first_name and last_name
                let first_name = billing_addr
                    .address
                    .first_name
                    .as_ref()
                    .map(|n| n.peek().clone())
                    .unwrap_or_default();
                let last_name = billing_addr
                    .address
                    .last_name
                    .as_ref()
                    .map(|n| n.peek().clone())
                    .unwrap_or_default();

                if !first_name.is_empty() || !last_name.is_empty() {
                    Secret::new(format!("{} {}", first_name, last_name).trim().to_string())
                } else {
                    Secret::new(DEFAULT_CARD_HOLDER_NAME.to_string())
                }
            } else {
                Secret::new(DEFAULT_CARD_HOLDER_NAME.to_string())
            };

            let card_data = requests::WorldpayxmlCard {
                card_number: Secret::new(card.card_number.peek().to_string()),
                expiry_date: requests::WorldpayxmlExpiryDate {
                    date: requests::WorldpayxmlDate {
                        month: card.card_exp_month.clone(),
                        year: formatted_year,
                    },
                },
                card_holder_name: Some(card_holder_name),
                cvc: card.card_cvc.clone(),
            };

            // Map card network to specific payment method type
            match card.card_network.as_ref() {
                Some(network) => match network {
                    common_enums::CardNetwork::Visa => {
                        Ok(requests::WorldpayxmlPaymentMethod::Visa(card_data))
                    }
                    common_enums::CardNetwork::Mastercard => {
                        Ok(requests::WorldpayxmlPaymentMethod::Ecmc(card_data))
                    }
                    _ => Ok(requests::WorldpayxmlPaymentMethod::Card(card_data)),
                },
                None => Ok(requests::WorldpayxmlPaymentMethod::Card(card_data)),
            }
        }
        _ => Err(errors::ConnectorError::NotSupported {
            message: "Selected payment method".to_string(),
            connector: "worldpayxml",
        }
        .into()),
    }
}

// Authorize flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::WorldpayxmlPaymentsRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: WorldpayxmlRouterData<
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
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Determine if manual capture
        let is_manual_capture = router_data.request.capture_method == Some(CaptureMethod::Manual)
            || router_data.request.capture_method == Some(CaptureMethod::ManualMultiple);

        // Extract billing address first (needed for payment method)
        let billing_address = router_data
            .resource_common_data
            .address
            .get_payment_billing()
            .and_then(|billing| {
                billing
                    .address
                    .as_ref()
                    .map(|addr| requests::WorldpayxmlBillingAddress {
                        address: requests::WorldpayxmlAddress {
                            first_name: addr.first_name.clone(),
                            last_name: addr.last_name.clone(),
                            address1: addr.line1.clone(),
                            address2: addr.line2.clone(),
                            address3: addr.line3.clone(),
                            postal_code: addr.zip.clone(),
                            city: addr.city.clone().map(|c| c.expose()),
                            state: addr.state.clone(),
                            country_code: addr.country,
                        },
                    })
            });

        // Get payment method
        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => get_worldpayxml_payment_method(
                &router_data.request.payment_method_data,
                card,
                billing_address.as_ref(),
            )?,
            _ => {
                return Err(errors::ConnectorError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "worldpayxml",
                }
                .into())
            }
        };

        // Convert amount using the connector's amount converter
        let converted_amount = super::WorldpayxmlAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            submit: requests::WorldpayxmlSubmit {
                order: requests::WorldpayxmlOrder {
                    order_code: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    capture_delay: if is_manual_capture {
                        "OFF".to_string()
                    } else {
                        "0".to_string().to_string()
                    },
                    description: router_data
                        .resource_common_data
                        .description
                        .clone()
                        .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
                    amount: requests::WorldpayxmlAmount {
                        value: converted_amount,
                        currency_code: router_data.request.currency,
                        exponent: if router_data.request.currency.is_three_decimal_currency() {
                            "3".to_string()
                        } else if router_data.request.currency.is_zero_decimal_currency() {
                            "0".to_string()
                        } else {
                            "2".to_string()
                        },
                    },
                    payment_details: requests::WorldpayxmlPaymentDetails {
                        action: if is_manual_capture {
                            WorldpayxmlAction::Authorise
                        } else {
                            WorldpayxmlAction::Sale
                        },
                        payment_method,
                    },
                    shopper: requests::WorldpayxmlShopper {
                        shopper_email_address: router_data.request.email.clone(),
                        browser: router_data
                            .request
                            .browser_info
                            .as_ref()
                            .map(|browser_info| requests::WorldpayxmlBrowser {
                                accept_header: browser_info.accept_header.clone(),
                                user_agent_header: browser_info.user_agent.clone(),
                                http_accept_language: browser_info.accept_language.clone(),
                                time_zone: browser_info.time_zone,
                                browser_language: browser_info.language.clone(),
                                browser_java_enabled: browser_info.java_enabled,
                                browser_java_script_enabled: browser_info.java_script_enabled,
                                browser_colour_depth: browser_info.color_depth.map(u32::from),
                                browser_screen_height: browser_info.screen_height,
                                browser_screen_width: browser_info.screen_width,
                            }),
                    },
                    billing_address,
                },
            },
        })
    }
}

// Capture flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlCaptureRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_transaction_id from request
        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::ConnectorError::MissingConnectorTransactionID)?;

        // Convert amount using the connector's amount converter
        let converted_amount = super::WorldpayxmlAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            modify: requests::WorldpayxmlModify {
                order_modification: requests::WorldpayxmlOrderModification {
                    order_code: connector_transaction_id.clone(),
                    capture: requests::WorldpayxmlCapture {
                        amount: requests::WorldpayxmlAmount {
                            value: converted_amount,
                            currency_code: router_data.request.currency,
                            exponent: if router_data.request.currency.is_three_decimal_currency() {
                                "3".to_string()
                            } else if router_data.request.currency.is_zero_decimal_currency() {
                                "0".to_string()
                            } else {
                                "2".to_string()
                            },
                        },
                    },
                },
            },
        })
    }
}

// Void flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlVoidRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_transaction_id from request
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            modify: requests::WorldpayxmlVoidModify {
                order_modification: requests::WorldpayxmlVoidOrderModification {
                    order_code: connector_transaction_id,
                    cancel: requests::WorldpayxmlCancel {},
                },
            },
        })
    }
}

// Refund flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlRefundRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_transaction_id from request
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        // Convert refund amount using the connector's amount converter
        let converted_amount = super::WorldpayxmlAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            modify: requests::WorldpayxmlRefundModify {
                order_modification: requests::WorldpayxmlRefundOrderModification {
                    order_code: connector_transaction_id,
                    refund: requests::WorldpayxmlRefund {
                        amount: requests::WorldpayxmlAmount {
                            value: converted_amount,
                            currency_code: router_data.request.currency,
                            exponent: if router_data.request.currency.is_three_decimal_currency() {
                                "3".to_string()
                            } else if router_data.request.currency.is_zero_decimal_currency() {
                                "0".to_string()
                            } else {
                                "2".to_string()
                            },
                        },
                    },
                },
            },
        })
    }
}

// PSync flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlPSyncRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_transaction_id from request
        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::ConnectorError::MissingConnectorTransactionID)?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            inquiry: requests::WorldpayxmlInquiry {
                order_inquiry: requests::WorldpayxmlOrderInquiry {
                    order_code: connector_transaction_id,
                },
            },
        })
    }
}

// RSync flow transformers - REUSE PSync request structure via type alias
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlRSyncRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_refund_id from request
        // This could be either the connector_refund_id OR the original connector_transaction_id
        let order_code = router_data.request.connector_refund_id.clone();

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            inquiry: requests::WorldpayxmlInquiry {
                order_inquiry: requests::WorldpayxmlOrderInquiry { order_code },
            },
        })
    }
}

// Helper function to map lastEvent to AttemptStatus
fn map_worldpayxml_authorize_status(
    last_event: &WorldpayxmlLastEvent,
    is_auto_capture: bool,
    capture_sync_status: Option<&AttemptStatus>,
) -> AttemptStatus {
    match last_event {
        WorldpayxmlLastEvent::Authorised => {
            if is_auto_capture {
                AttemptStatus::Pending
            } else {
                // Check if we're in CaptureInitiated or VoidInitiated state
                match capture_sync_status {
                    Some(AttemptStatus::CaptureInitiated) => AttemptStatus::CaptureInitiated,
                    Some(AttemptStatus::VoidInitiated) => AttemptStatus::VoidInitiated,
                    _ => AttemptStatus::Authorized,
                }
            }
        }
        WorldpayxmlLastEvent::Refused => AttemptStatus::Failure,
        WorldpayxmlLastEvent::Cancelled => AttemptStatus::Voided,
        WorldpayxmlLastEvent::Captured => AttemptStatus::Charged,
        WorldpayxmlLastEvent::SentForRefund => AttemptStatus::Pending,
        WorldpayxmlLastEvent::Refunded => AttemptStatus::Charged,
        WorldpayxmlLastEvent::RefundFailed => AttemptStatus::Failure,
        WorldpayxmlLastEvent::Expired => AttemptStatus::Failure,
        WorldpayxmlLastEvent::Error => AttemptStatus::Failure,
    }
}

// Helper function to map lastEvent to RefundStatus
fn map_worldpayxml_refund_status(last_event: &WorldpayxmlLastEvent) -> RefundStatus {
    match last_event {
        WorldpayxmlLastEvent::Refunded => RefundStatus::Success,
        WorldpayxmlLastEvent::SentForRefund => RefundStatus::Pending,
        WorldpayxmlLastEvent::RefundFailed => RefundStatus::Failure,
        WorldpayxmlLastEvent::Captured => RefundStatus::Pending,
        _ => RefundStatus::Pending, // Default to pending for unknown statuses
    }
}

// Helper function to parse string last_event from webhook/JSON responses
fn parse_last_event(event_str: &str) -> Result<WorldpayxmlLastEvent, errors::ConnectorError> {
    serde_json::from_str(&format!("\"{}\"", event_str))
        .map_err(|_| errors::ConnectorError::ResponseDeserializationFailed)
}

// Response transformers - Authorize
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::WorldpayxmlAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for top-level error first
        if let Some(error) = &response.reply.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract order status
        let order_status = response
            .reply
            .order_status
            .as_ref()
            .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

        // Check for error in order status
        if let Some(error) = &order_status.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(order_status.order_code.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract payment details
        let payment = order_status
            .payment
            .as_ref()
            .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

        // Determine if auto-capture
        let is_auto_capture = router_data.request.capture_method != Some(CaptureMethod::Manual)
            && router_data.request.capture_method != Some(CaptureMethod::ManualMultiple);

        // Map status from lastEvent
        let status = map_worldpayxml_authorize_status(
            &payment.last_event,
            is_auto_capture,
            Some(&router_data.resource_common_data.status),
        );

        // Build success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(order_status.order_code.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: payment
                .authorisation_id
                .as_ref()
                .map(|auth_id| auth_id.id.clone()),
            connector_response_reference_id: Some(order_status.order_code.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - Capture
impl TryFrom<ResponseRouterData<responses::WorldpayxmlCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for top-level error first
        if let Some(error) = &response.reply.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::CaptureFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::CaptureFailed),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract ok response
        let ok_response = response
            .reply
            .ok
            .as_ref()
            .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

        // Extract captureReceived
        let capture_received = &ok_response.capture_received;

        // Build success response
        // Status is CaptureInitiated (capture confirmed but not yet processed)
        // Actual completion must be verified via PSync
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(capture_received.order_code.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(capture_received.order_code.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::CaptureInitiated,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - Void
impl TryFrom<ResponseRouterData<responses::WorldpayxmlVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for top-level error first
        if let Some(error) = &response.reply.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::VoidFailed),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract ok response
        let ok_response = response
            .reply
            .ok
            .as_ref()
            .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

        // Extract cancelReceived
        let cancel_received = &ok_response.cancel_received;

        // Build success response
        // Status is VoidInitiated (cancellation confirmed but not yet processed)
        // Actual completion must be verified via PSync
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(cancel_received.order_code.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(cancel_received.order_code.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::VoidInitiated,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - PSync
impl TryFrom<ResponseRouterData<responses::WorldpayxmlTransactionResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlTransactionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Match on the response enum to handle both XML and JSON formats
        match &item.response {
            responses::WorldpayxmlTransactionResponse::Payment(xml_response) => {
                // Process XML response (same structure as Authorize)
                let response = xml_response.as_ref();

                // Check for top-level error first
                if let Some(error) = &response.reply.error {
                    return Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: AttemptStatus::Failure,
                            ..router_data.resource_common_data.clone()
                        },
                        response: Err(ErrorResponse {
                            code: error.code.clone(),
                            message: error.message.clone(),
                            reason: Some(error.message.clone()),
                            status_code: item.http_code,
                            attempt_status: Some(AttemptStatus::Failure),
                            connector_transaction_id: None,
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                        }),
                        ..router_data.clone()
                    });
                }

                // Extract order status
                let order_status = response
                    .reply
                    .order_status
                    .as_ref()
                    .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

                // Special handling: If error exists but payment is None, return current status (don't fail)
                if let Some(error) = &order_status.error {
                    if order_status.payment.is_none() {
                        // Error exists but no payment data - return current status as Pending
                        let payments_response_data = PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                order_status.order_code.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: Some(order_status.order_code.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        };

                        return Ok(Self {
                            resource_common_data: PaymentFlowData {
                                status: AttemptStatus::Pending,
                                ..router_data.resource_common_data.clone()
                            },
                            response: Ok(payments_response_data),
                            ..router_data.clone()
                        });
                    }

                    // Error exists with payment data - fail the payment
                    return Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: AttemptStatus::Failure,
                            ..router_data.resource_common_data.clone()
                        },
                        response: Err(ErrorResponse {
                            code: error.code.clone(),
                            message: error.message.clone(),
                            reason: Some(error.message.clone()),
                            status_code: item.http_code,
                            attempt_status: Some(AttemptStatus::Failure),
                            connector_transaction_id: Some(order_status.order_code.clone()),
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                        }),
                        ..router_data.clone()
                    });
                }

                // Extract payment details
                let payment = order_status
                    .payment
                    .as_ref()
                    .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

                // Determine if auto-capture from request data
                let is_auto_capture = router_data.request.capture_method
                    != Some(CaptureMethod::Manual)
                    && router_data.request.capture_method != Some(CaptureMethod::ManualMultiple);

                // Map status from lastEvent - reuse the helper function
                let status = map_worldpayxml_authorize_status(
                    &payment.last_event,
                    is_auto_capture,
                    Some(&router_data.resource_common_data.status),
                );

                // Build success response
                let payments_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        order_status.order_code.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: payment
                        .authorisation_id
                        .as_ref()
                        .map(|auth_id| auth_id.id.clone()),
                    connector_response_reference_id: Some(order_status.order_code.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Ok(payments_response_data),
                    ..router_data.clone()
                })
            }
            responses::WorldpayxmlTransactionResponse::Webhook(webhook_response) => {
                // Process JSON webhook response
                let order_code = webhook_response
                    .order_code
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());

                let last_event_str = webhook_response
                    .last_event
                    .as_ref()
                    .or(webhook_response.payment_status.as_ref())
                    .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

                // Parse string to enum
                let last_event = parse_last_event(last_event_str)?;

                // Determine if auto-capture from request data
                let is_auto_capture = router_data.request.capture_method
                    != Some(CaptureMethod::Manual)
                    && router_data.request.capture_method != Some(CaptureMethod::ManualMultiple);

                // Map status from lastEvent
                let status = map_worldpayxml_authorize_status(
                    &last_event,
                    is_auto_capture,
                    Some(&router_data.resource_common_data.status),
                );

                // Build success response
                let payments_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(order_code.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(order_code),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Ok(payments_response_data),
                    ..router_data.clone()
                })
            }
        }
    }
}

// Response transformers - Refund
impl TryFrom<ResponseRouterData<responses::WorldpayxmlRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for top-level error first
        if let Some(error) = &response.reply.error {
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract ok response
        let ok_response = response
            .reply
            .ok
            .as_ref()
            .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

        // Extract refundReceived
        let refund_received = &ok_response.refund_received;

        // Build success response
        // Status is Pending (refund initiated but not completed)
        // Actual completion must be verified via RSync
        let refunds_response_data = RefundsResponseData {
            connector_refund_id: refund_received.order_code.clone(),
            refund_status: RefundStatus::Pending,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - RSync (REUSE PSync response structure via type alias)
impl TryFrom<ResponseRouterData<responses::WorldpayxmlRsyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlRsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Match on the response enum to handle both XML and JSON formats (same as PSync)
        match &item.response {
            responses::WorldpayxmlTransactionResponse::Payment(xml_response) => {
                // Process XML response
                let response = xml_response.as_ref();

                // Check for top-level error first
                if let Some(error) = &response.reply.error {
                    return Ok(Self {
                        response: Err(ErrorResponse {
                            code: error.code.clone(),
                            message: error.message.clone(),
                            reason: Some(error.message.clone()),
                            status_code: item.http_code,
                            attempt_status: None,
                            connector_transaction_id: None,
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                        }),
                        ..router_data.clone()
                    });
                }

                // Extract order status
                let order_status = response
                    .reply
                    .order_status
                    .as_ref()
                    .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

                // Special handling: If error exists but payment is None, return Pending (don't fail)
                if let Some(_error) = &order_status.error {
                    if order_status.payment.is_none() {
                        // Error exists but no payment data - return current status as Pending
                        let refunds_response_data = RefundsResponseData {
                            connector_refund_id: order_status.order_code.clone(),
                            refund_status: RefundStatus::Pending,
                            status_code: item.http_code,
                        };

                        return Ok(Self {
                            response: Ok(refunds_response_data),
                            ..router_data.clone()
                        });
                    }
                }

                // Extract payment details
                let payment = order_status
                    .payment
                    .as_ref()
                    .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

                // Map status from lastEvent using refund status mapping
                let refund_status = map_worldpayxml_refund_status(&payment.last_event);

                // Check if refund failed and extract error details from ISO8583ReturnCode
                if refund_status == RefundStatus::Failure {
                    if let Some(return_code) = &payment.iso8583_return_code {
                        return Ok(Self {
                            response: Err(ErrorResponse {
                                code: return_code.code.clone(),
                                message: return_code.description.clone(),
                                reason: Some(return_code.description.clone()),
                                status_code: item.http_code,
                                attempt_status: None,
                                connector_transaction_id: Some(order_status.order_code.clone()),
                                network_decline_code: None,
                                network_advice_code: None,
                                network_error_message: None,
                            }),
                            ..router_data.clone()
                        });
                    }
                }

                // Build success response
                let refunds_response_data = RefundsResponseData {
                    connector_refund_id: order_status.order_code.clone(),
                    refund_status,
                    status_code: item.http_code,
                };

                Ok(Self {
                    response: Ok(refunds_response_data),
                    ..router_data.clone()
                })
            }
            responses::WorldpayxmlTransactionResponse::Webhook(webhook_response) => {
                // Process JSON webhook response
                let order_code = webhook_response
                    .order_code
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());

                let last_event_str = webhook_response
                    .last_event
                    .as_ref()
                    .or(webhook_response.payment_status.as_ref())
                    .ok_or(errors::ConnectorError::ResponseDeserializationFailed)?;

                // Parse string to enum
                let last_event = parse_last_event(last_event_str)?;

                // Map status from lastEvent using refund status mapping
                let refund_status = map_worldpayxml_refund_status(&last_event);

                // Build success response
                let refunds_response_data = RefundsResponseData {
                    connector_refund_id: order_code,
                    refund_status,
                    status_code: item.http_code,
                };

                Ok(Self {
                    response: Ok(refunds_response_data),
                    ..router_data.clone()
                })
            }
        }
    }
}
