use common_utils::types::MinorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, ClientAuthenticationToken, PSync, RSync, RepeatPayment},
    connector_types::{
        BamboraapacClientAuthenticationResponse as BamboraapacClientAuthenticationResponseDomain,
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{CardToken, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, PaymentMethodToken},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// ============================================================================
// XML SERIALIZATION STRUCTURES (for quick-xml)
// ============================================================================

// Inner Transaction XML structure (inside CDATA)
#[derive(Debug, Serialize)]
#[serde(rename = "Transaction")]
struct TransactionXml {
    #[serde(rename = "CustRef")]
    cust_ref: String,
    #[serde(rename = "Amount")]
    amount: i64,
    #[serde(rename = "TrnType")]
    trn_type: i32,
    #[serde(rename = "AccountNumber")]
    account_number: String,
    #[serde(rename = "CreditCard")]
    credit_card: CreditCardXml,
    #[serde(rename = "Security")]
    security: SecurityXml,
}

#[derive(Debug, Serialize)]
struct CreditCardXml {
    #[serde(rename = "@Registered")]
    registered: &'static str,
    #[serde(rename = "CardNumber")]
    card_number: String,
    #[serde(rename = "ExpM")]
    exp_month: String,
    #[serde(rename = "ExpY")]
    exp_year: String,
    #[serde(rename = "CVN")]
    cvn: String,
    #[serde(rename = "CardHolderName")]
    card_holder_name: String,
}

#[derive(Debug, Serialize)]
struct SecurityXml {
    #[serde(rename = "UserName")]
    username: String,
    #[serde(rename = "Password")]
    password: String,
}

// Capture XML structure
#[derive(Debug, Serialize)]
#[serde(rename = "Capture")]
struct CaptureXml {
    #[serde(rename = "Receipt")]
    receipt: String,
    #[serde(rename = "Amount")]
    amount: i64,
    #[serde(rename = "Security")]
    security: SecurityXml,
}

// Refund XML structure
#[derive(Debug, Serialize)]
#[serde(rename = "Refund")]
struct RefundXml {
    #[serde(rename = "CustRef")]
    cust_ref: String,
    #[serde(rename = "Receipt")]
    receipt: String,
    #[serde(rename = "Amount")]
    amount: i64,
    #[serde(rename = "Security")]
    security: SecurityXml,
}

// Query Transaction XML structure
#[derive(Debug, Serialize)]
#[serde(rename = "QueryTransaction")]
struct QueryTransactionXml {
    #[serde(rename = "Criteria")]
    criteria: QueryCriteriaXml,
    #[serde(rename = "Security")]
    security: SecurityXml,
}

#[derive(Debug, Serialize)]
struct QueryCriteriaXml {
    #[serde(rename = "AccountNumber")]
    account_number: String,
    #[serde(rename = "TrnStartTimestamp")]
    trn_start_timestamp: &'static str,
    #[serde(rename = "TrnEndTimestamp")]
    trn_end_timestamp: &'static str,
    #[serde(rename = "Receipt")]
    receipt: String,
}

// ============================================================================
// END XML SERIALIZATION STRUCTURES
// ============================================================================

// Authentication Type Definition
#[derive(Debug, Clone)]
pub struct BamboraapacAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub account_number: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for BamboraapacAuthType {
    type Error = IntegrationError;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Bamboraapac {
                username,
                password,
                account_number,
                ..
            } => Ok(Self {
                username: username.clone(),
                password: password.clone(),
                account_number: account_number.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }),
        }
    }
}

// Transaction Types for Bambora APAC
#[derive(Debug, Clone, Copy)]
pub enum BamboraapacTrnType {
    Purchase = 1,
    PreAuth = 2,
    Capture = 3,
    Refund = 5,
    DirectDebit = 7,
}

impl From<BamboraapacTrnType> for i32 {
    fn from(trn_type: BamboraapacTrnType) -> Self {
        match trn_type {
            BamboraapacTrnType::Purchase => 1,
            BamboraapacTrnType::PreAuth => 2,
            BamboraapacTrnType::Capture => 3,
            BamboraapacTrnType::Refund => 5,
            BamboraapacTrnType::DirectDebit => 7,
        }
    }
}

// Request Structure for SOAP/XML
#[derive(Debug, Clone)]
pub struct BamboraapacPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub account_number: Secret<String>,
    pub cust_number: Option<String>,
    pub cust_ref: String,
    pub amount: MinorUnit,
    pub trn_type: BamboraapacTrnType,
    pub card_number: Secret<String>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub cvn: Secret<String>,
    pub card_holder_name: Secret<String>,
    pub username: Secret<String>,
    pub password: Secret<String>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    BamboraapacPaymentRequest<T>
{
    // Generate SOAP XML request using quick-xml serialization
    pub fn to_soap_xml(&self) -> String {
        // Build the inner Transaction XML using quick-xml
        let transaction_xml = TransactionXml {
            cust_ref: self.cust_ref.clone(),
            amount: self.amount.get_amount_as_i64(),
            trn_type: i32::from(self.trn_type),
            account_number: self.account_number.peek().to_string(),
            credit_card: CreditCardXml {
                registered: "False",
                card_number: self.card_number.peek().to_string(),
                exp_month: self.exp_month.peek().to_string(),
                exp_year: self.exp_year.peek().to_string(),
                cvn: self.cvn.peek().to_string(),
                card_holder_name: self.card_holder_name.peek().to_string(),
            },
            security: SecurityXml {
                username: self.username.peek().to_string(),
                password: self.password.peek().to_string(),
            },
        };

        // Serialize using quick-xml
        let transaction_xml_string = quick_xml::se::to_string(&transaction_xml)
            .unwrap_or_else(|_| String::from("<Transaction/>"));

        // Wrap in SOAP envelope (only the envelope structure, data is safely serialized)
        format!(
            r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:dts="http://www.ippayments.com.au/interface/api/dts">
<soapenv:Body>
<dts:SubmitSinglePayment>
<dts:trnXML><![CDATA[{transaction_xml_string}]]></dts:trnXML>
</dts:SubmitSinglePayment>
</soapenv:Body>
</soapenv:Envelope>"#
        )
    }
}

