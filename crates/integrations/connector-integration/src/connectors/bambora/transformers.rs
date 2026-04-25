use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Deserializer, Serialize};

// Authentication Types

#[derive(Debug, Clone)]
pub struct BamboraAuthType {
    pub api_key: Secret<String>,
}

impl BamboraAuthType {
    /// Generates the Passcode authorization header
    /// Format: "Passcode base64(merchant_id:api_key)"
    pub fn generate_authorization_header(&self) -> String {
        self.api_key.peek().to_string()
    }
}

impl TryFrom<&ConnectorSpecificConfig> for BamboraAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Bambora {
                merchant_id,
                api_key,
                ..
            } => {
                let auth_string = format!("{}:{}", merchant_id.peek(), api_key.peek());
                let encoded = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    auth_string.as_bytes(),
                );
                Ok(Self {
                    api_key: Secret::new(format!("Passcode {encoded}")),
                })
            }
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// Error Response Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BamboraErrorResponse {
    pub code: i32,
    pub category: i32,
    pub message: String,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<serde_json::Value>,
}

// Request Types

#[derive(Debug, Serialize)]
pub struct BamboraPaymentsRequest<T: PaymentMethodDataTypes> {
    pub order_number: String,
    pub amount: FloatMajorUnit,
    pub payment_method: PaymentMethodType,
    pub card: BamboraCard<T>,
    pub billing: BamboraBillingAddress,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodType {
    Card,
}

#[derive(Debug, Serialize)]
pub struct BamboraCard<T: PaymentMethodDataTypes> {
    pub name: Secret<String>,
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvd: Secret<String>,
    pub complete: bool, // true for auto-capture, false for manual capture
}

#[derive(Debug, Serialize)]
pub struct BamboraBillingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub province: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_address: Option<common_utils::pii::Email>,
}

// Response Types

/// Bambora transaction type enum
/// Represents the type of transaction as returned by Bambora API
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum BamboraPaymentType {
    /// Payment (auto-captured or completed)
    #[serde(rename = "P")]
    Payment,
    /// Pre-authorization (authorized, not captured)
    #[serde(rename = "PA")]
    PreAuth,
    /// Pre-auth completion (captured)
    #[serde(rename = "PAC")]
    PreAuthCompletion,
    /// Return/Refund
    #[serde(rename = "R")]
    Return,
    /// Void payment
    #[serde(rename = "VP")]
    VoidPayment,
    /// Void refund
    #[serde(rename = "VR")]
    VoidRefund,
}

/// Helper function to deserialize string or i32 as String
fn str_or_i32<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StrOrI32 {
        Str(String),
        I32(i32),
    }

    let value = StrOrI32::deserialize(deserializer)?;
    Ok(match value {
        StrOrI32::Str(v) => v,
        StrOrI32::I32(v) => v.to_string(),
    })
}

// Type aliases for macro-based flow implementations
// Each flow needs a unique response type name to avoid duplicate templating struct definitions
pub type BamboraAuthorizeResponse = BamboraPaymentsResponse;
pub type BamboraCaptureResponse = BamboraPaymentsResponse;
pub type BamboraPSyncResponse = BamboraPaymentsResponse;
pub type BamboraVoidResponse = BamboraPaymentsResponse;
pub type BamboraRefundResponse = BamboraPaymentsResponse;
pub type BamboraRSyncResponse = BamboraPaymentsResponse;

