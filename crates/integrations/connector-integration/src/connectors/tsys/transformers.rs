use common_enums::AttemptStatus;
use common_utils::types::{MinorUnit, StringMinorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

use super::TsysRouterData;
use domain_types::errors::{ConnectorError, IntegrationError};

// ============================================================================
// Card Data Source Enum
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysCardDataSource {
    #[default]
    #[serde(rename = "INTERNET")]
    Internet,
    #[serde(rename = "SWIPE")]
    Swipe,
    #[serde(rename = "NFC")]
    Nfc,
    #[serde(rename = "EMV")]
    Emv,
    #[serde(rename = "EMV_CONTACTLESS")]
    EmvContactless,
    #[serde(rename = "BAR_CODE")]
    BarCode,
    #[serde(rename = "MANUAL")]
    Manual,
    #[serde(rename = "PHONE")]
    Phone,
    #[serde(rename = "MAIL")]
    Mail,
    #[serde(rename = "FALLBACK_SWIPE")]
    FallbackSwipe,
}

// ============================================================================
// Terminal Capability Enum
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTerminalCapability {
    #[default]
    #[serde(rename = "NO_TERMINAL_MANUAL")]
    NoTerminalManual,
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "MAGSTRIPE_READ_ONLY")]
    MagstripeReadOnly,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "ICC_CHIP_READ_ONLY")]
    IccChipReadOnly,
    #[serde(rename = "KEYED_ENTRY_ONLY")]
    KeyedEntryOnly,
    #[serde(rename = "MAGSTRIPE_CONTACTLESS_ONLY")]
    MagstripeContactlessOnly,
    #[serde(rename = "MAGSTRIPE_KEYED_ENTRY_ONLY")]
    MagstripeKeyedEntryOnly,
    #[serde(rename = "MAGSTRIPE_ICC_KEYED_ENTRY_ONLY")]
    MagstripeIccKeyedEntryOnly,
    #[serde(rename = "MAGSTRIPE_ICC_ONLY")]
    MagstripeIccOnly,
    #[serde(rename = "ICC_KEYED_ENTRY_ONLY")]
    IccKeyedEntryOnly,
    #[serde(rename = "ICC_CHIP_CONTACT_CONTACTLESS")]
    IccChipContactContactless,
    #[serde(rename = "ICC_CONTACTLESS_ONLY")]
    IccContactlessOnly,
    #[serde(rename = "OTHER_CAPABILITY_FOR_MASTERCARD")]
    OtherCapabilityForMastercard,
    #[serde(rename = "MAGSTRIPE_SIGNATURE_FOR_AMEX_ONLY")]
    MagstripeSignatureForAmexOnly,
}

// ============================================================================
// Terminal Operating Environment Enum
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTerminalOperatingEnvironment {
    #[default]
    #[serde(rename = "NO_TERMINAL")]
    NoTerminal,
    #[serde(rename = "ON_MERCHANT_PREMISES_ATTENDED")]
    OnMerchantPremisesAttended,
    #[serde(rename = "ON_MERCHANT_PREMISES_UNATTENDED")]
    OnMerchantPremisesUnattended,
    #[serde(rename = "OFF_MERCHANT_PREMISES_ATTENDED")]
    OffMerchantPremisesAttended,
    #[serde(rename = "OFF_MERCHANT_PREMISES_UNATTENDED")]
    OffMerchantPremisesUnattended,
    #[serde(rename = "ON_CUSTOMER_PREMISES_UNATTENDED")]
    OnCustomerPremisesUnattended,
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "ELECTRONIC_DELIVERY_AMEX")]
    ElectronicDeliveryAmex,
    #[serde(rename = "PHYSICAL_DELIVERY_AMEX")]
    PhysicalDeliveryAmex,
    #[serde(rename = "OFF_MERCHANT_PREMISES_MPOS")]
    OffMerchantPremisesMpos,
    #[serde(rename = "ON_MERCHANT_PREMISES_MPOS")]
    OnMerchantPremisesMpos,
    #[serde(rename = "OFF_MERCHANT_PREMISES_CUSTOMER_POS")]
    OffMerchantPremisesCustomerPos,
    #[serde(rename = "ON_MERCHANT_PREMISES_CUSTOMER_POS")]
    OnMerchantPremisesCustomerPos,
    #[serde(rename = "OFF_CUSTOMER_PREMISES_UNATTENDED")]
    OffCustomerPremisesUnattended,
}