// Response Structure - Nested SOAP/XML response
// This matches the structure after removing namespace prefixes
// The Envelope wrapper is automatically skipped by the XML deserializer
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BamboraapacPaymentResponse {
    pub body: BodyResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BodyResponse {
    pub submit_single_payment_response: SubmitSinglePaymentResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SubmitSinglePaymentResponse {
    pub submit_single_payment_result: String, // HTML-encoded XML string
}

// Inner payment response structure (after decoding HTML entities)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PaymentResponse {
    pub response_code: u8,
    pub receipt: String,
    pub credit_card_token: Option<String>,
    pub declined_code: Option<String>,
    pub declined_message: Option<String>,
}

// Error Response Structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BamboraapacErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl Default for BamboraapacErrorResponse {
    fn default() -> Self {
        Self {
            error_code: Some("UNKNOWN_ERROR".to_string()),
            error_message: Some("Unknown error occurred".to_string()),
        }
    }
}

// ============================================================================
// CAPTURE FLOW STRUCTURES
// ============================================================================

// Capture Request Structure
#[derive(Debug, Clone)]
pub struct BamboraapacCaptureRequest {
    pub receipt: String,
    pub amount: MinorUnit,
    pub username: Secret<String>,
    pub password: Secret<String>,
}

// Capture Response Structure
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BamboraapacCaptureResponse {
    pub body: CaptureBodyResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CaptureBodyResponse {
    pub submit_single_capture_response: SubmitSingleCaptureResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SubmitSingleCaptureResponse {
    pub submit_single_capture_result: String, // HTML-encoded XML string
}

// Inner capture response structure (after decoding HTML entities)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CaptureResponse {
    pub response_code: u8,
    pub timestamp: Option<String>,
    pub receipt: String,
    pub settlement_date: Option<String>,
    pub declined_code: Option<String>,
    pub declined_message: Option<String>,
}

// ============================================================================
// REFUND FLOW STRUCTURES
// ============================================================================

// Refund Request Structure
#[derive(Debug, Clone)]
pub struct BamboraapacRefundRequest {
    pub cust_ref: String,
    pub receipt: String, // Original transaction receipt/ID to refund
    pub amount: MinorUnit,
    pub username: Secret<String>,
    pub password: Secret<String>,
}

// Refund Response Structure
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BamboraapacRefundResponse {
    pub body: RefundBodyResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundBodyResponse {
    #[serde(rename = "SubmitSingleRefundResponse")]
    pub submit_single_refund_response: SubmitSingleRefundResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SubmitSingleRefundResponse {
    pub submit_single_refund_result: String, // HTML-encoded XML string
}

// Inner refund response structure (after decoding HTML entities)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RefundResponseInner {
    pub response_code: u8,
    pub timestamp: Option<String>,
    pub receipt: String,
    pub settlement_date: Option<String>,
    pub declined_code: Option<String>,
    pub declined_message: Option<String>,
}

// ============================================================================
// SYNC FLOW STRUCTURES (PSync and RSync)
// ============================================================================

// Sync Request Structure
#[derive(Debug, Clone)]
pub struct BamboraapacSyncRequest {
    pub account_number: Secret<String>,
    pub receipt: String, // Transaction receipt/ID to query
    pub username: Secret<String>,
    pub password: Secret<String>,
}

// Sync Response Structure
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BamboraapacSyncResponse {
    pub body: SyncBodyResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QueryTransactionResponse {
    pub query_transaction_result: String, // HTML-encoded XML string
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SyncBodyResponse {
    pub query_transaction_response: QueryTransactionResponse,
}

// Inner sync response structures (after decoding HTML entities)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SyncResponse {
    pub response_code: u8,
    pub receipt: String,
    pub declined_code: Option<String>,
    pub declined_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QueryResponse {
    #[serde(rename = "Response")]
    pub response: Option<SyncResponse>,
}

// Inner payment response structure for successful queries
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct InnerPaymentResponse {
    pub response_code: u8,
    pub receipt: String,
    pub credit_card_token: Option<String>,
    pub declined_code: Option<String>,
    pub declined_message: Option<String>,
}

// Request Transformation Implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for BamboraapacPaymentRequest<T>
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
        let auth = BamboraapacAuthType::try_from(&router_data.connector_config)?;

        // Determine transaction type based on capture method
        let trn_type = match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => BamboraapacTrnType::PreAuth,
            _ => BamboraapacTrnType::Purchase,
        };

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Get card number using peek() method
                let card_number_str = card_data.card_number.peek().to_string();

                Ok(Self {
                    account_number: auth.account_number,
                    cust_number: router_data
                        .request
                        .customer_id
                        .as_ref()
                        .map(|id| id.get_string_repr().to_string()),
                    cust_ref: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    amount: router_data.request.minor_amount,
                    trn_type,
                    card_number: Secret::new(card_number_str),
                    exp_month: card_data.card_exp_month.clone(),
                    exp_year: card_data.get_expiry_year_4_digit(),
                    cvn: card_data.card_cvc.clone(),
                    card_holder_name: card_data.card_holder_name.clone().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "payment_method.card.card_holder_name",
                            context: Default::default(),
                        },
                    )?,
                    username: auth.username,
                    password: auth.password,
                    _phantom: std::marker::PhantomData,
                })
            }
            // TODO: Use the token from payment_method_token to populate the SOAP XML TokenNumber field
            // instead of raw card data. The token returned by Bamboraapac's SOAP-based tokenization
            // (ClientAuthenticationToken flow) should be sent as TokenNumber in the Transaction XML.
            PaymentMethodData::CardToken(CardToken { .. }) => {
                let token = router_data
                    .resource_common_data
                    .payment_method_token
                    .as_ref()
                    .map(|t| match t {
                        PaymentMethodToken::Token(s) => s.clone(),
                    })
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "payment_method_token",
                            context: Default::default(),
                        })
                    })?;

                Ok(Self {
                    account_number: auth.account_number,
                    cust_number: router_data
                        .request
                        .customer_id
                        .as_ref()
                        .map(|id| id.get_string_repr().to_string()),
                    cust_ref: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    amount: router_data.request.minor_amount,
                    trn_type,
                    card_number: token,
                    exp_month: Secret::new(String::new()),
                    exp_year: Secret::new(String::new()),
                    cvn: Secret::new(String::new()),
                    card_holder_name: Secret::new(String::new()),
                    username: auth.username,
                    password: auth.password,
                    _phantom: std::marker::PhantomData,
                })
            }
            _ => Err(
                IntegrationError::not_implemented("Payment method not supported".to_string())
                    .into(),
            ),
        }
    }
}