#[derive(Debug, Deserialize, Serialize)]
pub struct BamboraPaymentsResponse {
    #[serde(deserialize_with = "str_or_i32")]
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorizing_merchant_id: Option<i32>,
    #[serde(deserialize_with = "str_or_i32")]
    pub approved: String, // "1" for approved, "0" for declined
    pub message: String,
    #[serde(deserialize_with = "str_or_i32")]
    pub message_id: String,
    pub auth_code: String,
    pub created: String,
    pub order_number: String,
    #[serde(rename = "type")]
    pub payment_type: BamboraPaymentType,
    pub amount: FloatMajorUnit,
    pub payment_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<BamboraCardResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BamboraCardResponse {
    pub card_type: String,
    pub last_four: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_bin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_match: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_result: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvd_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs: Option<BamboraAvsDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BamboraAvsDetails {
    pub id: String,
    pub message: String,
    pub processed: bool,
}

// Request Transformation

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for BamboraPaymentsRequest<T>
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
        // Extract card data
        let payment_method_data = &item.request.payment_method_data;
        let card = match payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Get cardholder name - prefer billing full name, fallback to customer name
                let cardholder_name = item
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .or_else(|| item.request.customer_name.clone().map(Secret::new))
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing.first_name or customer_name",
                        context: Default::default(),
                    })?;

                // Determine if this should be auto-capture or authorization
                let is_auto_capture = !crate::utils::is_manual_capture(item.request.capture_method);

                // Get 2-digit expiry year using utility function
                let expiry_year = card_data.get_card_expiry_year_2_digit()?;

                BamboraCard {
                    name: cardholder_name,
                    number: card_data.card_number.clone(),
                    expiry_month: card_data.card_exp_month.clone(),
                    expiry_year,
                    cvd: card_data.card_cvc.clone(),
                    complete: is_auto_capture,
                }
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
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "bambora",
                    context: Default::default(),
                }
                .into());
            }
        };

        // Extract billing address - mandatory field
        let payment_billing = item
            .resource_common_data
            .address
            .get_payment_billing()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing",
                context: Default::default(),
            })?;

        let billing_address =
            payment_billing
                .address
                .as_ref()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "billing.address",
                    context: Default::default(),
                })?;

        // Bambora requires province/state for US and CA addresses in 2-letter format
        // Convert full state names (e.g., "California", "New York") to 2-letter codes (e.g., "CA", "NY")
        let province = billing_address.state.clone().and_then(|state| {
            crate::utils::get_state_code_for_country(&state, billing_address.country)
        });

        let billing = BamboraBillingAddress {
            name: billing_address
                .first_name
                .clone()
                .or(billing_address.last_name.clone()),
            address_line1: billing_address.line1.clone(),
            address_line2: billing_address.line2.clone(),
            city: billing_address.city.clone().map(|s| s.expose()),
            province,
            country: billing_address.country,
            postal_code: billing_address.zip.clone(),
            phone_number: payment_billing
                .phone
                .as_ref()
                .and_then(|p| p.number.clone()),
            email_address: payment_billing.email.clone(),
        };

        // Convert amount from minor units to major units using FloatMajorUnitForConnector
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount from minor to major units")?;

        Ok(Self {
            order_number: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            payment_method: PaymentMethodType::Card,
            card,
            billing,
        })
    }
}

// Response Transformation

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<BamboraPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Status mapping using Bambora's payment_type field for robustness
        // payment_type: "P" = Payment (auto-captured), "PA" = Pre-authorization (manual)
        let is_approved = item.response.approved == "1";

        let status = if is_approved {
            // Use payment_type to determine if captured or just authorized
            match item.response.payment_type {
                BamboraPaymentType::PreAuth => AttemptStatus::Authorized, // Pre-auth (manual capture)
                BamboraPaymentType::Payment => AttemptStatus::Charged,    // Payment (auto-capture)
                BamboraPaymentType::PreAuthCompletion => AttemptStatus::Charged,
                BamboraPaymentType::Return
                | BamboraPaymentType::VoidPayment
                | BamboraPaymentType::VoidRefund => {
                    // Unexpected types for Authorize flow - mark as pending
                    AttemptStatus::Pending
                }
            }
        } else {
            // For failed transactions, check if it was meant to be auto-capture or manual
            let is_auto_capture = item
                .router_data
                .request
                .capture_method
                .map(|cm| matches!(cm, common_enums::CaptureMethod::Automatic))
                .unwrap_or(true);

            if is_auto_capture {
                AttemptStatus::Failure
            } else {
                AttemptStatus::AuthorizationFailed
            }
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_number.clone()),
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

// Capture (Complete Pre-Authorization) Implementation

#[derive(Debug, Serialize)]
pub struct BamboraCaptureRequest {
    pub amount: FloatMajorUnit,
    pub payment_method: PaymentMethodType,
}

impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for BamboraCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let _transaction_id = match &item.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id,
            ResponseId::EncodedData(_) | ResponseId::NoResponseId => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
                .into());
            }
        };

        // Convert amount from minor units to major units using FloatMajorUnitForConnector
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(item.request.minor_amount_to_capture, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert capture amount from minor to major units")?;

        Ok(Self {
            amount,
            payment_method: PaymentMethodType::Card,
        })
    }
}

impl TryFrom<ResponseRouterData<BamboraPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Status mapping for capture completion
        // For approved captures, payment_type should be "PAC" (Pre-auth Completion)
        let is_approved = item.response.approved == "1";

        let status = if is_approved {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Failure
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_number.clone()),
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

// PSync uses GET request, so no request body is needed
#[derive(Debug, Serialize)]
pub struct BamboraSyncRequest;

impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for BamboraSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // GET request - no body needed
        Ok(Self)
    }
}

// PSync Response Transformation
// The GET /payments/{transId} endpoint returns the same structure as authorization
impl TryFrom<ResponseRouterData<BamboraPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Status mapping using Bambora's payment_type field for accuracy
        // payment_type indicates the actual transaction state:
        // "P" = Payment (auto-captured or completed)
        // "PA" = Pre-authorization (authorized, not captured)
        // "PAC" = Pre-auth completion (captured)
        let is_approved = item.response.approved == "1";

        let status = if is_approved {
            // Use payment_type to determine if captured or just authorized
            match item.response.payment_type {
                BamboraPaymentType::PreAuth => AttemptStatus::Authorized, // Pre-auth only
                BamboraPaymentType::Payment | BamboraPaymentType::PreAuthCompletion => {
                    AttemptStatus::Charged // Payment or Pre-auth completion
                }
                BamboraPaymentType::VoidPayment | BamboraPaymentType::VoidRefund => {
                    AttemptStatus::Voided // Void types map to Voided status
                }
                BamboraPaymentType::Return => {
                    // Return/Refund is handled separately in refund flows
                    // If seen in PSync, mark as pending for investigation
                    AttemptStatus::Pending
                }
            }
        } else {
            // For failed transactions, check if it was meant to be auto-capture or manual
            let is_auto_capture = item
                .router_data
                .request
                .capture_method
                .map(|cm| matches!(cm, common_enums::CaptureMethod::Automatic))
                .unwrap_or(true);

            if is_auto_capture {
                AttemptStatus::Failure
            } else {
                AttemptStatus::AuthorizationFailed
            }
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_number.clone()),
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

// Refund Implementation

#[derive(Debug, Serialize)]
pub struct BamboraRefundRequest {
    pub amount: FloatMajorUnit,
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for BamboraRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Convert amount from minor units to major units using FloatMajorUnitForConnector
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self { amount })
    }
}

impl TryFrom<ResponseRouterData<BamboraPaymentsResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Status mapping following hyperswitch pattern
        // Only check approved field for refund
        let is_approved = item.response.approved == "1";

        let refund_status = if is_approved {
            RefundStatus::Success
        } else {
            RefundStatus::Failure
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Refund Sync (RSync) Implementation

impl TryFrom<ResponseRouterData<BamboraPaymentsResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Status mapping following hyperswitch pattern
        // Only check approved field for refund sync
        let is_approved = item.response.approved == "1";

        let refund_status = if is_approved {
            RefundStatus::Success
        } else {
            RefundStatus::Failure
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ============================================================================
// Void Implementation
// ============================================================================

/// Void Request Structure
/// Per technical specification:
/// - Endpoint: POST /payments/{transId}/void
/// - Request body: amount, order_number
/// - Response: Identical to Make Payment response but with type "VP" (void payment)
/// - Can void pre-authorizations (PA) before they are captured
/// - Cannot void already completed payments - use refund instead
#[derive(Debug, Serialize)]
pub struct BamboraVoidRequest {
    pub amount: FloatMajorUnit,
}

impl TryFrom<&RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for BamboraVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        if item.request.connector_transaction_id.is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            }
            .into());
        }

        // Get the amount from the original transaction
        // For void, we typically void the full amount
        let minor_amount = item
            .request
            .amount
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: Default::default(),
            })?;

        // Get currency from request
        let currency = item
            .request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: Default::default(),
            })?;

        // Convert amount from minor units to major units using FloatMajorUnitForConnector
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert void amount from minor to major units")?;

        Ok(Self { amount })
    }
}