// ============================================================================
// Cardholder Authentication Method Enum
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysCardholderAuthenticationMethod {
    #[default]
    #[serde(rename = "NOT_AUTHENTICATED")]
    NotAuthenticated,
    #[serde(rename = "PIN")]
    Pin,
    #[serde(rename = "ELECTRONIC_SIGNATURE_ANALYSIS")]
    ElectronicSignatureAnalysis,
    #[serde(rename = "MANUAL_SIGNATURE")]
    ManualSignature,
    #[serde(rename = "MANUAL_OTHER")]
    ManualOther,
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "SYSTEMATIC_OTHER")]
    SystematicOther,
    #[serde(rename = "E_TICKET_ENV_AMEX")]
    ETicketEnvAmex,
    #[serde(rename = "OFFLINE_PIN")]
    OfflinePin,
}

// ============================================================================
// Authentication Type
// ============================================================================

#[derive(Debug, Clone)]
pub struct TsysAuthType {
    pub device_id: Secret<String>,
    pub transaction_key: Secret<String>,
    pub developer_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TsysAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Tsys {
                device_id,
                transaction_key,
                developer_id,
                ..
            } => Ok(Self {
                device_id: device_id.to_owned(),
                transaction_key: transaction_key.to_owned(),
                developer_id: developer_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ============================================================================
// AUTHORIZE FLOW - Request/Response
// ============================================================================

#[derive(Debug, Serialize)]
pub enum TsysPaymentsRequest<T: PaymentMethodDataTypes> {
    Auth(TsysPaymentAuthSaleRequest<T>),
    Sale(TsysPaymentAuthSaleRequest<T>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysPaymentAuthSaleRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "deviceID")]
    device_id: Secret<String>,
    transaction_key: Secret<String>,
    card_data_source: TsysCardDataSource,
    transaction_amount: StringMinorUnit,
    currency_code: common_enums::enums::Currency,
    card_number: RawCardNumber<T>,
    expiration_date: Secret<String>,
    cvv2: Secret<String>,
    order_number: String,
    terminal_capability: TsysTerminalCapability,
    terminal_operating_environment: TsysTerminalOperatingEnvironment,
    cardholder_authentication_method: TsysCardholderAuthenticationMethod,
    #[serde(rename = "developerID")]
    developer_id: Secret<String>,
}

// TryFrom for macro compatibility - owned TsysRouterData
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TsysPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item_data: TsysRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item_data.router_data;
        if item.resource_common_data.is_three_ds() {
            return Err(IntegrationError::NotImplemented(
                ("Three_ds payments through Tsys".to_string()).into(),
                Default::default(),
            )
            .into());
        };

        match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let auth: TsysAuthType = TsysAuthType::try_from(&item.connector_config)?;

                let auth_data = TsysPaymentAuthSaleRequest {
                    device_id: auth.device_id,
                    transaction_key: auth.transaction_key,
                    card_data_source: TsysCardDataSource::Internet,
                    transaction_amount: item_data
                        .connector
                        .amount_converter
                        .convert(item.request.minor_amount, item.request.currency)
                        .change_context(IntegrationError::AmountConversionFailed {
                            context: Default::default(),
                        })?,
                    currency_code: item.request.currency,
                    card_number: card_data.card_number.clone(),
                    expiration_date: card_data
                        .get_card_expiry_month_year_2_digit_with_delimiter("/".to_owned())?,
                    cvv2: card_data.card_cvc.clone(),
                    order_number: item
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    terminal_capability: TsysTerminalCapability::NoTerminalManual,
                    terminal_operating_environment: TsysTerminalOperatingEnvironment::NoTerminal,
                    cardholder_authentication_method:
                        TsysCardholderAuthenticationMethod::NotAuthenticated,
                    developer_id: auth.developer_id,
                };

                // Check if auto-capture or manual capture
                if item.request.is_auto_capture() {
                    Ok(Self::Sale(auth_data))
                } else {
                    Ok(Self::Auth(auth_data))
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                ("Payment method not implemented".to_string()).into(),
                Default::default(),
            ))?,
        }
    }
}