// Response Transformation Implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BamboraapacPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraapacPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        use common_utils::ext_traits::XmlExt;

        let router_data = &item.router_data;

        // Decode the HTML-encoded inner XML
        let inner_xml = item
            .response
            .body
            .submit_single_payment_response
            .submit_single_payment_result
            .replace("&lt;", "<")
            .replace("&gt;", ">");

        // Parse the inner Response XML
        let response: PaymentResponse = inner_xml.as_str().parse_xml().change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "bamboraapac"),
        )?;

        // Map Bambora response code to standard status
        // 0 = Approved, 1 = Not Approved
        let status = if response.response_code == 0 {
            if router_data.request.capture_method == Some(common_enums::CaptureMethod::Manual) {
                common_enums::AttemptStatus::Authorized
            } else {
                common_enums::AttemptStatus::Charged
            }
        } else {
            common_enums::AttemptStatus::Failure
        };

        // Handle error responses
        if status == common_enums::AttemptStatus::Failure {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: response
                        .declined_code
                        .clone()
                        .unwrap_or_else(|| "DECLINED".to_string()),
                    message: response
                        .declined_message
                        .clone()
                        .unwrap_or_else(|| "Payment declined".to_string()),
                    reason: response.declined_message.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: Some(response.receipt.clone()),
                    network_decline_code: response.declined_code.clone(),
                    network_advice_code: None,
                    network_error_message: response.declined_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.receipt.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.receipt.clone()),
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

// ============================================================================
// CAPTURE FLOW TRANSFORMERS
// ============================================================================

// Capture Request Transformation
impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for BamboraapacCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            Capture,
            PaymentFlowData,
            PaymentsCaptureData,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = BamboraapacAuthType::try_from(&router_data.connector_config)?;

        // Get the connector transaction ID (receipt) from the payment attempt
        let receipt = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) | ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_transaction_id",
                        context: Default::default()
                    }
                ))
            }
        };

        Ok(Self {
            receipt,
            amount: router_data.request.minor_amount_to_capture,
            username: auth.username,
            password: auth.password,
        })
    }
}

// Capture Response Transformation
impl TryFrom<ResponseRouterData<BamboraapacCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraapacCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        use common_utils::ext_traits::XmlExt;

        let router_data = &item.router_data;

        // Decode the HTML-encoded inner XML
        let inner_xml = item
            .response
            .body
            .submit_single_capture_response
            .submit_single_capture_result
            .replace("&lt;", "<")
            .replace("&gt;", ">");

        // Parse the inner Response XML
        let response: CaptureResponse = inner_xml.as_str().parse_xml().change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "bamboraapac"),
        )?;

        // Map Bambora response code to standard status (0 = Approved)
        let status = if response.response_code == 0 {
            common_enums::AttemptStatus::Charged
        } else {
            common_enums::AttemptStatus::Failure
        };

        // Handle error responses
        if status == common_enums::AttemptStatus::Failure {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: response
                        .declined_code
                        .clone()
                        .unwrap_or_else(|| "DECLINED".to_string()),
                    message: response
                        .declined_message
                        .clone()
                        .unwrap_or_else(|| "Capture declined".to_string()),
                    reason: response.declined_message.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: Some(response.receipt.clone()),
                    network_decline_code: response.declined_code.clone(),
                    network_advice_code: None,
                    network_error_message: response.declined_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.receipt.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.receipt.clone()),
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