impl TryFrom<ResponseRouterData<BamboraPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Status mapping following hyperswitch pattern
        // Only check approved field for void
        let is_approved = item.response.approved == "1";

        let status = if is_approved {
            AttemptStatus::Voided
        } else {
            AttemptStatus::VoidFailed
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_number.clone()),
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

// Macro Wrapper Type Implementations

use crate::connectors::bambora::BamboraRouterData;
use domain_types::errors::{ConnectorError, IntegrationError};

// Authorize - wrapper to RouterDataV2
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BamboraRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BamboraPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: BamboraRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

// Capture - wrapper to RouterDataV2
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BamboraRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for BamboraCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: BamboraRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

// Void - wrapper to RouterDataV2
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BamboraRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for BamboraVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: BamboraRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

// Refund - wrapper to RouterDataV2
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BamboraRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for BamboraRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: BamboraRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

// ============================================================================
// SetupMandate Implementation
// ============================================================================
//
// Bambora NA does not expose a SetupMandate/verify endpoint that works with
// the standard payments passcode. The canonical card-on-file pattern is
// used: POST to `/v1/payments` with `complete=false` (auth-only) using a
// small verification amount. The resulting Bambora transaction id is
// surfaced as the `connector_mandate_id` and is consumed by RepeatPayment
// (MIT) as a reference-transaction token.

#[derive(Debug, Serialize)]
pub struct BamboraSetupMandateRequest<T: PaymentMethodDataTypes> {
    pub order_number: String,
    pub amount: FloatMajorUnit,
    pub payment_method: PaymentMethodType,
    pub card: BamboraCard<T>,
    pub billing: BamboraBillingAddress,
    /// Credential-on-file marker. "first_recurring" tells Bambora that this
    /// is the authorizing transaction in a card-on-file recurring series.
    /// The resulting transaction id is usable as a token-reference for
    /// subsequent MIT charges (with card_on_file.type="subsequent_recurring").
    pub card_on_file: BamboraCardOnFile,
}

pub type BamboraSetupMandateResponse = BamboraPaymentsResponse;

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    > for BamboraSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method_data = &item.request.payment_method_data;
        let card = match payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let cardholder_name = item
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .or_else(|| item.request.customer_name.clone().map(Secret::new))
                    .unwrap_or_else(|| Secret::new(String::new()));

                let expiry_year = card_data.get_card_expiry_year_2_digit()?;

                BamboraCard {
                    name: cardholder_name,
                    number: card_data.card_number.clone(),
                    expiry_month: card_data.card_exp_month.clone(),
                    expiry_year,
                    cvd: card_data.card_cvc.clone(),
                    // Capture the verification amount so the resulting
                    // transaction id is usable as a token-reference for
                    // subsequent MIT charges (Bambora does not permit a
                    // pre-auth to be used as a COF token-ref).
                    complete: true,
                }
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Payment method not supported for SetupMandate".to_string(),
                    connector: "bambora",
                    context: Default::default(),
                }
                .into());
            }
        };

        // Billing address (best-effort — Bambora requires billing for most
        // transactions but we supply what is available).
        let payment_billing = item.resource_common_data.address.get_payment_billing();
        let billing_address = payment_billing.and_then(|pb| pb.address.as_ref());

        let billing = if let Some(addr) = billing_address {
            let province = addr
                .state
                .clone()
                .and_then(|state| crate::utils::get_state_code_for_country(&state, addr.country));
            BamboraBillingAddress {
                name: addr.first_name.clone().or(addr.last_name.clone()),
                address_line1: addr.line1.clone(),
                address_line2: addr.line2.clone(),
                city: addr.city.clone().map(|s| s.expose()),
                province,
                country: addr.country,
                postal_code: addr.zip.clone(),
                phone_number: payment_billing
                    .and_then(|pb| pb.phone.as_ref())
                    .and_then(|p| p.number.clone()),
                email_address: payment_billing.and_then(|pb| pb.email.clone()),
            }
        } else {
            BamboraBillingAddress {
                name: None,
                address_line1: None,
                address_line2: None,
                city: None,
                province: None,
                country: None,
                postal_code: None,
                phone_number: None,
                email_address: None,
            }
        };

        // Amount: use incoming amount if supplied, else fall back to 1 cent
        // (Bambora rejects $0 auth-only attempts so we enforce a minimum).
        let converter = FloatMajorUnitForConnector;
        let minor_amount = item
            .request
            .minor_amount
            .unwrap_or(common_utils::types::MinorUnit::new(1));
        let amount = converter
            .convert(minor_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert setup-mandate amount")?;

        Ok(Self {
            order_number: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            payment_method: PaymentMethodType::Card,
            card,
            billing,
            card_on_file: BamboraCardOnFile {
                cof_type: "first_recurring",
            },
        })
    }
}

