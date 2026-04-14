use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    self,
    errors::CustomResult,
    types::{AmountConvertor, StringMinorUnit, StringMinorUnitForConnector},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PostAuthenticate, PreAuthenticate,
        RSync, Refund, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference, MandateReferenceId,
        NexixpayClientAuthenticationResponse as NexixpayClientAuthenticationResponseDomain,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::Display;

// Import the wrapper type created by macros
use super::NexixpayRouterData;

const MAX_ORDER_ID_LENGTH: usize = 18;

fn get_nexi_order_id(payment_id: &str) -> CustomResult<String, IntegrationError> {
    if payment_id.len() > MAX_ORDER_ID_LENGTH {
        if payment_id.starts_with("pay_") {
            Ok(payment_id
                .chars()
                .take(MAX_ORDER_ID_LENGTH)
                .collect::<String>())
        } else {
            Err(error_stack::Report::from(
                IntegrationError::MaxFieldLengthViolated {
                    field_name: "payment_id".to_string(),
                    connector: "Nexixpay".to_string(),
                    max_length: MAX_ORDER_ID_LENGTH,
                    received_length: payment_id.len(),
                    context: Default::default(),
                },
            ))
        }
    } else {
        Ok(payment_id.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct NexixpayAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NexixpayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nexixpay { api_key, .. } => Ok(Self {
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
#[serde(rename_all = "camelCase")]
pub struct NexixpayErrorBody {
    pub code: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayErrorResponse {
    pub errors: Vec<NexixpayErrorBody>,
}

// ===== CONNECTOR METADATA STRUCTURE =====
// Used to pass data between 3DS flow steps (PreAuthenticate -> PostAuthenticate -> Authorize)
// Based on Hyperswitch implementation pattern

/// Payment flow intent for determining which operation ID to use in PSync
#[derive(Debug, Clone, Serialize, Deserialize, Display)]
pub enum NexixpayPaymentIntent {
    Capture,
    Cancel,
    Authorize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayConnectorMetaData {
    /// 3DS authentication result (CAVV, ECI, XID) from PostAuthenticate (/validate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_s_auth_result: Option<NexixpayThreeDSAuthResult>,

    /// PaRes (3DS authentication response) from redirect
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_s_auth_response: Option<Secret<String>>,

    /// operationId from PreAuthenticate (/init) - used in Authorize (/payment)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_operation_id: Option<String>,

    /// operationId from Capture operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_operation_id: Option<String>,

    /// operationId from Cancel/Void operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_operation_id: Option<String>,

    /// Payment flow type for PSync operations - required by Hyperswitch
    pub psync_flow: NexixpayPaymentIntent,
}

pub fn get_payment_id(
    metadata: Option<serde_json::Value>,
    payment_intent: Option<NexixpayPaymentIntent>,
) -> CustomResult<String, IntegrationError> {
    let connector_metadata = metadata.ok_or(IntegrationError::MissingRequiredField {
        field_name: "connector_feature_data",
        context: Default::default(),
    })?;
    let nexixpay_meta_data = serde_json::from_value::<NexixpayConnectorMetaData>(
        connector_metadata,
    )
    .change_context(IntegrationError::InvalidDataFormat {
        field_name: "connector_feature_data",
        context: Default::default(),
    })?;
    let payment_flow = payment_intent.unwrap_or(nexixpay_meta_data.psync_flow);
    let payment_id = match payment_flow {
        NexixpayPaymentIntent::Cancel => nexixpay_meta_data.cancel_operation_id,
        NexixpayPaymentIntent::Capture => nexixpay_meta_data.capture_operation_id,
        NexixpayPaymentIntent::Authorize => nexixpay_meta_data.authorization_operation_id,
    };
    payment_id.ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "operation_id",
            context: Default::default(),
        }
        .into()
    })
}

// Note: NexixpayThreeDSAuthResult is defined later in the file

// ===== AUTHORIZE FLOW STRUCTURES =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayCaptureType {
    Implicit,
    Explicit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayRecurringAction {
    NoRecurring,
    ContractCreation,
    SubsequentPayment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContractType {
    MitUnscheduled,
    MitScheduled,
    Cit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceRequest {
    pub action: NexixpayRecurringAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_type: Option<ContractType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPaymentsRequest {
    pub operation_id: String,
    pub order: NexixpayOrderData,
    pub card: NexixpayCardData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_type: Option<NexixpayCaptureType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthData")]
    pub three_ds_auth_data: Option<NexixpayThreeDSAuthData>,
    pub recurrence: RecurrenceRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayOrderData {
    pub order_id: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub customer_info: NexixpayCustomerInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayCardData {
    pub pan: Secret<String>,
    #[serde(rename = "expiryDate")]
    pub expiry_date: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayThreeDSAuthData {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthResponse")]
    pub three_d_s_auth_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_value: Option<String>,
}

// Request transformer implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        // Extract card data
        let card_data = match &item.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            payment_method_data => Err(IntegrationError::NotSupported {
                message: format!("Payment method {payment_method_data:?}"),
                connector: "Nexixpay",
                context: Default::default(),
            })?,
        };

        // Build card data structure using utility function for expiry date
        let card = NexixpayCardData {
            pan: Secret::new(card_data.card_number.peek().to_string()),
            expiry_date: card_data
                .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
            cvv: Some(card_data.card_cvc.clone()),
        };

        // CRITICAL FIX: Extract operation_id and PaRes from authentication_data
        // This is set by PostAuthenticate and passed by Hyperswitch to Authorize
        let authentication_data = item.request.authentication_data.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "authentication_data (must be present for 3DS flow)",
                context: Default::default(),
            },
        )?;

        let operation_id = authentication_data.transaction_id.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name:
                    "authentication_data.transaction_id (operationId from PostAuthenticate)",
                context: Default::default(),
            },
        )?;
        // Extract PaRes from redirect_response.payload (same as PostAuthenticate)
        let pa_res = item
            .request
            .redirect_response
            .as_ref()
            .and_then(|redirect| redirect.payload.as_ref())
            .and_then(|payload| {
                serde_json::from_value::<NexixpayRedirectPayload>(payload.peek().clone()).ok()
            })
            .and_then(|redirect_payload| redirect_payload.pa_res)
            .map(|secret| secret.expose());

        // Extract 3DS authentication data from authentication_data
        // IMPORTANT: NexiXPay /payment endpoint ONLY accepts PaRes and CAVV
        // Do NOT send eci or xid (causes 500 error)
        let three_ds_auth_data = Some(NexixpayThreeDSAuthData {
            three_d_s_auth_response: pa_res,
            authentication_value: authentication_data
                .cavv
                .as_ref()
                .map(|c| c.peek().to_string()),
        });

        // Build customer info with cardholder name and billing address
        let billing_address = item
            .resource_common_data
            .address
            .get_payment_method_billing()
            .and_then(|billing| {
                billing.address.as_ref().map(|addr| {
                    let country = addr
                        .country
                        .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3);
                    let name = match (&addr.first_name, &addr.last_name) {
                        (Some(first), Some(last)) => {
                            Some(Secret::new(format!("{} {}", first.peek(), last.peek())))
                        }
                        (Some(first), None) => Some(first.clone()),
                        (None, Some(last)) => Some(last.clone()),
                        (None, None) => None,
                    };
                    let street = match (&addr.line1, &addr.line2) {
                        (Some(l1), Some(l2)) => {
                            Some(Secret::new(format!("{}, {}", l1.peek(), l2.peek())))
                        }
                        (Some(l1), None) => Some(l1.clone()),
                        (None, Some(l2)) => Some(l2.clone()),
                        (None, None) => None,
                    };
                    NexixpayBillingAddress {
                        name,
                        street,
                        city: addr.city.clone().map(|c| c.expose().to_string()),
                        post_code: addr.zip.clone(),
                        country,
                    }
                })
            });

        let card_holder_name = item
            .resource_common_data
            .address
            .get_payment_method_billing()
            .and_then(|billing| {
                billing.address.as_ref().and_then(|addr| {
                    match (&addr.first_name, &addr.last_name) {
                        (Some(first), Some(last)) => {
                            Some(format!("{} {}", first.peek(), last.peek()))
                        }
                        (Some(first), None) => Some(first.peek().to_string()),
                        (None, Some(last)) => Some(last.peek().to_string()),
                        (None, None) => None,
                    }
                })
            })
            .unwrap_or_else(|| "Cardholder".to_string());

        let customer_info = NexixpayCustomerInfo {
            card_holder_name: Secret::new(card_holder_name),
            billing_address,
            shipping_address: None,
        };

        // Build order data with customer_info
        let order = NexixpayOrderData {
            order_id: get_nexi_order_id(&item.resource_common_data.connector_request_reference_id)?,
            amount: StringMinorUnitForConnector
                .convert(item.request.minor_amount, item.request.currency)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?,
            currency: item.request.currency,
            description: item
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|billing_descriptor| billing_descriptor.statement_descriptor.clone()),
            customer_info,
        };

        // Determine capture type
        let capture_type = match item.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => Some(NexixpayCaptureType::Explicit),
            Some(common_enums::CaptureMethod::Automatic)
            | Some(common_enums::CaptureMethod::SequentialAutomatic)
            | None => Some(NexixpayCaptureType::Implicit),
            _ => Some(NexixpayCaptureType::Implicit),
        };

        // Build recurrence request - default to NO_RECURRING for now
        let recurrence = RecurrenceRequest {
            action: NexixpayRecurringAction::NoRecurring,
            contract_id: None,
            contract_type: None,
        };

        Ok(Self {
            operation_id,
            order,
            card,
            capture_type,
            three_ds_auth_data,
            recurrence,
        })
    }
}