// ============================================================================
// PSYNC FLOW TRANSFORMERS
// ============================================================================

// PSync Request Transformation
impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for BamboraapacSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = BamboraapacAuthType::try_from(&router_data.connector_config)?;

        // Get the connector transaction ID to query
        let receipt = router_data
            .request
            .connector_transaction_id
            .clone()
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;

        Ok(Self {
            account_number: auth.account_number,
            receipt,
            username: auth.username,
            password: auth.password,
        })
    }
}

// PSync Response Transformation
impl TryFrom<ResponseRouterData<BamboraapacSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraapacSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        use common_utils::ext_traits::XmlExt;

        let router_data = &item.router_data;

        // Decode the HTML-encoded inner XML
        let inner_xml = item
            .response
            .body
            .query_transaction_response
            .query_transaction_result
            .replace("&lt;", "<")
            .replace("&gt;", ">");

        // Parse the inner QueryResponse XML
        let query_response: QueryResponse = inner_xml.as_str().parse_xml().change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "bamboraapac"),
        )?;

        // Check if response element exists
        let response = match &query_response.response {
            Some(resp) => resp,
            None => {
                // No matching transaction found
                return Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::Failure,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Err(ErrorResponse {
                        code: "NO_TRANSACTION_FOUND".to_string(),
                        message: "No matching transaction found".to_string(),
                        reason: Some("Transaction not found in query results".to_string()),
                        status_code: item.http_code,
                        attempt_status: Some(common_enums::AttemptStatus::Failure),
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    ..router_data.clone()
                });
            }
        };

        // Map Bambora response code to standard status (0 = Approved)
        let status = if response.response_code == 0 {
            if router_data.request.capture_method == Some(common_enums::CaptureMethod::Manual) {
                common_enums::AttemptStatus::Authorized
            } else {
                common_enums::AttemptStatus::Charged
            }
        } else {
            common_enums::AttemptStatus::Failure
        };

        // Handle transaction error responses
        if status == common_enums::AttemptStatus::Failure {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: response
                        .declined_code
                        .clone()
                        .unwrap_or_else(|| "DECLINED".to_string()),
                    message: response
                        .declined_message
                        .clone()
                        .unwrap_or_else(|| "Payment declined".to_string()),
                    reason: response.declined_message.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: Some(response.receipt.clone()),
                    network_decline_code: response.declined_code.clone(),
                    network_advice_code: None,
                    network_error_message: response.declined_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.receipt.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.receipt.clone()),
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

// ============================================================================
// REFUND FLOW TRANSFORMERS
// ============================================================================

// Refund Request Transformation
impl
    TryFrom<
        &RouterDataV2<
            domain_types::connector_flow::Refund,
            RefundFlowData,
            RefundsData,
            RefundsResponseData,
        >,
    > for BamboraapacRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            domain_types::connector_flow::Refund,
            RefundFlowData,
            RefundsData,
            RefundsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = BamboraapacAuthType::try_from(&router_data.connector_config)?;

        // Get the connector transaction ID to refund
        let receipt = router_data.request.connector_transaction_id.clone();

        Ok(Self {
            cust_ref: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            receipt,
            amount: router_data.request.minor_refund_amount,
            username: auth.username,
            password: auth.password,
        })
    }
}

// Refund Response Transformation
impl TryFrom<ResponseRouterData<BamboraapacRefundResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::Refund,
        RefundFlowData,
        RefundsData,
        RefundsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraapacRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        use common_utils::ext_traits::XmlExt;

        let router_data = &item.router_data;

        // Decode the HTML-encoded inner XML
        let inner_xml = item
            .response
            .body
            .submit_single_refund_response
            .submit_single_refund_result
            .replace("&lt;", "<")
            .replace("&gt;", ">");

        // Parse the inner RefundResponse XML
        let response: RefundResponseInner = inner_xml.as_str().parse_xml().change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "bamboraapac"),
        )?;

        // Map Bambora response code to standard refund status (0 = Approved)
        let refund_status = if response.response_code == 0 {
            common_enums::RefundStatus::Success
        } else {
            common_enums::RefundStatus::Failure
        };

        // Handle error responses
        if refund_status == common_enums::RefundStatus::Failure {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: response
                        .declined_code
                        .clone()
                        .unwrap_or_else(|| "DECLINED".to_string()),
                    message: response
                        .declined_message
                        .clone()
                        .unwrap_or_else(|| "Refund declined".to_string()),
                    reason: response.declined_message.clone(),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(response.receipt.clone()),
                    network_decline_code: response.declined_code.clone(),
                    network_advice_code: None,
                    network_error_message: response.declined_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success response
        let refund_response_data = RefundsResponseData {
            connector_refund_id: response.receipt.clone(),
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refund_response_data),
            ..router_data.clone()
        })
    }
}

// ============================================================================
// RSYNC FLOW TRANSFORMERS
// ============================================================================