// Response types for Authorize
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysPaymentStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTransactionStatus {
    Approved,
    Declined,
    Void,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysResponse {
    pub status: TsysPaymentStatus,
    pub response_code: String,
    pub response_message: String,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysErrorResponse {
    pub status: TsysPaymentStatus,
    pub response_code: String,
    pub response_message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TsysResponseTypes {
    SuccessResponse(TsysResponse),
    ErrorResponse(TsysErrorResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(clippy::enum_variant_names)]
pub enum TsysPaymentsResponse {
    AuthResponse(TsysResponseTypes),
    SaleResponse(TsysResponseTypes),
    CaptureResponse(TsysResponseTypes),
    VoidResponse(TsysResponseTypes),
}

// Separate wrapper types for each flow to avoid macro conflicts
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TsysAuthorizeResponse(pub TsysPaymentsResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TsysCaptureResponse(pub TsysPaymentsResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TsysVoidResponse(pub TsysPaymentsResponse);

fn get_payments_response(connector_response: TsysResponse, http_code: u16) -> PaymentsResponseData {
    PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(connector_response.transaction_id.clone()),
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: Some(connector_response.transaction_id),
        incremental_authorization_allowed: None,
        status_code: http_code,
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TsysAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let TsysAuthorizeResponse(response_data) = item.response;
        let (response, status) = match response_data {
            TsysPaymentsResponse::AuthResponse(resp) => match resp {
                TsysResponseTypes::SuccessResponse(auth_response) => {
                    // Check if the status is actually PASS or FAIL
                    match auth_response.status {
                        TsysPaymentStatus::Pass => (
                            Ok(get_payments_response(auth_response, item.http_code)),
                            AttemptStatus::Authorized,
                        ),
                        TsysPaymentStatus::Fail => {
                            let error_resp = TsysErrorResponse {
                                status: auth_response.status,
                                response_code: auth_response.response_code,
                                response_message: auth_response.response_message,
                            };
                            (
                                Err(get_error_response(&error_resp, item.http_code)),
                                AttemptStatus::AuthorizationFailed,
                            )
                        }
                    }
                }
                TsysResponseTypes::ErrorResponse(error_response) => (
                    Err(get_error_response(&error_response, item.http_code)),
                    AttemptStatus::AuthorizationFailed,
                ),
            },
            TsysPaymentsResponse::SaleResponse(resp) => match resp {
                TsysResponseTypes::SuccessResponse(sale_response) => {
                    // Check if the status is actually PASS or FAIL
                    match sale_response.status {
                        TsysPaymentStatus::Pass => (
                            Ok(get_payments_response(sale_response, item.http_code)),
                            AttemptStatus::Charged,
                        ),
                        TsysPaymentStatus::Fail => {
                            let error_resp = TsysErrorResponse {
                                status: sale_response.status,
                                response_code: sale_response.response_code,
                                response_message: sale_response.response_message,
                            };
                            (
                                Err(get_error_response(&error_resp, item.http_code)),
                                AttemptStatus::Failure,
                            )
                        }
                    }
                }
                TsysResponseTypes::ErrorResponse(error_response) => (
                    Err(get_error_response(&error_response, item.http_code)),
                    AttemptStatus::Failure,
                ),
            },
            _ => {
                let generic_error = TsysErrorResponse {
                    status: TsysPaymentStatus::Fail,
                    response_code: item.http_code.to_string(),
                    response_message: item.http_code.to_string(),
                };
                (
                    Err(get_error_response(&generic_error, item.http_code)),
                    AttemptStatus::Failure,
                )
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

// TryFrom for Capture flow
impl TryFrom<ResponseRouterData<TsysCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<TsysCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let TsysCaptureResponse(response_data) = item.response;
        let (response, status) = match response_data {
            TsysPaymentsResponse::CaptureResponse(resp) => match resp {
                TsysResponseTypes::SuccessResponse(capture_response) => {
                    // Check if the status is actually PASS or FAIL
                    match capture_response.status {
                        TsysPaymentStatus::Pass => (
                            Ok(get_payments_response(capture_response, item.http_code)),
                            AttemptStatus::Charged,
                        ),
                        TsysPaymentStatus::Fail => {
                            let error_resp = TsysErrorResponse {
                                status: capture_response.status,
                                response_code: capture_response.response_code,
                                response_message: capture_response.response_message,
                            };
                            (
                                Err(get_error_response(&error_resp, item.http_code)),
                                AttemptStatus::CaptureFailed,
                            )
                        }
                    }
                }
                TsysResponseTypes::ErrorResponse(error_response) => (
                    Err(get_error_response(&error_response, item.http_code)),
                    AttemptStatus::CaptureFailed,
                ),
            },
            _ => {
                let generic_error = TsysErrorResponse {
                    status: TsysPaymentStatus::Fail,
                    response_code: item.http_code.to_string(),
                    response_message: item.http_code.to_string(),
                };
                (
                    Err(get_error_response(&generic_error, item.http_code)),
                    AttemptStatus::CaptureFailed,
                )
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

// TryFrom for Void flow
impl TryFrom<ResponseRouterData<TsysVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<TsysVoidResponse, Self>) -> Result<Self, Self::Error> {
        let TsysVoidResponse(response_data) = item.response;
        let (response, status) = match response_data {
            TsysPaymentsResponse::VoidResponse(resp) => match resp {
                TsysResponseTypes::SuccessResponse(void_response) => {
                    // Check if the status is actually PASS or FAIL
                    match void_response.status {
                        TsysPaymentStatus::Pass => (
                            Ok(get_payments_response(void_response, item.http_code)),
                            AttemptStatus::Voided,
                        ),
                        TsysPaymentStatus::Fail => {
                            let error_resp = TsysErrorResponse {
                                status: void_response.status,
                                response_code: void_response.response_code,
                                response_message: void_response.response_message,
                            };
                            (
                                Err(get_error_response(&error_resp, item.http_code)),
                                AttemptStatus::VoidFailed,
                            )
                        }
                    }
                }
                TsysResponseTypes::ErrorResponse(error_response) => (
                    Err(get_error_response(&error_response, item.http_code)),
                    AttemptStatus::VoidFailed,
                ),
            },
            _ => {
                let generic_error = TsysErrorResponse {
                    status: TsysPaymentStatus::Fail,
                    response_code: item.http_code.to_string(),
                    response_message: item.http_code.to_string(),
                };
                (
                    Err(get_error_response(&generic_error, item.http_code)),
                    AttemptStatus::VoidFailed,
                )
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

// ============================================================================
// PSYNC FLOW - Request/Response
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysSearchTransactionRequest {
    #[serde(rename = "deviceID")]
    device_id: Secret<String>,
    transaction_key: Secret<String>,
    #[serde(rename = "transactionID")]
    transaction_id: String,
    #[serde(rename = "developerID")]
    developer_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TsysSyncRequest {
    search_transaction: TsysSearchTransactionRequest,
}

// Wrapper struct for PSync to avoid macro conflicts
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TsysPSyncRequest(TsysSyncRequest);

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TsysPSyncResponse(TsysSyncResponse);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for TsysPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item_data: TsysRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item_data.router_data;
        let auth: TsysAuthType = TsysAuthType::try_from(&item.connector_config)?;

        let search_transaction = TsysSearchTransactionRequest {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            transaction_id: item
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?,
            developer_id: auth.developer_id,
        };

        Ok(Self(TsysSyncRequest { search_transaction }))
    }
}

// PSync Response
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransactionDetails {
    #[serde(rename = "transactionID")]
    transaction_id: String,
    transaction_type: String,
    transaction_status: TsysTransactionStatus,
}

impl From<TsysTransactionDetails> for AttemptStatus {
    fn from(item: TsysTransactionDetails) -> Self {
        match item.transaction_status {
            TsysTransactionStatus::Approved => {
                if item.transaction_type.contains("Auth-Only") {
                    Self::Authorized
                } else {
                    Self::Charged
                }
            }
            TsysTransactionStatus::Void => Self::Voided,
            TsysTransactionStatus::Declined => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysPaymentsSyncResponse {
    pub status: TsysPaymentStatus,
    pub response_code: String,
    pub response_message: String,
    pub transaction_details: TsysTransactionDetails,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SearchResponseTypes {
    SuccessResponse(TsysPaymentsSyncResponse),
    ErrorResponse(TsysErrorResponse),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TsysSyncResponse {
    search_transaction_response: SearchResponseTypes,
}

fn get_payments_sync_response(
    connector_response: &TsysPaymentsSyncResponse,
    http_code: u16,
) -> PaymentsResponseData {
    PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(
            connector_response
                .transaction_details
                .transaction_id
                .clone(),
        ),
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: Some(
            connector_response
                .transaction_details
                .transaction_id
                .clone(),
        ),
        incremental_authorization_allowed: None,
        status_code: http_code,
    }
}

impl TryFrom<ResponseRouterData<TsysPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<TsysPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let TsysPSyncResponse(TsysSyncResponse {
            search_transaction_response,
        }) = item.response;
        let (response, status) = match search_transaction_response {
            SearchResponseTypes::SuccessResponse(search_response) => (
                Ok(get_payments_sync_response(&search_response, item.http_code)),
                AttemptStatus::from(search_response.transaction_details),
            ),
            SearchResponseTypes::ErrorResponse(error_response) => (
                Err(get_error_response(&error_response, item.http_code)),
                item.router_data.resource_common_data.status,
            ),
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
// CAPTURE FLOW - Request/Response
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysCaptureRequest {
    #[serde(rename = "deviceID")]
    device_id: Secret<String>,
    transaction_key: Secret<String>,
    transaction_amount: StringMinorUnit,
    #[serde(rename = "transactionID")]
    transaction_id: String,
    #[serde(rename = "developerID")]
    developer_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TsysPaymentsCaptureRequest {
    capture: TsysCaptureRequest,
}

// TryFrom for macro compatibility - owned TsysRouterData
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TsysPaymentsCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item_data: TsysRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item_data.router_data;
        let auth: TsysAuthType = TsysAuthType::try_from(&item.connector_config)?;

        let capture = TsysCaptureRequest {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            transaction_id: item
                .request
                .connector_transaction_id
                .clone()
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?,
            developer_id: auth.developer_id,
            transaction_amount: item_data
                .connector
                .amount_converter
                .convert(item.request.minor_amount_to_capture, item.request.currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
        };

        Ok(Self { capture })
    }
}

// ============================================================================
// VOID FLOW - Request/Response
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysCancelRequest {
    #[serde(rename = "deviceID")]
    device_id: Secret<String>,
    transaction_key: Secret<String>,
    #[serde(rename = "transactionID")]
    transaction_id: String,
    #[serde(rename = "developerID")]
    developer_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TsysPaymentsCancelRequest {
    void: TsysCancelRequest,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TsysPaymentsCancelRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item_data: TsysRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item_data.router_data;
        let auth: TsysAuthType = TsysAuthType::try_from(&item.connector_config)?;

        let void = TsysCancelRequest {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            transaction_id: item.request.connector_transaction_id.clone(),
            developer_id: auth.developer_id,
        };

        Ok(Self { void })
    }
}

// ============================================================================
// REFUND FLOW - Request/Response
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysReturnRequest {
    #[serde(rename = "deviceID")]
    device_id: Secret<String>,
    transaction_key: Secret<String>,
    transaction_amount: StringMinorUnit,
    #[serde(rename = "transactionID")]
    transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TsysRefundRequest {
    #[serde(rename = "Return")]
    return_request: TsysReturnRequest,
}

// TryFrom for macro compatibility - owned TsysRouterData
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for TsysRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item_data: TsysRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item_data.router_data;
        let auth: TsysAuthType = TsysAuthType::try_from(&item.connector_config)?;

        let return_request = TsysReturnRequest {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            transaction_amount: item_data
                .connector
                .amount_converter
                .convert(MinorUnit(item.request.refund_amount), item.request.currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
            transaction_id: item.request.connector_transaction_id.clone(),
        };

        Ok(Self { return_request })
    }
}

// Refund Response
impl From<TsysPaymentStatus> for common_enums::enums::RefundStatus {
    fn from(item: TsysPaymentStatus) -> Self {
        match item {
            TsysPaymentStatus::Pass => Self::Success,
            TsysPaymentStatus::Fail => Self::Failure,
        }
    }
}

impl From<TsysTransactionDetails> for common_enums::enums::RefundStatus {
    fn from(item: TsysTransactionDetails) -> Self {
        match item.transaction_status {
            // TSYS API uses Approved status for processing refunds
            TsysTransactionStatus::Approved => Self::Pending,
            TsysTransactionStatus::Void => Self::Success, // TSYS marks successful refunds as VOID
            TsysTransactionStatus::Declined => Self::Failure,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RefundResponse {
    return_response: TsysResponseTypes,
}

impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = match item.response.return_response {
            TsysResponseTypes::SuccessResponse(return_response) => Ok(RefundsResponseData {
                connector_refund_id: return_response.transaction_id,
                refund_status: common_enums::enums::RefundStatus::from(return_response.status),
                status_code: item.http_code,
            }),
            TsysResponseTypes::ErrorResponse(error_response) => {
                Err(get_error_response(&error_response, item.http_code))
            }
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// ============================================================================
// RSYNC FLOW - Request/Response
// ============================================================================

// Wrapper struct for RSync to avoid macro conflicts
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TsysRSyncRequest(TsysSyncRequest);

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TsysRSyncResponse(TsysSyncResponse);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysRouterData<RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>, T>,
    > for TsysRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item_data: TsysRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item_data.router_data;
        let auth: TsysAuthType = TsysAuthType::try_from(&item.connector_config)?;

        let search_transaction = TsysSearchTransactionRequest {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            transaction_id: item.request.connector_refund_id.clone(),
            developer_id: auth.developer_id,
        };

        Ok(Self(TsysSyncRequest { search_transaction }))
    }
}

impl TryFrom<ResponseRouterData<TsysRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<TsysRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let TsysRSyncResponse(TsysSyncResponse {
            search_transaction_response,
        }) = item.response;
        let response = match search_transaction_response {
            SearchResponseTypes::SuccessResponse(search_response) => Ok(RefundsResponseData {
                connector_refund_id: search_response.transaction_details.transaction_id.clone(),
                refund_status: common_enums::enums::RefundStatus::from(
                    search_response.transaction_details,
                ),
                status_code: item.http_code,
            }),
            SearchResponseTypes::ErrorResponse(error_response) => {
                Err(get_error_response(&error_response, item.http_code))
            }
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// ============================================================================
// ERROR RESPONSE HELPER
// ============================================================================

fn get_error_response(
    connector_response: &TsysErrorResponse,
    status_code: u16,
) -> domain_types::router_data::ErrorResponse {
    domain_types::router_data::ErrorResponse {
        code: connector_response.response_code.clone(),
        message: connector_response.response_message.clone(),
        reason: Some(connector_response.response_message.clone()),
        status_code,
        attempt_status: None,
        connector_transaction_id: None,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    }
}