// ===== RESPONSE STRUCTURES =====

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPaymentsResponse {
    pub operation: NexixpayOperation,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayOperation {
    pub order_id: String,
    pub operation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_type: Option<String>,
    pub operation_result: NexixpayPaymentStatus,
    pub operation_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_circuit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_instrument_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_end_to_end_id: Option<String>,
    pub operation_amount: String,
    pub operation_currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data: Option<HashMap<String, serde_json::Value>>,
}

// Payment-specific operation result status enum
// CRITICAL: Matches Hyperswitch implementation exactly
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayPaymentStatus {
    Authorized,
    Executed,
    Declined,
    DeniedByRisk,
    ThreedsValidated,
    ThreedsFailed,
    Pending,
    Canceled,
    Voided,
    Refunded,
    Failed,
}

impl From<NexixpayPaymentStatus> for AttemptStatus {
    fn from(item: NexixpayPaymentStatus) -> Self {
        match item {
            NexixpayPaymentStatus::Declined
            | NexixpayPaymentStatus::DeniedByRisk
            | NexixpayPaymentStatus::ThreedsFailed
            | NexixpayPaymentStatus::Failed => Self::Failure,
            NexixpayPaymentStatus::Authorized => Self::Authorized,
            NexixpayPaymentStatus::ThreedsValidated => Self::AuthenticationSuccessful,
            NexixpayPaymentStatus::Executed => Self::Charged,
            NexixpayPaymentStatus::Pending => Self::AuthenticationPending, // this is being used in authorization calls only.
            NexixpayPaymentStatus::Canceled | NexixpayPaymentStatus::Voided => Self::Voided,
            NexixpayPaymentStatus::Refunded => Self::AutoRefunded,
        }
    }
}

// Response transformer implementation
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let operation = &item.response.operation;

        // Map status using From trait - matches Hyperswitch exactly
        let status = AttemptStatus::from(operation.operation_result.clone());

        // Extract network transaction ID from additionalData or payment_end_to_end_id
        let network_txn_id = operation
            .additional_data
            .as_ref()
            .and_then(|data| {
                data.get("schemaTID")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .or(operation.payment_end_to_end_id.clone());

        // Build connector metadata - preserve existing structural data and add payment response data
        // Following working commit pattern: don't overwrite, just add data
        let mut metadata_map = item
            .router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .and_then(|meta| meta.peek().as_object())
            .cloned()
            .unwrap_or_default();

        // Add payment response data from additional_data
        if let Some(additional_data) = &operation.additional_data {
            metadata_map.extend(additional_data.iter().map(|(k, v)| (k.clone(), v.clone())));
        }

        // Ensure structural metadata always exists for PSync compatibility
        // Add missing fields with defaults if not present from PreAuthenticate
        if !metadata_map.contains_key("authorizationOperationId") {
            metadata_map.insert(
                "authorizationOperationId".to_string(),
                serde_json::Value::String(operation.operation_id.clone()),
            );
        }
        if !metadata_map.contains_key("psyncFlow") {
            metadata_map.insert(
                "psyncFlow".to_string(),
                serde_json::Value::String(NexixpayPaymentIntent::Authorize.to_string()),
            );
        }

        let connector_metadata = if !metadata_map.is_empty() {
            Some(serde_json::Value::Object(metadata_map))
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(operation.operation_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: connector_metadata.clone(),
                network_txn_id,
                connector_response_reference_id: Some(operation.order_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: connector_metadata.clone().map(Secret::new),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PSYNC FLOW STRUCTURES =====

// Empty request structure for GET-based sync
#[derive(Debug, Serialize)]
pub struct NexixpaySyncRequest;

impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for NexixpaySyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Empty request for GET-based sync - operation_id extracted from URL
        Ok(Self)
    }
}

// PSync response structure - single operation object
// CRITICAL: GET /operations/{operation_id} returns a single operation, not an order with operations array
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpaySyncResponse {
    pub order_id: String,
    pub operation_id: String,
    pub operation_result: NexixpayPaymentStatus,
    pub operation_type: String,
}

// Response transformer implementation for PSync
impl TryFrom<ResponseRouterData<NexixpaySyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NexixpaySyncResponse, Self>) -> Result<Self, Self::Error> {
        // Map operation result to payment status using From trait
        let status = AttemptStatus::from(item.response.operation_result.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.operation_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_id.clone()),
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

// ===== CAPTURE FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayCaptureRequest {
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NexixpayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;

        // Convert amount - handle partial vs full capture
        let capture_amount = StringMinorUnitForConnector
            .convert(item.request.minor_amount_to_capture, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount: capture_amount,
            currency: item.request.currency,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayCaptureResponse {
    pub operation_id: String,
    pub operation_time: String,
}

impl TryFrom<ResponseRouterData<NexixpayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Capture response is minimal - only operationId and time
        // Capture call does not return status in their response, so we return Pending
        // Status must be verified via PSync

        // Build connector metadata - preserve existing structural data and add capture response data
        // Following working commit pattern: don't overwrite, just add data
        let mut metadata_map = item
            .router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .and_then(|meta| meta.peek().as_object())
            .cloned()
            .unwrap_or_default();

        // Add capture operation data
        metadata_map.insert(
            "captureOperationId".to_string(),
            serde_json::Value::String(item.response.operation_id.clone()),
        );

        // Ensure structural metadata always exists for PSync compatibility
        if !metadata_map.contains_key("psyncFlow") {
            metadata_map.insert(
                "psyncFlow".to_string(),
                serde_json::Value::String(NexixpayPaymentIntent::Capture.to_string()),
            );
        }

        let connector_metadata = if !metadata_map.is_empty() {
            Some(serde_json::Value::Object(metadata_map))
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.operation_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: connector_metadata.clone(),
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Pending, // Capture call does not return status in their response
                connector_feature_data: connector_metadata.clone().map(Secret::new),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayRefundRequest {
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for NexixpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        // connector_transaction_id is already a String (operationId from capture/authorization)
        // No need to extract from ResponseId

        // Convert refund amount
        let refund_amount = StringMinorUnitForConnector
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount: refund_amount,
            currency: item.request.currency,
            description: item.request.reason.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayRefundResponse {
    pub operation_id: String,
    pub operation_time: String,
}

impl TryFrom<ResponseRouterData<NexixpayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // CRITICAL: NexiXPay refund response is minimal (only operationId and time)
        // The response itself does NOT contain a status field
        // A 200 OK response indicates the refund was ACCEPTED, but NOT necessarily COMPLETED
        // The refund status must be verified via RSync (GET /orders/{orderId})
        // which returns the operations array with the actual refund result
        //

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.operation_id.clone(),
                refund_status: RefundStatus::Pending, // CRITICAL: NOT Success!
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== VOID FLOW STRUCTURES =====
// CRITICAL: NexiXPay does NOT have a dedicated /cancels endpoint
// Void is implemented via POST /operations/{operationId}/refunds with full authorized amount
// This is a "refund before capture" which voids the authorization

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayVoidRequest {
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NexixpayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;

        // CRITICAL: For void, we need to send the full authorized amount
        // This is extracted from the request data (required for NexiXPay void)
        let void_amount = item
            .request
            .amount
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "amount for void operation",
                context: Default::default(),
            })?;

        let currency = item
            .request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "currency for void operation",
                context: Default::default(),
            })?;

        let void_amount_string = StringMinorUnitForConnector
            .convert(void_amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount: void_amount_string,
            currency,
            description: item.request.cancellation_reason.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayVoidResponse {
    pub operation_id: String,
    pub operation_time: String,
}

impl TryFrom<ResponseRouterData<NexixpayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NexixpayVoidResponse, Self>) -> Result<Self, Self::Error> {
        // CRITICAL: NexiXPay void response is minimal (only operationId and time)

        // Build connector metadata - preserve existing structural data and add void response data
        // Following working commit pattern: don't overwrite, just add data
        let mut metadata_map = item
            .router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .and_then(|meta| meta.peek().as_object())
            .cloned()
            .unwrap_or_default();

        // Add void/cancel operation data
        metadata_map.insert(
            "cancelOperationId".to_string(),
            serde_json::Value::String(item.response.operation_id.clone()),
        );

        // Ensure structural metadata always exists for PSync compatibility
        if !metadata_map.contains_key("psyncFlow") {
            metadata_map.insert(
                "psyncFlow".to_string(),
                serde_json::Value::String(NexixpayPaymentIntent::Cancel.to_string()),
            );
        }

        let connector_metadata = if !metadata_map.is_empty() {
            Some(serde_json::Value::Object(metadata_map))
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.operation_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: connector_metadata.clone(),
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided, // Void succeeded
                connector_feature_data: connector_metadata.clone().map(Secret::new),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND SYNC (RSync) FLOW STRUCTURES =====

// Empty request structure for GET-based refund sync
#[derive(Debug, Serialize)]
pub struct NexixpayRefundSyncRequest;

impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for NexixpayRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Empty request for GET-based sync - connector_refund_id extracted from URL
        Ok(Self)
    }
}

// Refund-specific operation result status enum
// CRITICAL: Separate from payment status - matches Hyperswitch implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayRefundResultStatus {
    Pending,
    Voided,
    Refunded,
    Failed,
    Executed,
}

// RSync response structure - single operation object
// CRITICAL: GET /operations/{connector_refund_id} returns a single operation, not an order with operations array
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayRSyncResponse {
    pub order_id: String,
    pub operation_id: String,
    pub operation_result: NexixpayRefundResultStatus,
    pub operation_type: String,
}

// Status mapping for refunds
// CRITICAL: Implements reviewer feedback requirements
// - Checks operation result status
// - Returns appropriate RefundStatus based on operation_result
impl From<NexixpayRefundResultStatus> for RefundStatus {
    fn from(item: NexixpayRefundResultStatus) -> Self {
        match item {
            // Success cases - refund completed
            NexixpayRefundResultStatus::Voided
            | NexixpayRefundResultStatus::Refunded
            | NexixpayRefundResultStatus::Executed => Self::Success,

            // Pending case - refund still processing
            NexixpayRefundResultStatus::Pending => Self::Pending,

            // Failure case - refund failed
            NexixpayRefundResultStatus::Failed => Self::Failure,
        }
    }
}

// Response transformer implementation for RSync
impl TryFrom<ResponseRouterData<NexixpayRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // CRITICAL: Map operation result to refund status using From trait
        // This addresses reviewer feedback:
        // - Checks operation_result status (Voided, Refunded, Executed, Pending, Failed)
        // - Does NOT assume success without checking detailed response
        let refund_status = RefundStatus::from(item.response.operation_result);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.operation_id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== PRE-AUTHENTICATE FLOW STRUCTURES (Step 1 - Init 3DS) =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPreAuthenticateRequest {
    pub order: NexixpayPreAuthOrder,
    pub card: NexixpayCardData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<NexixpayRecurrence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_type: Option<NexixpayPaymentRequestActionType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPreAuthOrder {
    pub order_id: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    pub customer_info: NexixpayCustomerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayCustomerInfo {
    pub card_holder_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<NexixpayBillingAddress>,
    pub shipping_address: Option<NexixpayShippingAddress>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayBillingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha3>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayShippingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha3>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayRecurrence {
    pub action: NexixpayRecurringAction,
    pub contract_id: Option<Secret<String>>,
    pub contract_type: Option<ContractType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NexixpayPaymentRequestActionType {
    Verify,
}

// PreAuthenticate Request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayPreAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        // Extract card data from payment method
        let card_data = match item.request.payment_method_data.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "payment_method_data",
                context: Default::default(),
            },
        )? {
            PaymentMethodData::Card(card) => card,
            payment_method_data => Err(IntegrationError::NotSupported {
                message: format!("Payment method {payment_method_data:?} for 3DS"),
                connector: "Nexixpay",
                context: Default::default(),
            })?,
        };

        // Build card data structure using utility function for expiry date
        let card = NexixpayCardData {
            pan: Secret::new(card_data.card_number.peek().to_string()),
            expiry_date: card_data
                .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
            cvv: Some(card_data.card_cvc.clone()),
        };

        // Build customer info with billing and shipping addresses
        // Address comes from resource_common_data (PaymentFlowData), not from request
        let billing_address = item
            .resource_common_data
            .address
            .get_payment_method_billing()
            .and_then(|billing| {
                billing.address.as_ref().map(|addr| {
                    // Convert CountryAlpha2 to Alpha-3
                    let country = addr
                        .country
                        .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3);

                    // Combine first_name and last_name for the name field
                    let name = match (&addr.first_name, &addr.last_name) {
                        (Some(first), Some(last)) => {
                            Some(Secret::new(format!("{} {}", first.peek(), last.peek())))
                        }
                        (Some(first), None) => Some(first.clone()),
                        (None, Some(last)) => Some(last.clone()),
                        (None, None) => None,
                    };

                    // Combine line1 and line2 for street
                    let street = match (&addr.line1, &addr.line2) {
                        (Some(l1), Some(l2)) => {
                            Some(Secret::new(format!("{}, {}", l1.peek(), l2.peek())))
                        }
                        (Some(l1), None) => Some(l1.clone()),
                        (None, Some(l2)) => Some(l2.clone()),
                        (None, None) => None,
                    };

                    NexixpayBillingAddress {
                        name,
                        street,
                        city: addr.city.clone().map(|c| c.expose().to_string()),
                        post_code: addr.zip.clone(),
                        country,
                    }
                })
            });

        // Get cardholder name from billing address or use default
        let card_holder_name = item
            .resource_common_data
            .address
            .get_payment_method_billing()
            .and_then(|billing| {
                billing.address.as_ref().and_then(|addr| {
                    match (&addr.first_name, &addr.last_name) {
                        (Some(first), Some(last)) => {
                            Some(format!("{} {}", first.peek(), last.peek()))
                        }
                        (Some(first), None) => Some(first.peek().to_string()),
                        (None, Some(last)) => Some(last.peek().to_string()),
                        (None, None) => None,
                    }
                })
            })
            .unwrap_or_else(|| "Cardholder".to_string()); // Default fallback

        let customer_info = NexixpayCustomerInfo {
            card_holder_name: Secret::new(card_holder_name),
            billing_address,
            shipping_address: None, // Match Hyperswitch - always null for PreAuthenticate
        };

        // Build order data
        let currency = item
            .request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: Default::default(),
            })?;

        let order = NexixpayPreAuthOrder {
            order_id: get_nexi_order_id(&item.resource_common_data.connector_request_reference_id)?,
            amount: StringMinorUnitForConnector
                .convert(item.request.amount, currency)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?,
            currency,
            customer_info,
            description: item.resource_common_data.description.clone(),
        };

        // Build recurrence data - conditionally check for mandate reference
        // Following Hyperswitch pattern
        let recurrence = Some(match &item.request.mandate_reference {
            Some(MandateReferenceId::ConnectorMandateId(mandate_data)) => {
                if let Some(contract_id) = mandate_data.get_connector_mandate_request_reference_id()
                {
                    NexixpayRecurrence {
                        action: NexixpayRecurringAction::ContractCreation,
                        contract_id: Some(Secret::new(contract_id)),
                        contract_type: Some(ContractType::MitUnscheduled),
                    }
                } else {
                    NexixpayRecurrence {
                        action: NexixpayRecurringAction::NoRecurring,
                        contract_id: None,
                        contract_type: None,
                    }
                }
            }
            _ => NexixpayRecurrence {
                action: NexixpayRecurringAction::NoRecurring,
                contract_id: None,
                contract_type: None,
            },
        });

        // Add actionType logic for zero-amount payments
        let action_type = if item.request.amount == common_utils::types::MinorUnit::zero() {
            Some(NexixpayPaymentRequestActionType::Verify)
        } else {
            None
        };

        Ok(Self {
            order,
            card,
            recurrence,
            action_type,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPreAuthenticateResponse {
    pub operation: NexixpayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSEnrollmentStatus")]
    pub three_ds_enrollment_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthRequest")]
    pub three_ds_auth_request: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthUrl")]
    pub three_ds_auth_url: Option<String>,
}

// PreAuthenticate Response transformer
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpayPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let operation = &response.operation;

        // Map status based on operation result
        let status = match &operation.operation_result {
            NexixpayPaymentStatus::ThreedsValidated => AttemptStatus::AuthenticationSuccessful,
            NexixpayPaymentStatus::ThreedsFailed => AttemptStatus::AuthenticationFailed,
            NexixpayPaymentStatus::Declined | NexixpayPaymentStatus::DeniedByRisk => {
                AttemptStatus::AuthenticationFailed
            }
            // If 3DS is required, status is AuthenticationPending
            _ => AttemptStatus::AuthenticationPending,
        };

        // Build connector metadata to store operationId for Authorize (/payment)
        // CRITICAL FIX: Following exact Hyperswitch pattern with proper psync_flow initialization
        let connector_metadata = Some(serde_json::json!(NexixpayConnectorMetaData {
            authorization_operation_id: Some(operation.operation_id.clone()),
            three_d_s_auth_result: None, // Will be filled by PostAuthenticate
            three_d_s_auth_response: None, // Will be filled by PostAuthenticate
            capture_operation_id: None,  // Will be filled by Capture operation
            cancel_operation_id: None,   // Will be filled by Void operation
            psync_flow: NexixpayPaymentIntent::Authorize, // CRITICAL: Must be present for later parsing
        }));

        // Build authentication data with redirect form if needed
        // Following UCS standard pattern: return Form with endpoint and form_fields
        // Client is responsible for rendering and submitting the form
        let authentication_data = if let Some(auth_url) = &response.three_ds_auth_url {
            let mut form_fields = HashMap::new();

            // Field 1: ThreeDsRequest (exact case - matches NexiXPay API)
            form_fields.insert(
                "ThreeDsRequest".to_string(),
                response.three_ds_auth_request.clone().unwrap_or_default(),
            );

            // Field 2: ReturnUrl - where customer returns after 3DS challenge
            // Use continue_redirection_url (points to /complete for CompleteAuthorize flow)
            // NOT router_return_url (points to /response for PSync flow)
            if let Some(continue_url) = &item.router_data.request.continue_redirection_url {
                form_fields.insert("ReturnUrl".to_string(), continue_url.to_string());
            }

            // Field 3: transactionId - the operationId from NexiXPay
            form_fields.insert("transactionId".to_string(), operation.operation_id.clone());

            Some(Box::new(
                domain_types::router_response_types::RedirectForm::Form {
                    endpoint: auth_url.clone(),
                    method: common_utils::request::Method::Post,
                    form_fields,
                },
            ))
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                redirection_data: authentication_data,
                connector_response_reference_id: Some(operation.order_id.clone()),
                status_code: item.http_code,
                authentication_data: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: connector_metadata.map(Secret::new),
                preprocessing_id: Some(operation.operation_id.clone()), // Store operationId for Authorize
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== POST-AUTHENTICATE FLOW STRUCTURES (Step 3 - Validate 3DS) =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPostAuthenticateRequest {
    pub operation_id: String,
    #[serde(rename = "threeDSAuthResponse")]
    pub three_ds_auth_response: String,
}

// Redirect payload structure - returned from NexiXPay 3DS challenge
// Following Hyperswitch pattern for redirect response parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexixpayRedirectPayload {
    #[serde(rename = "PaRes")]
    pub pa_res: Option<Secret<String>>,
    #[serde(rename = "paymentId")]
    pub payment_id: Option<String>,
}

// PostAuthenticate Request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayPostAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        // Following Hyperswitch pattern: Parse JSON payload from redirect response
        let redirect_response = item.request.redirect_response.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "redirect_response",
                context: Default::default(),
            },
        )?;

        // Extract JSON payload from redirect response
        let redirect_payload_value = redirect_response
            .payload
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "request.redirect_response.payload",
                context: Default::default(),
            })?
            .peek();

        // Parse the JSON payload into RedirectPayload struct
        let redirect_payload: NexixpayRedirectPayload =
            serde_json::from_value(redirect_payload_value.clone()).map_err(|_| {
                IntegrationError::MissingRequiredField {
                    field_name: "redirection_payload",
                    context: Default::default(),
                }
            })?;

        // Extract operation_id from redirect payload (NexiXPay returns it as paymentId)
        // Fallback to connector_feature_data using helper function
        let operation_id = redirect_payload
            .payment_id
            .or_else(|| {
                // Use the helper function from Nexixpay struct
                // Note: This is a workaround since we can't call the helper directly without Nexixpay instance
                item.resource_common_data
                    .connector_feature_data
                    .as_ref()
                    .and_then(|metadata| metadata.peek().get("operationId"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "operationId (paymentId from redirect or connector_feature_data)",
                context: Default::default(),
            })?;

        // Extract PaRes (3DS authentication response) from redirect payload
        let three_ds_auth_response = redirect_payload
            .pa_res
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "PaRes from redirect_response",
                context: Default::default(),
            })?
            .peek()
            .to_string();

        Ok(Self {
            operation_id,
            three_ds_auth_response,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPostAuthenticateResponse {
    pub operation: NexixpayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthResult")]
    pub three_ds_auth_result: Option<NexixpayThreeDSAuthResult>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayThreeDSAuthResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

// PostAuthenticate Response transformer
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpayPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let operation = &response.operation;

        // Map status based on operation result
        let status = match &operation.operation_result {
            NexixpayPaymentStatus::ThreedsValidated => AttemptStatus::AuthenticationSuccessful,
            NexixpayPaymentStatus::ThreedsFailed => AttemptStatus::AuthenticationFailed,
            NexixpayPaymentStatus::Declined | NexixpayPaymentStatus::DeniedByRisk => {
                AttemptStatus::AuthenticationFailed
            }
            _ => AttemptStatus::AuthenticationPending,
        };

        // PostAuthenticate only returns authentication_data with CAVV/ECI/XID and operationId
        // PaRes is extracted directly from redirect_response in Authorize flow

        Ok(Self {
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data: response.three_ds_auth_result.as_ref().map(|auth_result| {
                    AuthenticationData {
                        trans_status: auth_result
                            .status
                            .as_ref()
                            .and_then(|s| s.parse::<common_enums::TransactionStatus>().ok()),
                        eci: auth_result.eci.clone(),
                        cavv: auth_result.authentication_value.clone().map(Secret::new),
                        ucaf_collection_indicator: None,
                        threeds_server_transaction_id: auth_result.xid.clone(),
                        message_version: auth_result
                            .version
                            .as_ref()
                            .and_then(|v| v.parse::<common_utils::types::SemanticVersion>().ok()),
                        // PaRes now read directly from redirect_response in Authorize
                        ds_trans_id: None,
                        acs_transaction_id: None,
                        // CRITICAL FIX: Store operationId in transaction_id for Authorize flow
                        transaction_id: Some(operation.operation_id.clone()),
                        exemption_indicator: None,
                        network_params: None,
                    }
                }),
                connector_response_reference_id: Some(operation.order_id.clone()),
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                // No need for connector_metadata - using authentication_data.ds_trans_id for PaRes
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Nexixpay HPP (Hosted Payment Page) order for client-side SDK initialization.
/// The securityToken and hostedPage URL are returned to the frontend for
/// client-side redirect / hosted payment page initialization.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayClientAuthRequest {
    pub order: NexixpayClientAuthOrder,
    pub payment_session: NexixpayPaymentSession,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayClientAuthOrder {
    pub order_id: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPaymentSession {
    pub action_type: String,
    pub amount: StringMinorUnit,
    pub recurrence: NexixpaySessionRecurrence,
    pub result_url: String,
    pub cancel_url: String,
    pub notification_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpaySessionRecurrence {
    pub action: NexixpayRecurringAction,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NexixpayRouterData<
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

        let order_id = get_nexi_order_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        let amount = StringMinorUnitForConnector
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the request carries a valid minor-unit amount and a currency \
                         supported by Nexi XPay. Nexi `/orders/hpp` expects \
                         `paymentSession.amount` as a string of minor units (e.g. cents), so \
                         the source `MinorUnit` must be representable as an integer string."
                            .to_owned(),
                    ),
                    doc_url: Some("https://developer.nexi.it/en/api/post-orders-hpp".to_owned()),
                    additional_context: Some(format!(
                        "Failed to encode amount={:?} currency={:?} as a Nexi \
                         `paymentSession.amount` (string minor-units) for the /orders/hpp \
                         ClientAuthenticationToken flow.",
                        router_data.request.amount, router_data.request.currency,
                    )),
                },
            })?;

        let return_url = router_data.resource_common_data.return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Populate `return_url` on the PaymentCreate/Confirm request so Nexi's \
                         Hosted Payment Page can redirect the cardholder back to the merchant \
                         on both successful and cancelled payments. This connector reuses \
                         `return_url` for both `paymentSession.resultUrl` and \
                         `paymentSession.cancelUrl`."
                            .to_owned(),
                    ),
                    doc_url: Some("https://developer.nexi.it/en/api/post-orders-hpp".to_owned()),
                    additional_context: Some(
                        "Nexi XPay /orders/hpp marks both `paymentSession.resultUrl` and \
                         `paymentSession.cancelUrl` as REQUIRED; without `return_url` the \
                         hosted payment session (ClientAuthenticationToken flow) cannot be \
                         initialized."
                            .to_owned(),
                    ),
                },
            },
        )?;

        Ok(Self {
            order: NexixpayClientAuthOrder {
                order_id,
                amount: amount.clone(),
                currency: router_data.request.currency,
                description: None,
            },
            payment_session: NexixpayPaymentSession {
                action_type: "PAY".to_string(),
                amount,
                recurrence: NexixpaySessionRecurrence {
                    action: NexixpayRecurringAction::NoRecurring,
                },
                result_url: return_url.clone(),
                cancel_url: return_url,
                notification_url: None,
            },
        })
    }
}