// RSync Request Transformation (reuses BamboraapacSyncRequest)
impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for BamboraapacSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = BamboraapacAuthType::try_from(&router_data.connector_config)?;

        // Get the refund connector transaction ID to query
        let receipt = router_data.request.connector_refund_id.clone();

        Ok(Self {
            account_number: auth.account_number,
            receipt,
            username: auth.username,
            password: auth.password,
        })
    }
}

// RSync Response Transformation
impl TryFrom<ResponseRouterData<BamboraapacSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraapacSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        use common_utils::ext_traits::XmlExt;

        let router_data = &item.router_data;

        // Decode the HTML-encoded inner XML
        let inner_xml = item
            .response
            .body
            .query_transaction_response
            .query_transaction_result
            .replace("&lt;", "<")
            .replace("&gt;", ">");

        // Parse the inner QueryResponse XML
        let query_response: QueryResponse = inner_xml.as_str().parse_xml().change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "bamboraapac"),
        )?;

        // Check if response element exists
        let response = match &query_response.response {
            Some(resp) => resp,
            None => {
                // No matching transaction found
                return Ok(Self {
                    resource_common_data: RefundFlowData {
                        status: common_enums::RefundStatus::Failure,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Err(ErrorResponse {
                        code: "NO_TRANSACTION_FOUND".to_string(),
                        message: "No matching refund transaction found".to_string(),
                        reason: Some("Refund transaction not found in query results".to_string()),
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
        };

        // Map Bambora response code to standard refund status (0 = Approved)
        let refund_status = if response.response_code == 0 {
            common_enums::RefundStatus::Success
        } else {
            common_enums::RefundStatus::Failure
        };

        // Handle transaction error responses
        if refund_status == common_enums::RefundStatus::Failure {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: response
                        .declined_code
                        .clone()
                        .unwrap_or_else(|| "DECLINED".to_string()),
                    message: response
                        .declined_message
                        .clone()
                        .unwrap_or_else(|| "Refund status check failed".to_string()),
                    reason: response.declined_message.clone(),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(response.receipt.clone()),
                    network_decline_code: response.declined_code.clone(),
                    network_advice_code: None,
                    network_error_message: response.declined_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success response
        let refund_response_data = RefundsResponseData {
            connector_refund_id: response.receipt.clone(),
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refund_response_data),
            ..router_data.clone()
        })
    }
}

// ============================================================================
// SETUP MANDATE FLOW STRUCTURES
// ============================================================================

use domain_types::connector_types::SetupMandateRequestData;

// SetupMandate Request Structure (Customer Registration without payment)
#[derive(Debug, Clone)]
pub struct BamboraapacSetupMandateRequest {
    pub customer_storage_number: Option<String>,
    pub cust_number: String,
    pub card_number: Secret<String>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub card_holder_name: Secret<String>,
    pub username: Secret<String>,
    pub password: Secret<String>,
}

// SetupMandate Response Structure (Outer SOAP envelope)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BamboraapacSetupMandateResponse {
    #[serde(rename = "Body")]
    pub body: SetupMandateBodyResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SetupMandateBodyResponse {
    pub register_single_customer_response: RegisterSingleCustomerResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RegisterSingleCustomerResponse {
    pub register_single_customer_result: String, // HTML-encoded XML string
}

// Inner RegisterSingleCustomerResponse structure (after decoding HTML entities)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RegisterSingleCustomerResponseInner {
    pub return_value: u8,
    pub return_message: Option<String>,
    pub customer_id: Option<String>,
    pub cust_number: String,
    pub credit_card_token: Option<String>,
    pub action_code: Option<u8>,
}

// ============================================================================
// SETUP MANDATE FLOW TRANSFORMERS
// ============================================================================

// SetupMandate Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RouterDataV2<
            domain_types::connector_flow::SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    > for BamboraapacSetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            domain_types::connector_flow::SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = BamboraapacAuthType::try_from(&router_data.connector_config)?;

        // Extract card data from payment method data
        let card_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => Ok(card),
            _ => Err(IntegrationError::not_implemented(
                "Only card payment methods are supported for SetupMandate".to_string(),
            )),
        }?;

        // Get card number using peek() method
        let card_number_str = card_data.card_number.peek().to_string();

        // Generate customer number from customer_id or use connector request reference
        let cust_number = router_data
            .request
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string())
            .unwrap_or_else(|| {
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });

        Ok(Self {
            customer_storage_number: None, // Optional: Can be set if merchant wants specific storage numbering
            cust_number,
            card_number: Secret::new(card_number_str),
            exp_month: card_data.card_exp_month.clone(),
            exp_year: card_data.get_expiry_year_4_digit(),
            card_holder_name: card_data.card_holder_name.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "payment_method.card.card_holder_name",
                    context: Default::default(),
                },
            )?,
            username: auth.username,
            password: auth.password,
        })
    }
}