// Wrapper conversion for macro
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BamboraRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BamboraSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: BamboraRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<BamboraSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_approved = item.response.approved == "1";

        // Treat approved auth-only as Charged so the attempt reaches a
        // terminal state for downstream consumers; RepeatPayment consumes
        // the txn id as a reference-transaction token.
        let status = if is_approved {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Failure
        };

        let response = if is_approved {
            let mandate_reference = Some(Box::new(MandateReference {
                connector_mandate_id: Some(item.response.id.clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            }));
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_number.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        } else {
            Err(domain_types::router_data::ErrorResponse {
                status_code: item.http_code,
                code: item.response.message_id.clone(),
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
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

// ============================================================================
// RepeatPayment (MIT) Implementation
// ============================================================================
//
// Bambora NA supports Merchant-Initiated Transactions via the reference-
// transaction model: POST /v1/payments with `payment_method: "token"` and
// `token.code` = the prior Bambora transaction id captured during
// SetupMandate. This approach uses the standard /payments passcode and
// does not require a separate Payment Profile API passcode. We additionally
// include a `card_on_file` block with `type: "subsequent_recurring"` so
// Bambora recognises this as a credential-on-file MIT (required since
// Bambora's rollout of the COF mandates).

#[derive(Debug, Serialize)]
pub struct BamboraRepeatPaymentRequest {
    pub order_number: String,
    pub amount: FloatMajorUnit,
    pub payment_method: String,
    pub token: BamboraRepeatPaymentToken,
    pub card_on_file: BamboraCardOnFile,
}

#[derive(Debug, Serialize)]
pub struct BamboraRepeatPaymentToken {
    /// Prior Bambora transaction id (from SetupMandate), used as a
    /// reference-transaction token for card-on-file merchant-initiated
    /// charges.
    pub code: String,
    pub name: String,
    pub complete: bool,
}

#[derive(Debug, Serialize)]
pub struct BamboraCardOnFile {
    /// Credential-on-file transaction type. "subsequent_recurring" tells
    /// Bambora this is a follow-up merchant-initiated charge in a
    /// recurring series anchored to a prior authorization.
    #[serde(rename = "type")]
    pub cof_type: &'static str,
}

pub type BamboraRepeatPaymentResponse = BamboraPaymentsResponse;

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
    > for BamboraRepeatPaymentRequest
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
        let mandate_ref_id = match &item.request.mandate_reference {
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

        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert repeat-payment amount")?;

        Ok(Self {
            order_number: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            payment_method: "token".to_string(),
            token: BamboraRepeatPaymentToken {
                code: mandate_ref_id.clone(),
                name: "Cardholder".to_string(),
                complete: item.request.is_auto_capture(),
            },
            card_on_file: BamboraCardOnFile {
                cof_type: "subsequent_recurring",
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BamboraRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BamboraRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: BamboraRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<BamboraRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_approved = item.response.approved == "1";
        let is_auto_capture = item.router_data.request.is_auto_capture();

        let status = if is_approved {
            if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        } else {
            AttemptStatus::Failure
        };

        let response = if is_approved {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_number.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        } else {
            Err(domain_types::router_data::ErrorResponse {
                status_code: item.http_code,
                code: item.response.message_id.clone(),
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
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
