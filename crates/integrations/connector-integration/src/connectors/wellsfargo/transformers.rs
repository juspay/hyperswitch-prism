use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::consts;
use domain_types::errors::{ConnectorResponseTransformationError, IntegrationError};
use domain_types::payment_method_data::RawCardNumber;
use domain_types::{
    connector_flow::{Authorize, Capture, RSync, Refund, SetupMandate, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId, SetupMandateRequestData,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ErrorResponse},
    router_data_v2::RouterDataV2,
    utils::CardIssuer,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

// Re-export from common utils for use in this connector
pub use crate::utils::{convert_metadata_to_merchant_defined_info, MerchantDefinedInformation};

// Type alias for WellsfargoRouterData to avoid using super::
pub type WellsFargoRouterData<RD, T> = super::WellsfargoRouterData<RD, T>;

// REQUEST STRUCTURES

/// Commerce indicator for Wells Fargo
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommerceIndicator {
    CardPresent,
    Internet,
    Phone,
    International,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoPaymentsRequest<T: PaymentMethodDataTypes> {
    processing_information: ProcessingInformation,
    payment_information: PaymentInformation<T>,
    order_information: OrderInformationWithBill,
    client_reference_information: ClientReferenceInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_defined_information: Option<Vec<MerchantDefinedInformation>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingInformation {
    action_list: Option<Vec<WellsfargoActionsList>>,
    action_token_types: Option<Vec<WellsfargoActionsTokenType>>,
    authorization_options: Option<WellsfargoAuthorizationOptions>,
    commerce_indicator: CommerceIndicator,
    capture: Option<bool>,
    capture_options: Option<WellsfargoCaptureOptions>,
    payment_solution: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaymentInformation<T: PaymentMethodDataTypes> {
    Cards(Box<CardPaymentInformation<T>>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardPaymentInformation<T: PaymentMethodDataTypes> {
    card: Card<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card<T: PaymentMethodDataTypes> {
    number: RawCardNumber<T>,
    expiration_month: Secret<String>,
    expiration_year: Secret<String>,
    security_code: Option<Secret<String>>,
    #[serde(rename = "type")]
    card_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInformationWithBill {
    amount_details: Amount,
    bill_to: Option<BillTo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    total_amount: common_utils::types::StringMajorUnit,
    currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillTo {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    address1: Option<Secret<String>>,
    locality: Option<Secret<String>>,
    administrative_area: Option<Secret<String>>,
    postal_code: Option<Secret<String>>,
    country: Option<common_enums::CountryAlpha2>,
    email: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone_number: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientReferenceInformation {
    code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoCaptureRequest {
    processing_information: Option<ProcessingInformation>,
    order_information: OrderInformationWithBill,
    client_reference_information: ClientReferenceInformation,
    merchant_defined_information: Option<Vec<MerchantDefinedInformation>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInformationAmount {
    amount_details: Amount,
    #[serde(skip_serializing_if = "Option::is_none")]
    bill_to: Option<BillTo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoVoidRequest {
    client_reference_information: ClientReferenceInformation,
    reversal_information: ReversalInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    merchant_defined_information: Option<Vec<MerchantDefinedInformation>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReversalInformation {
    amount_details: Amount,
    reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WellsfargoRefundStatus {
    Succeeded,
    Transmitted,
    Failed,
    Pending,
    Voided,
    Cancelled,
}

impl From<WellsfargoRefundStatus> for RefundStatus {
    fn from(item: WellsfargoRefundStatus) -> Self {
        match item {
            WellsfargoRefundStatus::Succeeded | WellsfargoRefundStatus::Transmitted => {
                Self::Success
            }
            WellsfargoRefundStatus::Cancelled
            | WellsfargoRefundStatus::Failed
            | WellsfargoRefundStatus::Voided => Self::Failure,
            WellsfargoRefundStatus::Pending => Self::Pending,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoRefundRequest {
    order_information: OrderInformationAmount,
    client_reference_information: ClientReferenceInformation,
}

// MANDATE SUPPORT STRUCTURES

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WellsfargoActionsList {
    TokenCreate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WellsfargoActionsTokenType {
    PaymentInstrument,
    Customer,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoAuthorizationOptions {
    initiator: Option<WellsfargoPaymentInitiator>,
    merchant_initiated_transaction: Option<MerchantInitiatedTransaction>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantInitiatedTransaction {
    reason: Option<String>,
    previous_transaction_id: Option<Secret<String>>,
    original_authorized_amount: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoPaymentInitiator {
    #[serde(rename = "type")]
    initiator_type: Option<WellsfargoPaymentInitiatorTypes>,
    credential_stored_on_file: Option<bool>,
    stored_credential_used: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WellsfargoPaymentInitiatorTypes {
    Customer,
    Merchant,
}

/// Wells Fargo capture options
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoCaptureOptions {
    capture_sequence_number: u32,
    total_capture_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoZeroMandateRequest<T: PaymentMethodDataTypes> {
    processing_information: ProcessingInformation,
    payment_information: PaymentInformation<T>,
    order_information: OrderInformationWithBill,
    client_reference_information: ClientReferenceInformation,
}

// RESPONSE STRUCTURES

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoPaymentsResponse {
    pub id: String,
    pub status: Option<WellsfargoPaymentStatus>,
    pub status_information: Option<StatusInformation>, // For PSync/GET responses
    pub client_reference_information: Option<ClientReferenceInformation>,
    pub processor_information: Option<ClientProcessorInformation>,
    pub error_information: Option<WellsfargoErrorInformation>,
    pub token_information: Option<WellsfargoTokenInformation>, // For SetupMandate responses
    #[serde(rename = "_links")]
    pub links: Option<WellsfargoLinks>, // HATEOAS links to determine payment state
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoLinks {
    pub capture: Option<WellsfargoLink>,
    #[serde(rename = "self")]
    pub self_link: Option<WellsfargoLink>,
    pub auth_reversal: Option<WellsfargoLink>,
    pub void: Option<WellsfargoLink>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WellsfargoLink {
    pub href: String,
    pub method: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInformation {
    pub reason: Option<String>,
    pub message: Option<String>,
}

// Response structure for TSS (Transaction Search Service) endpoint
// Used for RSync (Refund Sync) to query transaction status
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoRSyncResponse {
    pub id: String,
    pub application_information: Option<RSyncApplicationInformation>,
    pub error_information: Option<WellsfargoErrorInformation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RSyncApplicationInformation {
    pub status: Option<WellsfargoRefundStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WellsfargoPaymentStatus {
    Authorized,
    AuthorizedPendingReview,
    Declined,
    InvalidRequest,
    PendingAuthentication,
    PendingReview,
    Reversed,
    PartialAuthorized,
    Transmitted,
    Pending,
    AuthorizedRiskDeclined,
    Voided,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientProcessorInformation {
    pub network_transaction_id: Option<String>,
    pub avs: Option<Avs>,
    pub card_verification: Option<CardVerification>,
    pub response_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Avs {
    pub code: Option<String>,
    pub code_raw: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardVerification {
    pub result_code: Option<String>,
    pub result_code_raw: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoErrorInformation {
    pub reason: Option<String>,
    pub message: Option<String>,
    pub details: Option<Vec<ErrorInfo>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorInfo {
    pub field: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoTokenInformation {
    pub payment_instrument: Option<WellsfargoPaymentInstrument>,
    pub customer: Option<WellsfargoCustomer>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoPaymentInstrument {
    pub id: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WellsfargoCustomer {
    pub id: Option<Secret<String>>,
}

// ERROR RESPONSE STRUCTURES

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WellsfargoErrorResponse {
    AuthenticationError(Box<WellsfargoAuthenticationErrorResponse>),
    NotAvailableError(NotAvailableErrorResponse),
    StandardError(StandardErrorResponse),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WellsfargoAuthenticationErrorResponse {
    pub response: AuthenticationErrorInformation,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthenticationErrorInformation {
    pub rmsg: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardErrorResponse {
    pub id: Option<String>, // Transaction ID if available in error response
    pub error_information: Option<WellsfargoErrorInformation>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
    pub details: Option<Vec<ErrorInfo>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NotAvailableErrorResponse {
    pub id: Option<String>, // Transaction ID if available in error response
    pub errors: Vec<NotAvailableErrorObject>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NotAvailableErrorObject {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub message: Option<String>,
}

// AUTH TYPE

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WellsfargoAuthType {
    pub api_key: Secret<String>,
    pub merchant_account: Secret<String>,
    pub api_secret: Secret<String>,
}

impl TryFrom<&domain_types::router_data::ConnectorSpecificConfig> for WellsfargoAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(
        auth_type: &domain_types::router_data::ConnectorSpecificConfig,
    ) -> Result<Self, Self::Error> {
        use domain_types::router_data::ConnectorSpecificConfig;
        match auth_type {
            ConnectorSpecificConfig::Wellsfargo {
                api_key,
                merchant_account,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.clone(),
                merchant_account: merchant_account.clone(),
                api_secret: api_secret.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// HELPER FUNCTIONS

/// Convert CardIssuer to CyberSource card type code
/// This is a local implementation for Wells Fargo only to avoid
/// affecting other connectors when new card types are added to the shared CardIssuer enum
fn card_issuer_to_string(card_issuer: CardIssuer) -> String {
    let card_type = match card_issuer {
        CardIssuer::AmericanExpress => "003",
        CardIssuer::Master => "002",
        CardIssuer::Maestro => "042",
        CardIssuer::Visa => "001",
        CardIssuer::Discover => "004",
        CardIssuer::DinersClub => "005",
        CardIssuer::CarteBlanche => "006",
        CardIssuer::JCB => "007",
        CardIssuer::CartesBancaires => "036",
        CardIssuer::UnionPay => "062",
    };
    card_type.to_string()
}

/// Helper function to build error response from Wellsfargo response
/// Used across all response transformations to avoid code duplication
fn build_error_response(
    response: &WellsfargoPaymentsResponse,
    http_code: u16,
    status: Option<AttemptStatus>,
    default_error_message: &str,
) -> ErrorResponse {
    let error_message = response
        .error_information
        .as_ref()
        .and_then(|info| info.message.clone())
        .or_else(|| {
            response
                .error_information
                .as_ref()
                .and_then(|info| info.reason.clone())
        })
        .unwrap_or_else(|| default_error_message.to_string());

    let error_code = response
        .error_information
        .as_ref()
        .and_then(|info| info.reason.clone());

    ErrorResponse {
        code: error_code.unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
        message: error_message.clone(),
        reason: Some(error_message),
        status_code: http_code,
        attempt_status: status,
        connector_transaction_id: Some(response.id.clone()),
        network_decline_code: response
            .processor_information
            .as_ref()
            .and_then(|info| info.response_code.clone()),
        network_advice_code: None,
        network_error_message: None,
    }
}

// REQUEST CONVERSION - TryFrom RouterDataV2 to WellsfargoPaymentsRequest

// Specific implementation for Authorize flow
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WellsFargoRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WellsfargoPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WellsFargoRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Access the router_data directly
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common_data = &router_data.resource_common_data;

        // Get payment method data
        let payment_information = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Use get_card_issuer for robust card type detection
                let card_issuer =
                    domain_types::utils::get_card_issuer(card_data.card_number.peek())
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "card_type",
                            context: Default::default(),
                        })
                        .attach_printable("Unable to determine card issuer from card number")?;
                let card_type = card_issuer_to_string(card_issuer);

                let card = Card {
                    number: card_data.card_number.clone(),
                    expiration_month: card_data.card_exp_month.clone(),
                    expiration_year: card_data.card_exp_year.clone(),
                    security_code: Some(card_data.card_cvc.clone()),
                    card_type: Some(card_type),
                };
                PaymentInformation::Cards(Box::new(CardPaymentInformation { card }))
            }
            // Connector supports these but not yet implemented
            PaymentMethodData::Wallet(_)
            | PaymentMethodData::CardToken(_)
            | PaymentMethodData::NetworkToken(_) => Err(IntegrationError::not_implemented(
                "Payment method supported by connector but not yet implemented".to_string(),
            ))?,
            // Connector does not support these payment methods
            PaymentMethodData::CardDetailsForNetworkTransactionId(_)
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
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::MobilePayment(_) => Err(IntegrationError::NotSupported {
                message: "Payment method".to_string(),
                connector: "Wellsfargo",
                context: Default::default(),
            })?,
        };

        // Get amount and currency - amount is in minor units (cents)
        let amount = request.minor_amount;
        let currency = request.currency;

        // Convert amount using the framework's amount converter
        let total_amount = item
            .connector
            .amount_converter
            .convert(amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount for Wells Fargo payment")?;

        let amount_details = Amount {
            total_amount,
            currency,
        };

        // Build billing information if available
        let billing = common_data.address.get_payment_billing();
        let email = request
            .email
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "email",
                context: Default::default(),
            })?;

        // Convert Email type to Secret<String>
        // Email wraps Secret<String, EmailStrategy>, we need to extract and re-wrap
        let email_inner = email.expose();
        let email_secret = Secret::new(email_inner.expose());

        let bill_to = billing
            .map(|addr| {
                let phone_number = addr.get_phone_with_country_code().ok();
                addr.address
                    .as_ref()
                    .map(|details| BillTo {
                        first_name: details.first_name.clone(),
                        last_name: details.last_name.clone(),
                        address1: details.line1.clone(),
                        locality: details.city.clone(),
                        administrative_area: details.to_state_code_as_optional().ok().flatten(),
                        postal_code: details.zip.clone(),
                        country: details.country,
                        email: email_secret.clone(),
                        phone_number: phone_number.clone(),
                    })
                    .unwrap_or_else(|| BillTo {
                        first_name: None,
                        last_name: None,
                        address1: None,
                        locality: None,
                        administrative_area: None,
                        postal_code: None,
                        country: None,
                        email: email_secret.clone(),
                        phone_number,
                    })
            })
            .or_else(|| {
                Some(BillTo {
                    first_name: None,
                    last_name: None,
                    address1: None,
                    locality: None,
                    administrative_area: None,
                    postal_code: None,
                    country: None,
                    email: email_secret.clone(),
                    phone_number: None,
                })
            });

        let order_information = OrderInformationWithBill {
            amount_details,
            bill_to,
        };

        // Processing information
        let processing_information = ProcessingInformation {
            commerce_indicator: CommerceIndicator::Internet,
            capture: request
                .capture_method
                .map(|method| matches!(method, common_enums::CaptureMethod::Automatic)),
            action_list: None,
            action_token_types: None,
            authorization_options: None,
            capture_options: None,
            payment_solution: None,
        };

        // Client reference - use payment_id from common data
        let client_reference_information = ClientReferenceInformation {
            code: Some(common_data.connector_request_reference_id.clone()),
        };

        // Merchant defined information from metadata
        let merchant_defined_information = request
            .metadata
            .clone()
            .map(|metadata| convert_metadata_to_merchant_defined_info(metadata.expose()));

        Ok(Self {
            processing_information,
            payment_information,
            order_information,
            client_reference_information,
            merchant_defined_information,
        })
    }
}

// CAPTURE REQUEST CONVERSION - TryFrom RouterDataV2 to WellsfargoCaptureRequest

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WellsFargoRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for WellsfargoCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WellsFargoRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common_data = &router_data.resource_common_data;

        // Amount information
        let amount = request.minor_amount_to_capture;
        let currency = request.currency;

        // Convert amount using the framework's amount converter
        let total_amount = item
            .connector
            .amount_converter
            .convert(amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount for Wells Fargo payment")?;

        let amount_details = Amount {
            total_amount,
            currency,
        };

        // Build bill_to if billing information is available
        let billing = common_data.address.get_payment_billing();
        let bill_to = billing.map(|addr| {
            let phone_number = addr.get_phone_with_country_code().ok();
            let email_secret = addr
                .email
                .clone()
                .map(|e| Secret::new(e.expose().expose()))
                .unwrap_or_else(|| Secret::new(String::new()));
            addr.address
                .as_ref()
                .map(|details| BillTo {
                    first_name: details.first_name.clone(),
                    last_name: details.last_name.clone(),
                    address1: details.line1.clone(),
                    locality: details.city.clone(),
                    administrative_area: details.to_state_code_as_optional().ok().flatten(),
                    postal_code: details.zip.clone(),
                    country: details.country,
                    email: email_secret.clone(),
                    phone_number: phone_number.clone(),
                })
                .unwrap_or_else(|| BillTo {
                    first_name: None,
                    last_name: None,
                    address1: None,
                    locality: None,
                    administrative_area: None,
                    postal_code: None,
                    country: None,
                    email: email_secret,
                    phone_number,
                })
        });

        let order_information = OrderInformationWithBill {
            amount_details,
            bill_to,
        };

        // Client reference - use connector_request_reference_id from common data
        let client_reference_information = ClientReferenceInformation {
            code: Some(common_data.connector_request_reference_id.clone()),
        };

        // Processing information with capture options
        let processing_information = ProcessingInformation {
            commerce_indicator: CommerceIndicator::Internet,
            capture: None,
            action_list: None,
            action_token_types: None,
            authorization_options: None,
            capture_options: Some(WellsfargoCaptureOptions {
                capture_sequence_number: 1,
                total_capture_count: 1,
            }),
            payment_solution: None,
        };

        // Merchant defined information from metadata
        let merchant_defined_information = request
            .metadata
            .clone()
            .map(|m| convert_metadata_to_merchant_defined_info(m.expose()));

        Ok(Self {
            processing_information: Some(processing_information),
            order_information,
            client_reference_information,
            merchant_defined_information,
        })
    }
}

// VOID REQUEST CONVERSION - TryFrom RouterDataV2 to WellsfargoVoidRequest

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WellsFargoRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for WellsfargoVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WellsFargoRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let common_data = &router_data.resource_common_data;
        let request = &router_data.request;

        // Amount information - must be provided in the request
        let amount = request
            .amount
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: Default::default(),
            })?;
        let currency = request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: Default::default(),
            })?;

        // Convert amount using the framework's amount converter
        let total_amount = item
            .connector
            .amount_converter
            .convert(amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount for Wells Fargo payment")?;

        let amount_details = Amount {
            total_amount,
            currency,
        };

        // Reversal information with amount and reason
        let reversal_information = ReversalInformation {
            amount_details,
            reason: request
                .cancellation_reason
                .clone()
                .unwrap_or_else(|| "Cancellation requested".to_string()),
        };

        let client_reference_information = ClientReferenceInformation {
            code: Some(common_data.connector_request_reference_id.clone()),
        };

        // Merchant defined information from metadata
        let merchant_defined_information = request
            .metadata
            .clone()
            .map(|m| convert_metadata_to_merchant_defined_info(m.expose()));

        Ok(Self {
            client_reference_information,
            reversal_information,
            merchant_defined_information,
        })
    }
}

// REFUND REQUEST CONVERSION - TryFrom RouterDataV2 to WellsfargoRefundRequest

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WellsFargoRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for WellsfargoRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WellsFargoRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // Amount information
        let amount = request.minor_refund_amount;
        let currency = request.currency;

        // Convert amount using the framework's amount converter
        let total_amount = item
            .connector
            .amount_converter
            .convert(amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount for Wells Fargo payment")?;

        let amount_details = Amount {
            total_amount,
            currency,
        };

        let order_information = OrderInformationAmount {
            amount_details,
            bill_to: None, // Refund doesn't need bill_to
        };

        // Client reference - use refund_id from request
        let client_reference_information = ClientReferenceInformation {
            code: Some(request.refund_id.clone()),
        };

        Ok(Self {
            order_information,
            client_reference_information,
        })
    }
}

// SETUPMANDATE REQUEST CONVERSION

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WellsFargoRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WellsfargoZeroMandateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WellsFargoRouterData<
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
        let common_data = &router_data.resource_common_data;

        // Get email - required for mandate setup
        let email = request
            .email
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "email",
                context: Default::default(),
            })?;
        let email_secret = Secret::new(email.peek().to_string());

        // Create billing information from address data
        let billing_address = common_data.address.get_payment_method_billing();

        let bill_to = billing_address
            .map(|addr| {
                let phone_number = addr.get_phone_with_country_code().ok();
                addr.address
                    .as_ref()
                    .map(|addr_details| BillTo {
                        first_name: addr_details.first_name.clone(),
                        last_name: addr_details.last_name.clone(),
                        address1: addr_details.line1.clone(),
                        locality: addr_details.city.clone(),
                        administrative_area: addr_details
                            .to_state_code_as_optional()
                            .ok()
                            .flatten(),
                        postal_code: addr_details.zip.clone(),
                        country: addr_details.country,
                        email: email_secret.clone(),
                        phone_number: phone_number.clone(),
                    })
                    .unwrap_or_else(|| BillTo {
                        first_name: request.customer_name.clone().map(Secret::new),
                        last_name: None,
                        address1: None,
                        locality: None,
                        administrative_area: None,
                        postal_code: None,
                        country: None,
                        email: email_secret.clone(),
                        phone_number,
                    })
            })
            .or_else(|| {
                // Fallback to minimal billing info if no address
                Some(BillTo {
                    first_name: request.customer_name.clone().map(Secret::new),
                    last_name: None,
                    address1: None,
                    locality: None,
                    administrative_area: None,
                    postal_code: None,
                    country: None,
                    email: email_secret.clone(),
                    phone_number: None,
                })
            });

        // Zero amount for mandate setup
        let order_information = OrderInformationWithBill {
            amount_details: Amount {
                total_amount: common_utils::types::StringMajorUnit::zero(),
                currency: request.currency,
            },
            bill_to,
        };

        // Processing information for mandate
        let processing_information = ProcessingInformation {
            commerce_indicator: CommerceIndicator::Internet,
            capture: Some(false),
            action_list: Some(vec![WellsfargoActionsList::TokenCreate]),
            action_token_types: Some(vec![
                WellsfargoActionsTokenType::PaymentInstrument,
                WellsfargoActionsTokenType::Customer,
            ]),
            authorization_options: Some(WellsfargoAuthorizationOptions {
                initiator: Some(WellsfargoPaymentInitiator {
                    initiator_type: Some(WellsfargoPaymentInitiatorTypes::Customer),
                    credential_stored_on_file: Some(true),
                    stored_credential_used: None,
                }),
                merchant_initiated_transaction: None,
            }),
            capture_options: None,
            payment_solution: None,
        };

        // Payment information from card
        let payment_information = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_issuer =
                    domain_types::utils::get_card_issuer(card_data.card_number.peek())
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "card_type",
                            context: Default::default(),
                        })
                        .attach_printable("Unable to determine card issuer from card number")?;
                let card_type = card_issuer_to_string(card_issuer);
                PaymentInformation::Cards(Box::new(CardPaymentInformation {
                    card: Card {
                        number: card_data.card_number.clone(),
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        security_code: Some(card_data.card_cvc.clone()),
                        card_type: Some(card_type),
                    },
                }))
            }
            _ => {
                return Err(IntegrationError::not_implemented(
                    "Payment method not supported for SetupMandate".to_string(),
                )
                .into());
            }
        };

        // Client reference - use payment_id
        let client_reference_information = ClientReferenceInformation {
            code: Some(common_data.connector_request_reference_id.clone()),
        };

        Ok(Self {
            processing_information,
            payment_information,
            order_information,
            client_reference_information,
        })
    }
}

// RESPONSE CONVERSION - TryFrom ResponseRouterData to RouterDataV2

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<WellsfargoPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<WellsfargoPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        // For Authorize flow, determine if it's auto-capture or manual based on capture_method
        let is_auto_capture = item
            .router_data
            .request
            .capture_method
            .map(|method| matches!(method, common_enums::CaptureMethod::Automatic))
            .unwrap_or(false);
        let status = map_attempt_status(
            &response.status,
            is_auto_capture,
            &response.error_information,
        );

        // Check if the payment was successful
        let response_data = if is_payment_successful(&response.status, &response.status_information)
        {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response
                    .processor_information
                    .as_ref()
                    .and_then(|info| info.network_transaction_id.clone()),
                connector_response_reference_id: response
                    .client_reference_information
                    .as_ref()
                    .and_then(|info| info.code.clone()),
                incremental_authorization_allowed: Some(status == AttemptStatus::Authorized),
                status_code: item.http_code,
            })
        } else {
            // Build error response using helper function
            Err(build_error_response(
                response,
                item.http_code,
                Some(status),
                consts::NO_ERROR_MESSAGE,
            ))
        };

        // Build connector response with additional payment method data
        let connector_response = response
            .processor_information
            .as_ref()
            .map(AdditionalPaymentMethodConnectorResponse::from)
            .map(ConnectorResponseData::with_additional_payment_method_data);

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// PSync Response Conversion - Handles GET response format which is different from Authorize
impl TryFrom<ResponseRouterData<WellsfargoPaymentsResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PSync,
        PaymentFlowData,
        domain_types::connector_types::PaymentsSyncData,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<WellsfargoPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // For PSync, check both status (Authorize response) and status_information (GET response)
        let is_success = is_payment_successful(&response.status, &response.status_information);

        let status = if is_success && response.status.is_none() {
            AttemptStatus::Authorized
        } else {
            // For PSync with status field, capture=false to correctly map "Authorized" to "Authorized" not "Charged"
            map_attempt_status(&response.status, false, &response.error_information)
        };

        // Check if the payment was successful
        let response_data = if is_success {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response
                    .processor_information
                    .as_ref()
                    .and_then(|info| info.network_transaction_id.clone()),
                connector_response_reference_id: response
                    .client_reference_information
                    .as_ref()
                    .and_then(|info| info.code.clone()),
                incremental_authorization_allowed: Some(status == AttemptStatus::Authorized),
                status_code: item.http_code,
            })
        } else {
            // Build error response using helper function
            Err(build_error_response(
                response,
                item.http_code,
                Some(status),
                consts::NO_ERROR_MESSAGE,
            ))
        };

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Capture Response Conversion - Reuses same response structure as Authorize
impl TryFrom<ResponseRouterData<WellsfargoPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<WellsfargoPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        // For Capture flow, capture=true
        let status = map_attempt_status(&response.status, true, &response.error_information);

        // Check if the capture was successful
        let response_data = if is_payment_successful(&response.status, &response.status_information)
        {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response
                    .processor_information
                    .as_ref()
                    .and_then(|info| info.network_transaction_id.clone()),
                connector_response_reference_id: response
                    .client_reference_information
                    .as_ref()
                    .and_then(|info| info.code.clone()),
                incremental_authorization_allowed: Some(status == AttemptStatus::Authorized),
                status_code: item.http_code,
            })
        } else {
            // Build error response using helper function
            Err(build_error_response(
                response,
                item.http_code,
                Some(status),
                consts::NO_ERROR_MESSAGE,
            ))
        };

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Void Response Conversion - Reuses same response structure as Authorize/Capture
impl TryFrom<ResponseRouterData<WellsfargoPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<WellsfargoPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        // For Void flow, capture=false
        let status = map_attempt_status(&response.status, false, &response.error_information);

        // Check if the void was successful
        let response_data = if is_payment_successful(&response.status, &response.status_information)
        {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response
                    .processor_information
                    .as_ref()
                    .and_then(|info| info.network_transaction_id.clone()),
                connector_response_reference_id: response
                    .client_reference_information
                    .as_ref()
                    .and_then(|info| info.code.clone()),
                incremental_authorization_allowed: Some(status == AttemptStatus::Authorized),
                status_code: item.http_code,
            })
        } else {
            // Build error response using helper function
            Err(build_error_response(
                response,
                item.http_code,
                Some(status),
                consts::NO_ERROR_MESSAGE,
            ))
        };

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// SETUPMANDATE RESPONSE CONVERSION

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<WellsfargoPaymentsResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<WellsfargoPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        // For SetupMandate flow, capture=false (zero-dollar auth)
        let mut status = map_attempt_status(&response.status, false, &response.error_information);

        // Check if the mandate setup was successful
        let response_data = if is_payment_successful(&response.status, &response.status_information)
        {
            // Extract mandate reference from token information
            // Wells Fargo returns both payment_instrument.id and customer.id in token_information
            // We store payment_instrument.id as the connector_mandate_id for future recurring payments
            let mandate_reference = response
                .token_information
                .as_ref()
                .and_then(|token_info| token_info.payment_instrument.as_ref())
                .map(|instrument| {
                    domain_types::connector_types::MandateReference {
                        connector_mandate_id: Some(instrument.id.clone().expose()),
                        payment_method_id: None, // Could potentially use token_information.customer.id here if needed
                        connector_mandate_request_reference_id: None,
                    }
                });

            // In case of zero auth mandates we want to make the payment reach the terminal status
            // so we are converting the authorized status to charged as well.
            if status == AttemptStatus::Authorized {
                status = AttemptStatus::Charged;
            }

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: None,
                network_txn_id: response
                    .processor_information
                    .as_ref()
                    .and_then(|info| info.network_transaction_id.clone()),
                connector_response_reference_id: response
                    .client_reference_information
                    .as_ref()
                    .and_then(|info| info.code.clone())
                    .or_else(|| Some(response.id.clone())),
                incremental_authorization_allowed: Some(status == AttemptStatus::Authorized),

                status_code: item.http_code,
            })
        } else {
            // Build error response using helper function
            Err(build_error_response(
                response,
                item.http_code,
                Some(status),
                consts::NO_ERROR_MESSAGE,
            ))
        };

        // Build connector response with additional payment method data
        let connector_response = response
            .processor_information
            .as_ref()
            .map(AdditionalPaymentMethodConnectorResponse::from)
            .map(ConnectorResponseData::with_additional_payment_method_data);

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Refund Response Conversion - Reuses same response structure as Authorize/Capture/Void
impl TryFrom<ResponseRouterData<WellsfargoPaymentsResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<WellsfargoPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let status = get_refund_status(&response.status, &response.error_information);

        // Check if the refund was successful
        let response_data = if is_payment_successful(&response.status, &response.status_information)
        {
            Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status: status,
                status_code: item.http_code,
            })
        } else {
            // Build error response using helper function
            Err(build_error_response(
                response,
                item.http_code,
                None,
                "Refund failed",
            ))
        };

        Ok(Self {
            response: response_data,
            resource_common_data: RefundFlowData {
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// RESPONSE CONVERSIONS - RSYNC (REFUND SYNC)

impl TryFrom<ResponseRouterData<WellsfargoRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<WellsfargoRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Extract status from application_information (TSS endpoint structure)
        let response_data = match response
            .application_information
            .as_ref()
            .and_then(|app_info| app_info.status.clone())
        {
            Some(refund_status) => {
                let status: RefundStatus = refund_status.clone().into();

                // Check if this is a failure status
                if matches!(status, RefundStatus::Failure) {
                    // Special handling for VOIDED status
                    if refund_status == WellsfargoRefundStatus::Voided {
                        Err(ErrorResponse {
                            code: consts::REFUND_VOIDED.to_string(),
                            message: consts::REFUND_VOIDED.to_string(),
                            reason: Some(consts::REFUND_VOIDED.to_string()),
                            status_code: item.http_code,
                            attempt_status: None,
                            connector_transaction_id: Some(response.id.clone()),
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                        })
                    } else {
                        // Other failure cases
                        Err(ErrorResponse {
                            code: response
                                .error_information
                                .as_ref()
                                .and_then(|info| info.reason.clone())
                                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                            message: response
                                .error_information
                                .as_ref()
                                .and_then(|info| info.message.clone())
                                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                            reason: response
                                .error_information
                                .as_ref()
                                .and_then(|info| info.message.clone()),
                            status_code: item.http_code,
                            attempt_status: None,
                            connector_transaction_id: Some(response.id.clone()),
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                        })
                    }
                } else {
                    // Success or pending status
                    Ok(RefundsResponseData {
                        connector_refund_id: response.id.clone(),
                        refund_status: status,
                        status_code: item.http_code,
                    })
                }
            }
            None => {
                // No status found - check for error information
                if let Some(error_info) = &response.error_information {
                    Err(ErrorResponse {
                        code: error_info
                            .reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                        message: error_info
                            .message
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                        reason: error_info.message.clone(),
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: Some(response.id.clone()),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    })
                } else {
                    // No status and no error - return unknown status error
                    Err(ErrorResponse {
                        code: consts::NO_ERROR_CODE.to_string(),
                        message: "Unable to determine refund status".to_string(),
                        reason: Some(consts::NO_ERROR_MESSAGE.to_string()),
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: Some(response.id.clone()),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    })
                }
            }
        };

        Ok(Self {
            response: response_data,
            resource_common_data: RefundFlowData {
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl From<&ClientProcessorInformation> for AdditionalPaymentMethodConnectorResponse {
    fn from(processor_information: &ClientProcessorInformation) -> Self {
        let payment_checks = Some(serde_json::json!({
                    "avs_response": processor_information.avs,
                    "card_verification": processor_information.card_verification
        }));

        Self::Card {
            authentication_data: None,
            payment_checks,
            card_network: None,
            domestic_network: None,
            auth_code: None,
        }
    }
}

fn is_payment_successful(
    status: &Option<WellsfargoPaymentStatus>,
    status_info: &Option<StatusInformation>,
) -> bool {
    // Check if status field indicates success
    let status_success = matches!(
        status,
        Some(WellsfargoPaymentStatus::Authorized)
            | Some(WellsfargoPaymentStatus::AuthorizedPendingReview)
            | Some(WellsfargoPaymentStatus::PartialAuthorized)
            | Some(WellsfargoPaymentStatus::Pending) // Capture operations return PENDING status
            | Some(WellsfargoPaymentStatus::Voided) // Void operations may return VOIDED status
            | Some(WellsfargoPaymentStatus::Reversed) // Void operations return REVERSED status
    );

    // For refund sync operations, check status_information.reason for "Success"
    let status_info_success = status_info
        .as_ref()
        .and_then(|info| info.reason.as_deref())
        .map(|reason| reason.eq_ignore_ascii_case("success"))
        .unwrap_or(false);

    status_success || status_info_success
}

/// Maps Wells Fargo payment status to AttemptStatus
/// The capture flag affects interpretation: Authorized+capture=true → Charged
fn map_attempt_status(
    status: &Option<WellsfargoPaymentStatus>,
    capture: bool,
    error_info: &Option<WellsfargoErrorInformation>,
) -> AttemptStatus {
    match status {
        Some(WellsfargoPaymentStatus::Authorized)
        | Some(WellsfargoPaymentStatus::AuthorizedPendingReview) => {
            if capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        }
        Some(WellsfargoPaymentStatus::Pending) => {
            if capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Pending
            }
        }
        Some(WellsfargoPaymentStatus::Transmitted) => AttemptStatus::Charged,
        Some(WellsfargoPaymentStatus::Voided) | Some(WellsfargoPaymentStatus::Reversed) => {
            AttemptStatus::Voided
        }
        Some(WellsfargoPaymentStatus::Declined)
        | Some(WellsfargoPaymentStatus::AuthorizedRiskDeclined)
        | Some(WellsfargoPaymentStatus::InvalidRequest) => AttemptStatus::Failure,
        Some(WellsfargoPaymentStatus::PendingAuthentication) => {
            AttemptStatus::AuthenticationPending
        }
        Some(WellsfargoPaymentStatus::PendingReview) => AttemptStatus::Pending,
        Some(WellsfargoPaymentStatus::PartialAuthorized) => {
            if capture {
                AttemptStatus::PartialCharged
            } else {
                AttemptStatus::Authorized
            }
        }
        None => {
            if error_info.is_some() {
                AttemptStatus::Failure
            } else {
                AttemptStatus::Pending
            }
        }
    }
}

/// Maps Wells Fargo payment status to RefundStatus
fn get_refund_status(
    status: &Option<WellsfargoPaymentStatus>,
    error_info: &Option<WellsfargoErrorInformation>,
) -> RefundStatus {
    match status {
        Some(WellsfargoPaymentStatus::Pending) => RefundStatus::Pending,
        Some(WellsfargoPaymentStatus::Transmitted) => RefundStatus::Pending,
        Some(WellsfargoPaymentStatus::Declined) => RefundStatus::Failure,
        Some(WellsfargoPaymentStatus::InvalidRequest) => RefundStatus::Failure,
        None => {
            if error_info.is_some() {
                RefundStatus::Failure
            } else {
                RefundStatus::Pending
            }
        }
        _ => RefundStatus::Success,
    }
}

/// Combines error information into a formatted error message
pub fn get_error_reason(
    error_info: Option<String>,
    detailed_error_info: Option<String>,
    avs_error_info: Option<String>,
) -> Option<String> {
    match (error_info, detailed_error_info, avs_error_info) {
        (Some(message), Some(details), Some(avs_message)) => Some(format!(
            "{message}, detailed_error_information: {details}, avs_message: {avs_message}",
        )),
        (Some(message), Some(details), None) => {
            Some(format!("{message}, detailed_error_information: {details}"))
        }
        (Some(message), None, Some(avs_message)) => {
            Some(format!("{message}, avs_message: {avs_message}"))
        }
        (None, Some(details), Some(avs_message)) => {
            Some(format!("{details}, avs_message: {avs_message}"))
        }
        (Some(message), None, None) => Some(message),
        (None, Some(details), None) => Some(details),
        (None, None, Some(avs_message)) => Some(avs_message),
        (None, None, None) => None,
    }
}