// SetupMandate Response Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BamboraapacSetupMandateResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraapacSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        use common_utils::ext_traits::XmlExt;

        let router_data = &item.router_data;

        // Decode the HTML-encoded inner XML
        let inner_xml = item
            .response
            .body
            .register_single_customer_response
            .register_single_customer_result
            .replace("&lt;", "<")
            .replace("&gt;", ">");

        // Parse the inner RegisterSingleCustomerResponse XML
        let response: RegisterSingleCustomerResponseInner =
            inner_xml.as_str().parse_xml().change_context(
                crate::utils::response_handling_fail_for_connector(item.http_code, "bamboraapac"),
            )?;

        // Map Bambora return_value to status
        // 0 = Successful, 1 = Invalid username/password, 2 = User does not belong to API User Group, etc.
        let status = if response.return_value == 0 {
            common_enums::AttemptStatus::Charged
        } else {
            common_enums::AttemptStatus::Failure
        };

        // Handle error responses
        if status == common_enums::AttemptStatus::Failure {
            let error_message = match response.return_value {
                1 => "Invalid username/password",
                2 => "User does not belong to an API User Group",
                4 => "Invalid CustomerStorageNumber",
                99 => "Exception encountered",
                _ => "Customer registration failed",
            };

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: format!("SETUP_MANDATE_ERROR_{}", response.return_value),
                    message: error_message.to_string(),
                    reason: Some(error_message.to_string()),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: Some(error_message.to_string()),
                }),
                ..router_data.clone()
            });
        }

        // Success response - customer registration successful
        // Use the credit_card_token as mandate_id for RepeatPayment
        // If not returned, fall back to customer_id or cust_number
        let connector_mandate_id = response
            .credit_card_token
            .clone()
            .or_else(|| response.customer_id.clone())
            .unwrap_or_else(|| response.cust_number.clone());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::NoResponseId,
            redirection_data: None,
            mandate_reference: Some(Box::new(domain_types::connector_types::MandateReference {
                connector_mandate_id: Some(connector_mandate_id.clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })),
            connector_metadata: Some(serde_json::json!({
                "customer_number": response.cust_number.clone(),
                "customer_id": response.customer_id.clone(),
                "credit_card_token": response.credit_card_token.clone(),
                "action_code": response.action_code
            })),
            network_txn_id: None,
            connector_response_reference_id: None,
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

// ============================================================================
// REPEAT PAYMENT FLOW STRUCTURES
// ============================================================================

// RepeatPayment Request Structure (Payment with tokenized card)
#[derive(Debug, Clone)]
pub struct BamboraapacRepeatPaymentRequest {
    pub account_number: Secret<String>,
    pub cust_ref: String,
    pub amount: MinorUnit,
    pub trn_type: BamboraapacTrnType,
    pub card_token: String, // The customer_id/token from SetupMandate
    pub username: Secret<String>,
    pub password: Secret<String>,
}

// ============================================================================
// REPEAT PAYMENT FLOW TRANSFORMERS
// ============================================================================

// RepeatPayment Request Transformation
impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    >
    TryFrom<
        &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
    > for BamboraapacRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = BamboraapacAuthType::try_from(&router_data.connector_config)?;

        // Extract the card token (customer_id from SetupMandate) from mandate_reference
        let token = match &router_data.request.mandate_reference {
            domain_types::connector_types::MandateReferenceId::ConnectorMandateId(mandate_ref) => {
                mandate_ref.get_connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    },
                )?
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::not_implemented(
                    "Only ConnectorMandateId is supported for RepeatPayment".to_string()
                )))
            }
        };

        // Determine transaction type based on capture method
        let trn_type = match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => BamboraapacTrnType::PreAuth,
            _ => BamboraapacTrnType::Purchase,
        };

        Ok(Self {
            account_number: auth.account_number,
            cust_ref: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: router_data.request.minor_amount,
            trn_type,
            card_token: token,
            username: auth.username,
            password: auth.password,
        })
    }
}

// RepeatPayment Response Transformation (reuses BamboraapacPaymentResponse)
impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<BamboraapacPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraapacPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        use common_utils::ext_traits::XmlExt;

        let router_data = &item.router_data;

        // Decode the HTML-encoded inner XML
        let inner_xml = item
            .response
            .body
            .submit_single_payment_response
            .submit_single_payment_result
            .replace("&lt;", "<")
            .replace("&gt;", ">");

        // Parse the inner Response XML
        let response: PaymentResponse = inner_xml.as_str().parse_xml().change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "bamboraapac"),
        )?;

        // Map Bambora response code to standard status
        // 0 = Approved, 1 = Not Approved
        let status = if response.response_code == 0 {
            if router_data.request.capture_method == Some(common_enums::CaptureMethod::Manual) {
                common_enums::AttemptStatus::Authorized
            } else {
                common_enums::AttemptStatus::Charged
            }
        } else {
            common_enums::AttemptStatus::Failure
        };

        // Handle error responses
        if status == common_enums::AttemptStatus::Failure {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: response
                        .declined_code
                        .clone()
                        .unwrap_or_else(|| "DECLINED".to_string()),
                    message: response
                        .declined_message
                        .clone()
                        .unwrap_or_else(|| "Payment declined".to_string()),
                    reason: response.declined_message.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: Some(response.receipt.clone()),
                    network_decline_code: response.declined_code.clone(),
                    network_advice_code: None,
                    network_error_message: response.declined_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.receipt.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.receipt.clone()),
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

// ============================================================================
// TYPE ALIASES TO AVOID DUPLICATE TEMPLATING STRUCTS IN MACRO FRAMEWORK
// ============================================================================

