use common_enums::enums;
use common_utils::types::FloatMajorUnit;
use domain_types::errors::{ConnectorError, IntegrationError};
use domain_types::{
    connector_flow::{RepeatPayment, SetupMandate},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    payment_method_data::{PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{
    connectors::powertranz::{PowertranzAmountConvertor, PowertranzRouterData},
    types::ResponseRouterData,
};

// ============================================================================
// Auth Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowertranzAuthType {
    pub power_tranz_id: Secret<String>,
    pub power_tranz_password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PowertranzAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Powertranz {
                power_tranz_id,
                power_tranz_password,
                ..
            } => Ok(Self {
                power_tranz_id: power_tranz_id.clone(),
                power_tranz_password: power_tranz_password.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// ============================================================================
// Payment Request Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzPaymentsRequest<T: PaymentMethodDataTypes> {
    pub transaction_identifier: String,
    pub total_amount: FloatMajorUnit,
    pub currency_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure: Option<bool>,
    pub source: PowertranzSource<T>,
    pub order_identifier: String,
    pub extended_data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzSource<T: PaymentMethodDataTypes> {
    pub cardholder_name: Secret<String>,
    pub card_pan: RawCardNumber<T>,
    pub card_cvv: Secret<String>,
    pub card_expiration: Secret<String>,
}

// Type definition for Capture, Void, Refund Request
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzBaseRequest {
    pub transaction_identifier: String,
    pub total_amount: Option<FloatMajorUnit>,
    pub refund: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzCaptureRequest {
    #[serde(flatten)]
    pub base: PowertranzBaseRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzVoidRequest {
    #[serde(flatten)]
    pub base: PowertranzBaseRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzRefundRequest {
    #[serde(flatten)]
    pub base: PowertranzBaseRequest,
}

// ============================================================================
// Payment Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzPaymentsResponse {
    pub transaction_type: u8,
    /// Approved field is present in authorize/capture/void responses but not in sync responses
    #[serde(default)]
    pub approved: Option<bool>,
    pub transaction_identifier: String,
    #[serde(rename = "IsoResponseCode")]
    pub iso_response_code: String,
    pub response_message: String,
    /// Authorization code (present in sync responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_code: Option<String>,
    /// RRN - Retrieval Reference Number (present in sync responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrn: Option<String>,
    pub errors: Option<Vec<PowertranzError>>,
}

pub type PowertranzPaymentsSyncResponse = PowertranzPaymentsResponse;
pub type PowertranzCaptureResponse = PowertranzPaymentsResponse;
pub type PowertranzVoidResponse = PowertranzPaymentsResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzRefundResponse {
    pub transaction_type: u8,
    /// Approved field is present in refund responses but not in sync responses
    #[serde(default)]
    pub approved: Option<bool>,
    pub transaction_identifier: String,
    #[serde(rename = "IsoResponseCode")]
    pub iso_response_code: String,
    pub response_message: String,
    pub errors: Option<Vec<PowertranzError>>,
}

pub type PowertranzRSyncResponse = PowertranzRefundResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzError {
    pub code: String,
    pub message: String,
}

// ============================================================================
// Error Response Types
// ============================================================================

/// PowerTranz ISO response codes that indicate success
/// Reference: Hyperswitch powertranz implementation
const ISO_SUCCESS_CODES: [&str; 7] = [
    "00",  // Approved or completed successfully
    "3D0", // 3D Secure authentication successful
    "3D1", // 3D Secure authentication attempted
    "HP0", // HostedPay success
    "TK0", // Token success
    "SP4", // Split payment success
    "FC0", // Fraud check success
];

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowertranzErrorResponse {
    pub errors: Vec<PowertranzError>,
}

/// Determine payment status from PowerTranz response
///
/// Maps PowerTranz transaction type and approval status to payment status
/// Transaction types:
/// 1 = Auth, 2 = Sale, 3 = Capture, 4 = Void, 5 = Refund
fn get_payment_status(
    transaction_type: u8,
    approved: Option<bool>,
    iso_response_code: &str,
) -> enums::AttemptStatus {
    use enums::AttemptStatus;

    // Determine if transaction is approved
    let is_approved = approved.unwrap_or_else(|| ISO_SUCCESS_CODES.contains(&iso_response_code));

    // Check if this is a 3DS flow
    let is_3ds = iso_response_code.starts_with("3D");

    match transaction_type {
        // Auth
        1 => match is_approved {
            true => AttemptStatus::Authorized,
            false => match is_3ds {
                true => AttemptStatus::AuthenticationPending,
                false => AttemptStatus::Failure,
            },
        },
        // Sale
        2 => match is_approved {
            true => AttemptStatus::Charged,
            false => match is_3ds {
                true => AttemptStatus::AuthenticationPending,
                false => AttemptStatus::Failure,
            },
        },
        // Capture
        3 => match is_approved {
            true => AttemptStatus::Charged,
            false => AttemptStatus::Failure,
        },
        // Void
        4 => match is_approved {
            true => AttemptStatus::Voided,
            false => AttemptStatus::VoidFailed,
        },
        // Refund
        5 => match is_approved {
            true => AttemptStatus::AutoRefunded,
            false => AttemptStatus::Failure,
        },
        // Risk Management or other
        _ => match is_approved {
            true => AttemptStatus::Pending,
            false => AttemptStatus::Failure,
        },
    }
}

pub fn build_powertranz_error_response(
    errors: &Option<Vec<PowertranzError>>,
    iso_response_code: &str,
    response_message: &str,
    status_code: u16,
) -> domain_types::router_data::ErrorResponse {
    use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};

    if let Some(errors) = errors {
        if !errors.is_empty() {
            let first_error = errors.first();
            return domain_types::router_data::ErrorResponse {
                status_code,
                code: first_error
                    .map(|e| e.code.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: first_error
                    .map(|e| e.message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: Some(
                    errors
                        .iter()
                        .map(|error| format!("{} : {}", error.code, error.message))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            };
        }
    }

    // ISO Error Case
    if !ISO_SUCCESS_CODES.contains(&iso_response_code) {
        return domain_types::router_data::ErrorResponse {
            status_code,
            code: iso_response_code.to_string(),
            message: response_message.to_string(),
            reason: Some(response_message.to_string()),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        };
    }

    domain_types::router_data::ErrorResponse {
        status_code,
        code: NO_ERROR_CODE.to_string(),
        message: NO_ERROR_MESSAGE.to_string(),
        reason: None, // or Some("Success".into())
        attempt_status: None,
        connector_transaction_id: None,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    }
}

// ============================================================================
// Request Transformers
// ============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PowertranzRouterData<
            RouterDataV2<
                domain_types::connector_flow::Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PowertranzPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PowertranzRouterData<
            RouterDataV2<
                domain_types::connector_flow::Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request_data = &item.router_data.request;
        let amount =
            PowertranzAmountConvertor::convert(request_data.amount, request_data.currency)?;
        // Use ISO 4217 numeric code (e.g., "840" for USD)
        let currency_code = request_data.currency.iso_4217().to_string();

        match &request_data.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                let card_expiration = card_data
                    .get_card_expiry_year_month_2_digit_with_delimiter(String::new())
                    .change_context(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    })?;

                Ok(Self {
                    transaction_identifier: uuid::Uuid::new_v4().to_string(),
                    total_amount: amount,
                    currency_code,
                    three_d_secure: Some(false),
                    source: PowertranzSource {
                        cardholder_name: card_data.card_holder_name.clone().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "payment_method.card.card_holder_name",
                                context: Default::default(),
                            },
                        )?,
                        card_pan: card_data.card_number.clone(),
                        card_cvv: card_data.card_cvc.clone(),
                        card_expiration,
                    },
                    order_identifier: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    extended_data: None,
                })
            }
            _ => Err(IntegrationError::NotSupported {
                message: "Payment method".to_string(),
                connector: "powertranz",
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PowertranzRouterData<
            RouterDataV2<
                domain_types::connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PowertranzCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PowertranzRouterData<
            RouterDataV2<
                domain_types::connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request_data = &item.router_data.request;
        let amount = PowertranzAmountConvertor::convert(
            common_utils::types::MinorUnit::new(request_data.amount_to_capture),
            request_data.currency,
        )?;

        Ok(Self {
            base: PowertranzBaseRequest {
                transaction_identifier: request_data
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    })?,
                total_amount: Some(amount),
                refund: None,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PowertranzRouterData<
            RouterDataV2<
                domain_types::connector_flow::Void,
                PaymentFlowData,
                PaymentVoidData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PowertranzVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PowertranzRouterData<
            RouterDataV2<
                domain_types::connector_flow::Void,
                PaymentFlowData,
                PaymentVoidData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            base: PowertranzBaseRequest {
                transaction_identifier: item.router_data.request.connector_transaction_id.clone(),
                total_amount: None,
                refund: None,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PowertranzRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    > for PowertranzRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PowertranzRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request_data = &item.router_data.request;
        let amount = PowertranzAmountConvertor::convert(
            common_utils::types::MinorUnit::new(request_data.refund_amount),
            request_data.currency,
        )?;

        Ok(Self {
            base: PowertranzBaseRequest {
                transaction_identifier: request_data.connector_transaction_id.clone(),
                total_amount: Some(amount),
                refund: Some(true),
            },
        })
    }
}

// ============================================================================
// Response Transformers
// ============================================================================

impl<T: PaymentMethodDataTypes, F> TryFrom<ResponseRouterData<PowertranzPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PowertranzPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // Determine payment status from transaction type and ISO code
        let status = get_payment_status(
            response.transaction_type,
            response.approved,
            &response.iso_response_code,
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transaction_identifier),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PowertranzPaymentsSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PowertranzPaymentsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // Determine payment status from transaction type and ISO code
        let status = get_payment_status(
            response.transaction_type,
            response.approved,
            &response.iso_response_code,
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transaction_identifier),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PowertranzCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PowertranzCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // Determine payment status
        let status = get_payment_status(
            response.transaction_type,
            response.approved,
            &response.iso_response_code,
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transaction_identifier),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PowertranzVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PowertranzVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // Determine payment status
        let status = get_payment_status(
            response.transaction_type,
            response.approved,
            &response.iso_response_code,
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transaction_identifier),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PowertranzRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PowertranzRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        // Determine approval status: use approved field if present, otherwise check ISO code
        let is_approved = response
            .approved
            .unwrap_or_else(|| ISO_SUCCESS_CODES.contains(&response.iso_response_code.as_str()));
        let refund_status = if is_approved {
            enums::RefundStatus::Success
        } else {
            enums::RefundStatus::Failure
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.transaction_identifier.clone(),
                refund_status,
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PowertranzRSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PowertranzRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        // Determine approval status: use approved field if present, otherwise check ISO code
        let is_approved = response
            .approved
            .unwrap_or_else(|| ISO_SUCCESS_CODES.contains(&response.iso_response_code.as_str()));
        let refund_status = if is_approved {
            enums::RefundStatus::Success
        } else {
            enums::RefundStatus::Failure
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.transaction_identifier.clone(),
                refund_status,
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

// ============================================================================
// SetupMandate Flow
// ============================================================================
//
// PowerTranz does not expose a dedicated mandate-setup endpoint. The
// idiomatic card-on-file / mandate pattern is to issue an auth-only
// (transaction_type = 1) request against the `/auth` endpoint. A
// zero-dollar (or caller-supplied) auth is treated as a verification and
// the returned `transaction_identifier` is surfaced as the
// `connector_mandate_id` used for subsequent RepeatPayment (MIT) calls.
//
// The request shape mirrors `PowertranzPaymentsRequest<T>` — same source
// (card) + transaction identifier contract — so downstream callers can
// replay the stored transaction_identifier on MIT. For zero-amount
// verification we fall back to 0 when the caller does not supply an
// amount.

/// SetupMandate request – identical wire shape to the standard
/// PowerTranz payment request (the `/auth` endpoint doubles as the
/// card-on-file verification endpoint for the zero/low-amount case).
pub type PowertranzSetupMandateRequest<T> = PowertranzPaymentsRequest<T>;

/// SetupMandate response – reuses the standard PowerTranz payments
/// response (transaction_type = 1 on success).
pub type PowertranzSetupMandateResponse = PowertranzPaymentsResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PowertranzRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PowertranzSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PowertranzRouterData<
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
        let request_data = &router_data.request;

        // ISO 4217 numeric currency code (e.g., "840" for USD) — same
        // convention as the Authorize flow.
        let currency_code = request_data.currency.iso_4217().to_string();

        // Prefer caller-supplied amount, fall back to 0 for a zero-dollar
        // verification. PowerTranz `/auth` accepts a zero-amount auth as
        // the card-on-file verification primitive.
        let amount = PowertranzAmountConvertor::convert(
            request_data
                .minor_amount
                .unwrap_or(common_utils::types::MinorUnit::new(0)),
            request_data.currency,
        )?;

        match &request_data.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                let card_expiration = card_data
                    .get_card_expiry_year_month_2_digit_with_delimiter(String::new())
                    .change_context(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    })?;

                // Cardholder name: prefer card-supplied holder name, fall
                // back to SetupMandateRequestData.customer_name.
                let cardholder_name = card_data
                    .card_holder_name
                    .clone()
                    .or_else(|| {
                        request_data
                            .customer_name
                            .as_ref()
                            .map(|name| Secret::new(name.clone()))
                    })
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payment_method.card.card_holder_name",
                        context: Default::default(),
                    })?;

                Ok(Self {
                    transaction_identifier: uuid::Uuid::new_v4().to_string(),
                    total_amount: amount,
                    currency_code,
                    three_d_secure: Some(false),
                    source: PowertranzSource {
                        cardholder_name,
                        card_pan: card_data.card_number.clone(),
                        card_cvv: card_data.card_cvc.clone(),
                        card_expiration,
                    },
                    order_identifier: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    extended_data: None,
                })
            }
            _ => Err(IntegrationError::NotSupported {
                message: "Payment method not supported for SetupMandate".to_string(),
                connector: "powertranz",
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PowertranzSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PowertranzSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // Reuse the shared status-mapping logic. Transaction type 1 (Auth)
        // + approved -> Authorized.
        let mut status = get_payment_status(
            response.transaction_type,
            response.approved,
            &response.iso_response_code,
        );

        // For zero-amount mandate setup, treat Authorized as Charged so
        // the attempt reaches a terminal state for downstream consumers.
        // Subsequent RepeatPayment (MIT) calls can replay the stored
        // transaction_identifier.
        if status == enums::AttemptStatus::Authorized {
            status = enums::AttemptStatus::Charged;
        }

        // The PowerTranz transaction_identifier IS the connector_mandate_id
        // used for subsequent RepeatPayment (MIT) calls.
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(response.transaction_identifier.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    response.transaction_identifier.clone(),
                ),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ============================================================================
// RepeatPayment Flow (MIT)
// ============================================================================
//
// PowerTranz does not expose a dedicated MIT / stored-credential endpoint.
// Subsequent merchant-initiated charges against a previously stored
// card-on-file are issued as a fresh `/sale` request using the caller-
// supplied card data. The initial `transaction_identifier` (captured as
// `connector_mandate_id` during SetupRecurring) is replayed in the
// OrderIdentifier for traceability. A new `TransactionIdentifier` is minted
// per replay so PowerTranz can uniquely track each MIT attempt.

pub type PowertranzRepeatPaymentRequest<T> = PowertranzPaymentsRequest<T>;
pub type PowertranzRepeatPaymentResponse = PowertranzPaymentsResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PowertranzRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PowertranzRepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PowertranzRouterData<
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
        let request_data = &router_data.request;

        let currency_code = request_data.currency.iso_4217().to_string();
        let amount =
            PowertranzAmountConvertor::convert(request_data.minor_amount, request_data.currency)?;

        let mandate_order_id = match &request_data.mandate_reference {
            MandateReferenceId::ConnectorMandateId(m) => m.get_connector_mandate_id(),
            _ => None,
        }
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "connector_mandate_id",
            context: Default::default(),
        })?;

        match &request_data.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                let card_expiration = card_data
                    .get_card_expiry_year_month_2_digit_with_delimiter(String::new())
                    .change_context(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    })?;

                let cardholder_name = card_data.card_holder_name.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "payment_method.card.card_holder_name",
                        context: Default::default(),
                    },
                )?;

                Ok(Self {
                    transaction_identifier: uuid::Uuid::new_v4().to_string(),
                    total_amount: amount,
                    currency_code,
                    three_d_secure: Some(false),
                    source: PowertranzSource {
                        cardholder_name,
                        card_pan: card_data.card_number.clone(),
                        card_cvv: card_data.card_cvc.clone(),
                        card_expiration,
                    },
                    order_identifier: mandate_order_id,
                    extended_data: None,
                })
            }
            _ => Err(IntegrationError::NotSupported {
                message: "Payment method not supported for RepeatPayment".to_string(),
                connector: "powertranz",
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PowertranzRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PowertranzRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let status = get_payment_status(
            response.transaction_type,
            response.approved,
            &response.iso_response_code,
        );

        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(response.transaction_identifier.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    response.transaction_identifier.clone(),
                ),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}