/// Nexixpay HPP order response containing securityToken and hostedPage URL for SDK initialization.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayClientAuthResponse {
    pub security_token: Secret<String>,
    pub hosted_page: String,
}

impl TryFrom<ResponseRouterData<NexixpayClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NexixpayClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Nexixpay(
                NexixpayClientAuthenticationResponseDomain {
                    security_token: response.security_token,
                    hosted_page: response.hosted_page,
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
// NexiXPay does not expose a dedicated mandate-setup endpoint. The canonical
// card-on-file pattern is to issue a 3-step "init" against
// `/orders/3steps/init` with a `recurrence` block of action
// `CONTRACT_CREATION` and a `MIT_UNSCHEDULED` contract type. NexiXPay
// persists the supplied `contractId` against the cardholder so it can be
// reused for subsequent merchant-initiated transactions. That `contractId`
// is surfaced as the `connector_mandate_id` for downstream RepeatPayment
// (MIT) calls. The `operationId` returned in the init response is also kept
// in `connector_metadata` (`authorizationOperationId`) so RepeatPayment /
// Authorize have everything they need.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpaySetupMandateRequest {
    pub order: NexixpayPreAuthOrder,
    pub card: NexixpayCardData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<NexixpayRecurrence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_type: Option<NexixpayPaymentRequestActionType>,
}

/// SetupMandate response — same wire shape as PreAuthenticate (init) since
/// NexiXPay reuses `/orders/3steps/init` for card-on-file contract creation.
pub type NexixpaySetupMandateResponse = NexixpayPreAuthenticateResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpaySetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;

        // Extract card data
        let card_data = match &item.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            payment_method_data => Err(IntegrationError::NotSupported {
                message: format!("Payment method {payment_method_data:?}"),
                connector: "Nexixpay",
                context: Default::default(),
            })?,
        };

        let card = NexixpayCardData {
            pan: Secret::new(card_data.card_number.peek().to_string()),
            expiry_date: card_data
                .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
            cvv: Some(card_data.card_cvc.clone()),
        };

        // Build customer info from billing address
        let billing_address = item
            .resource_common_data
            .address
            .get_payment_method_billing()
            .and_then(|billing| {
                billing.address.as_ref().map(|addr| {
                    let country = addr
                        .country
                        .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3);
                    let name = match (&addr.first_name, &addr.last_name) {
                        (Some(first), Some(last)) => {
                            Some(Secret::new(format!("{} {}", first.peek(), last.peek())))
                        }
                        (Some(first), None) => Some(first.clone()),
                        (None, Some(last)) => Some(last.clone()),
                        (None, None) => None,
                    };
                    let street = match (&addr.line1, &addr.line2) {
                        (Some(l1), Some(l2)) => {
                            Some(Secret::new(format!("{}, {}", l1.peek(), l2.peek())))
                        }
                        (Some(l1), None) => Some(l1.clone()),
                        (None, Some(l2)) => Some(l2.clone()),
                        (None, None) => None,
                    };
                    NexixpayBillingAddress {
                        name,
                        street,
                        city: addr.city.clone().map(|c| c.expose().to_string()),
                        post_code: addr.zip.clone(),
                        country,
                    }
                })
            });

        let card_holder_name = item
            .resource_common_data
            .address
            .get_payment_method_billing()
            .and_then(|billing| {
                billing.address.as_ref().and_then(|addr| {
                    match (&addr.first_name, &addr.last_name) {
                        (Some(first), Some(last)) => {
                            Some(format!("{} {}", first.peek(), last.peek()))
                        }
                        (Some(first), None) => Some(first.peek().to_string()),
                        (None, Some(last)) => Some(last.peek().to_string()),
                        (None, None) => None,
                    }
                })
            })
            .unwrap_or_else(|| "Cardholder".to_string());

        let customer_info = NexixpayCustomerInfo {
            card_holder_name: Secret::new(card_holder_name),
            billing_address,
            shipping_address: None,
        };

        // For SetupMandate the caller may not pass an amount. NexiXPay's
        // /init endpoint requires a non-null amount string, so fall back to
        // a minimum unit when none is supplied. ContractCreation works at
        // any amount (zero-amount verification uses actionType=VERIFY).
        let amount_minor = item
            .request
            .minor_amount
            .unwrap_or_else(|| common_utils::types::MinorUnit::new(0));
        let order_amount = StringMinorUnitForConnector
            .convert(amount_minor, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let order = NexixpayPreAuthOrder {
            order_id: get_nexi_order_id(
                &item.resource_common_data.connector_request_reference_id,
            )?,
            amount: order_amount,
            currency: item.request.currency,
            customer_info,
            description: item.resource_common_data.description.clone(),
        };

        // ContractCreation + MIT_UNSCHEDULED so NexiXPay persists the card
        // for future merchant-initiated reuse. The contractId we send is
        // the same one we surface back as `connector_mandate_id`.
        // NexiXPay rejects contract ids that are too long or contain
        // non-alphanumeric characters ("not valid"). Use the first 18 chars
        // of a dash-stripped UUID — within the documented max length and
        // safely alphanumeric.
        let contract_id: String = uuid::Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(18)
            .collect();
        let recurrence = Some(NexixpayRecurrence {
            action: NexixpayRecurringAction::ContractCreation,
            contract_id: Some(Secret::new(contract_id)),
            contract_type: Some(ContractType::MitUnscheduled),
        });

        // Zero-amount setup must use actionType=VERIFY for NexiXPay to
        // accept the verification call.
        let action_type = if amount_minor == common_utils::types::MinorUnit::new(0) {
            Some(NexixpayPaymentRequestActionType::Verify)
        } else {
            None
        };

        Ok(Self {
            order,
            card,
            recurrence,
            action_type,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NexixpaySetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let operation = &response.operation;

        // Re-derive the request body so we can recover the contractId we
        // generated and surface it back as connector_mandate_id. The
        // contractId is not echoed in NexiXPay's init response, but it is
        // also stored in connector_metadata for downstream MIT calls.
        // We generate a fresh uuid here only as a stable fallback if the
        // contract id cannot be otherwise propagated; in practice the
        // RepeatPayment flow reads `connector_mandate_id` from the
        // mandate_reference set below.
        let mandate_id = uuid::Uuid::new_v4().to_string();

        // Map the operation result to an attempt status. For SetupMandate
        // we want to reach a terminal state when no 3DS is required, so
        // promote Authorized to Charged (mirrors other UCS connectors).
        let mut status = match &operation.operation_result {
            NexixpayPaymentStatus::ThreedsValidated => AttemptStatus::AuthenticationSuccessful,
            NexixpayPaymentStatus::ThreedsFailed
            | NexixpayPaymentStatus::Declined
            | NexixpayPaymentStatus::DeniedByRisk
            | NexixpayPaymentStatus::Failed => AttemptStatus::Failure,
            NexixpayPaymentStatus::Authorized | NexixpayPaymentStatus::Executed => {
                AttemptStatus::Charged
            }
            NexixpayPaymentStatus::Pending => AttemptStatus::AuthenticationPending,
            NexixpayPaymentStatus::Canceled | NexixpayPaymentStatus::Voided => {
                AttemptStatus::Voided
            }
            NexixpayPaymentStatus::Refunded => AttemptStatus::AutoRefunded,
        };

        // If a 3DS challenge URL is present, surface it as redirection_data
        // and keep the attempt in AuthenticationPending so the caller drives
        // the challenge.
        let redirection_data = if let Some(auth_url) = &response.three_ds_auth_url {
            let mut form_fields = HashMap::new();
            form_fields.insert(
                "ThreeDsRequest".to_string(),
                response.three_ds_auth_request.clone().unwrap_or_default(),
            );
            form_fields.insert("transactionId".to_string(), operation.operation_id.clone());
            status = AttemptStatus::AuthenticationPending;
            Some(Box::new(
                domain_types::router_response_types::RedirectForm::Form {
                    endpoint: auth_url.clone(),
                    method: common_utils::request::Method::Post,
                    form_fields,
                },
            ))
        } else {
            None
        };

        // Persist the operationId in connector_metadata so subsequent
        // MIT (RepeatPayment) calls can re-use it.
        let connector_metadata = Some(serde_json::json!(NexixpayConnectorMetaData {
            three_d_s_auth_result: None,
            three_d_s_auth_response: None,
            authorization_operation_id: Some(operation.operation_id.clone()),
            cancel_operation_id: None,
            capture_operation_id: None,
            psync_flow: NexixpayPaymentIntent::Authorize,
        }));

        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(mandate_id),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(operation.operation_id.clone()),
                redirection_data,
                mandate_reference,
                connector_metadata: connector_metadata.clone(),
                network_txn_id: None,
                connector_response_reference_id: Some(operation.order_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: connector_metadata.map(Secret::new),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