// These aliases ensure each flow has unique response types for the macro framework
pub type BamboraapacAuthorizeResponse = BamboraapacPaymentResponse;
pub type BamboraapacRepeatPaymentResponse = BamboraapacPaymentResponse;
pub type BamboraapacPSyncRequest = BamboraapacSyncRequest;
pub type BamboraapacPSyncResponse = BamboraapacSyncResponse;
pub type BamboraapacRSyncRequest = BamboraapacSyncRequest;
pub type BamboraapacRSyncResponse = BamboraapacSyncResponse;

// ============================================================================
// GETSOAP XML TRAIT IMPLEMENTATIONS FOR MACRO FRAMEWORK
// ============================================================================

use super::super::macros::GetSoapXml;

// Implement GetSoapXml for all request types
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> GetSoapXml
    for BamboraapacPaymentRequest<T>
{
    fn to_soap_xml(&self) -> String {
        self.to_soap_xml()
    }
}

impl GetSoapXml for BamboraapacCaptureRequest {
    fn to_soap_xml(&self) -> String {
        // Build the inner Capture XML using quick-xml
        let capture_xml = CaptureXml {
            receipt: self.receipt.clone(),
            amount: self.amount.get_amount_as_i64(),
            security: SecurityXml {
                username: self.username.peek().to_string(),
                password: self.password.peek().to_string(),
            },
        };

        // Serialize using quick-xml
        let capture_xml_string =
            quick_xml::se::to_string(&capture_xml).unwrap_or_else(|_| String::from("<Capture/>"));

        // Wrap in SOAP envelope
        format!(
            r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:dts="http://www.ippayments.com.au/interface/api/dts">
<soapenv:Body>
<dts:SubmitSingleCapture>
<dts:trnXML><![CDATA[{capture_xml_string}]]></dts:trnXML>
</dts:SubmitSingleCapture>
</soapenv:Body>
</soapenv:Envelope>"#
        )
    }
}

impl GetSoapXml for BamboraapacRefundRequest {
    fn to_soap_xml(&self) -> String {
        // Build the inner Refund XML using quick-xml
        let refund_xml = RefundXml {
            cust_ref: self.cust_ref.clone(),
            receipt: self.receipt.clone(),
            amount: self.amount.get_amount_as_i64(),
            security: SecurityXml {
                username: self.username.peek().to_string(),
                password: self.password.peek().to_string(),
            },
        };

        // Serialize using quick-xml
        let refund_xml_string =
            quick_xml::se::to_string(&refund_xml).unwrap_or_else(|_| String::from("<Refund/>"));

        // Wrap in SOAP envelope
        format!(
            r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:dts="http://www.ippayments.com.au/interface/api/dts">
<soapenv:Header/>
<soapenv:Body>
<dts:SubmitSingleRefund>
<dts:trnXML><![CDATA[{refund_xml_string}]]></dts:trnXML>
</dts:SubmitSingleRefund>
</soapenv:Body>
</soapenv:Envelope>"#
        )
    }
}

impl GetSoapXml for BamboraapacSyncRequest {
    fn to_soap_xml(&self) -> String {
        // Build the inner QueryTransaction XML using quick-xml
        let query_xml = QueryTransactionXml {
            criteria: QueryCriteriaXml {
                account_number: self.account_number.peek().to_string(),
                trn_start_timestamp: "2024-06-23 00:00:00",
                trn_end_timestamp: "2099-12-31 23:59:59",
                receipt: self.receipt.clone(),
            },
            security: SecurityXml {
                username: self.username.peek().to_string(),
                password: self.password.peek().to_string(),
            },
        };

        // Serialize using quick-xml
        let query_xml_string = quick_xml::se::to_string(&query_xml)
            .unwrap_or_else(|_| String::from("<QueryTransaction/>"));

        // Wrap in SOAP envelope
        format!(
            r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:dts="http://www.ippayments.com.au/interface/api/dts">
<soapenv:Header/>
<soapenv:Body>
<dts:QueryTransaction>
<dts:queryXML><![CDATA[{query_xml_string}]]></dts:queryXML>
</dts:QueryTransaction>
</soapenv:Body>
</soapenv:Envelope>"#
        )
    }
}

impl GetSoapXml for BamboraapacSetupMandateRequest {
    fn to_soap_xml(&self) -> String {
        format!(
            r#"
            <soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/"
            xmlns:sipp="http://www.ippayments.com.au/interface/api/sipp">
                <soapenv:Header/>
                <soapenv:Body>
                    <sipp:RegisterSingleCustomer>
                        <sipp:registerSingleCustomerXML>
                            <![CDATA[
                <Register>
                    <Customer>
                        <CustNumber>{}</CustNumber>
                        <CreditCard>
                            <CardNumber>{}</CardNumber>
                            <ExpM>{}</ExpM>
                            <ExpY>{}</ExpY>
                            <CardHolderName>{}</CardHolderName>
                        </CreditCard>
                    </Customer>
                    <Security>
                        <UserName>{}</UserName>
                        <Password>{}</Password>
                    </Security>
                </Register>
            ]]>
                        </sipp:registerSingleCustomerXML>
                    </sipp:RegisterSingleCustomer>
                </soapenv:Body>
            </soapenv:Envelope>
        "#,
            self.cust_number,
            self.card_number.peek(),
            self.exp_month.peek(),
            self.exp_year.peek(),
            self.card_holder_name.peek(),
            self.username.peek(),
            self.password.peek()
        )
    }
}

impl GetSoapXml for BamboraapacRepeatPaymentRequest {
    fn to_soap_xml(&self) -> String {
        format!(
            r#"
            <soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/"
            xmlns:dts="http://www.ippayments.com.au/interface/api/dts">
                <soapenv:Body>
                    <dts:SubmitSinglePayment>
                        <dts:trnXML>
                            <![CDATA[
        <Transaction>
            <CustRef>{}</CustRef>
            <Amount>{}</Amount>
            <TrnType>{}</TrnType>
            <AccountNumber>{}</AccountNumber>
            <CreditCard>
                <TokeniseAlgorithmID>2</TokeniseAlgorithmID>
                <CardNumber>{}</CardNumber>
            </CreditCard>
            <Security>
                    <UserName>{}</UserName>
                    <Password>{}</Password>
            </Security>
        </Transaction>
                            ]]>
                        </dts:trnXML>
                    </dts:SubmitSinglePayment>
                </soapenv:Body>
            </soapenv:Envelope>
        "#,
            self.cust_ref,
            self.amount.get_amount_as_i64(),
            i32::from(self.trn_type),
            self.account_number.peek(),
            self.card_token,
            self.username.peek(),
            self.password.peek()
        )
    }
}

// ============================================================================
// TRYFROM IMPLEMENTATIONS FOR MACRO FRAMEWORK WRAPPER
// ============================================================================

// These implementations delegate to the existing TryFrom implementations from &RouterDataV2
// The macro framework wraps RouterDataV2 in a BamboraapacRouterData struct created by the create_all_prerequisites! macro

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BamboraapacRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BamboraapacPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: super::BamboraapacRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&data.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BamboraapacRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for BamboraapacPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: super::BamboraapacRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&data.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BamboraapacRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for BamboraapacCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: super::BamboraapacRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&data.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BamboraapacRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for BamboraapacRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: super::BamboraapacRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&data.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BamboraapacRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    > for BamboraapacRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: super::BamboraapacRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&data.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BamboraapacRouterData<
            RouterDataV2<
                domain_types::connector_flow::SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BamboraapacSetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: super::BamboraapacRouterData<
            RouterDataV2<
                domain_types::connector_flow::SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&data.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BamboraapacRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BamboraapacRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: super::BamboraapacRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&data.router_data)
    }
}

// ============================================================================
// CLIENT AUTHENTICATION TOKEN FLOW STRUCTURES
// ============================================================================

/// Request to create a tokenization session via Bambora APAC's SOAP API.
/// Uses the TokeniseCreditCard method on the sipp.asmx endpoint.
/// For client authentication, we send a minimal request to obtain a session token.
#[derive(Debug, Clone)]
pub struct BamboraapacClientAuthRequest {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub account_number: Secret<String>,
}

impl
    TryFrom<
        &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    > for BamboraapacClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = BamboraapacAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            username: auth.username,
            password: auth.password,
            account_number: auth.account_number,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::BamboraapacRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BamboraapacClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: super::BamboraapacRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&data.router_data)
    }
}

impl GetSoapXml for BamboraapacClientAuthRequest {
    fn to_soap_xml(&self) -> String {
        format!(
            r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:sipp="http://www.ippayments.com.au/interface/api/sipp">
<soapenv:Header/>
<soapenv:Body>
<sipp:TokeniseCreditCard>
<sipp:tokeniseCreditCardXML><![CDATA[
<TokeniseCreditCard>
    <CardNumber>4242424242424242</CardNumber>
    <ExpM>12</ExpM>
    <ExpY>30</ExpY>
    <TokeniseAlgorithmID>2</TokeniseAlgorithmID>
    <UserName>{}</UserName>
    <Password>{}</Password>
</TokeniseCreditCard>
]]></sipp:tokeniseCreditCardXML>
</sipp:TokeniseCreditCard>
</soapenv:Body>
</soapenv:Envelope>"#,
            self.username.peek(),
            self.password.peek()
        )
    }
}

/// Bambora APAC SOAP response for TokeniseCreditCard
/// The outer SOAP envelope wrapping the tokenization response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BamboraapacClientAuthResponse {
    #[serde(rename = "Body")]
    pub body: ClientAuthBodyResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ClientAuthBodyResponse {
    pub tokenise_credit_card_response: TokeniseCreditCardResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TokeniseCreditCardResponse {
    pub tokenise_credit_card_result: String,
}

/// Inner tokenization response (after decoding HTML entities from CDATA)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TokeniseCreditCardResult {
    pub return_value: String,
}

// ClientAuthenticationToken Response Transformation
impl TryFrom<ResponseRouterData<BamboraapacClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BamboraapacClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        use common_utils::ext_traits::XmlExt;

        let response = item.response;

        // The result is HTML-encoded XML in the tokenise_credit_card_result field
        let result_str = &response
            .body
            .tokenise_credit_card_response
            .tokenise_credit_card_result;

        // Decode HTML entities and parse the inner XML
        let decoded = result_str
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&apos;", "'");

        // Parse inner XML to extract the token
        let inner_result: TokeniseCreditCardResult =
            decoded
                .parse_xml()
                .map_err(|_| ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                })?;

        let token = inner_result.return_value;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Bamboraapac(
                BamboraapacClientAuthenticationResponseDomain {
                    token: Secret::new(token),
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
