use bytes::Bytes;
use common_enums::{self, AttemptStatus, CaptureMethod, Currency, FutureUsage};
use common_utils::{consts, date_time, ext_traits::Encode, types::MinorUnit, CustomResult};
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, SetupMandate, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, SetupMandateRequestData,
    },
    errors::ConnectorError,
    payment_address::AddressDetails,
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    utils::CardIssuer,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    types::ResponseRouterData,
    utils::{self},
};

use super::ArchipelRouterData;

const THREE_DS_MAX_SUPPORTED_VERSION: &str = "2.2.0";

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
#[serde(transparent)]
pub struct ArchipelTenantId(pub String);

impl From<String> for ArchipelTenantId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

pub struct ArchipelAuthType {
    pub(super) ca_certificate: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for ArchipelAuthType {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Archipel { api_key, .. } => {
                // Only use api_key as CA certificate if it looks like a PEM file
                let ca_certificate = api_key.peek().trim();
                let ca_certificate = if ca_certificate.starts_with("-----BEGIN") {
                    Some(api_key.to_owned())
                } else {
                    None
                };
                Ok(Self { ca_certificate })
            }
            _ => Err(ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ArchipelConfigData {
    pub tenant_id: ArchipelTenantId,
    pub platform_url: String,
}

impl TryFrom<&Option<Value>> for ArchipelConfigData {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(connector_metadata: &Option<Value>) -> Result<Self, Self::Error> {
        let config_data = to_connector_meta(connector_metadata.clone())?;
        Ok(config_data)
    }
}

impl TryFrom<&Option<Secret<Value>>> for ArchipelConfigData {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(connector_metadata: &Option<Secret<Value>>) -> Result<Self, Self::Error> {
        let metadata_value = connector_metadata.as_ref().map(|s| s.clone().expose());
        let config_data = to_connector_meta(metadata_value)?;
        Ok(config_data)
    }
}

fn to_connector_meta(
    connector_meta: Option<Value>,
) -> CustomResult<ArchipelConfigData, ConnectorError> {
    let meta_obj = connector_meta.ok_or_else(|| ConnectorError::NoConnectorMetaData)?;

    // Handle three cases:
    // Case 1: Direct String (for Refund flow)
    // Case 2: Object with "connector_meta_data" key (for other flows)
    // Case 3: Direct object with tenant_id and platform_url fields (from Hyperswitch)

    // Case 3: Direct object format - try to deserialize directly
    if let Ok(config_data) = serde_json::from_value::<ArchipelConfigData>(meta_obj.clone()) {
        return Ok(config_data);
    }

    // Case 1 & 2: Need to extract string first
    let connector_meta_str = if let Some(direct_str) = meta_obj.as_str() {
        // Case 1: Direct string, use it as is
        direct_str.to_string()
    } else {
        // Case 2: Object with nested "connector_meta_data" key
        meta_obj
            .get("connector_meta_data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConnectorError::InvalidDataFormat {
                field_name: "connector_meta_data",
            })?
            .to_string()
    };

    // Parse the JSON string to get ArchipelConfigData
    let config_data: ArchipelConfigData = serde_json::from_str(&connector_meta_str)
        .change_context(ConnectorError::InvalidDataFormat {
            field_name: "ArchipelConfigData",
        })?;

    Ok(config_data)
}

#[derive(Debug, Default, Serialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum ArchipelPaymentInitiator {
    #[default]
    Customer,
    Merchant,
}

#[derive(Debug, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ArchipelPaymentCertainty {
    #[default]
    Final,
    Estimated,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelOrderRequest {
    amount: MinorUnit,
    currency: String,
    certainty: ArchipelPaymentCertainty,
    initiator: ArchipelPaymentInitiator,
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
pub struct CardExpiryDate {
    month: Secret<String>,
    year: Secret<String>,
}

#[derive(Debug, Serialize, Default, Eq, PartialEq, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApplicationSelectionIndicator {
    #[default]
    ByDefault,
    CustomerChoice,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Archipel3DS {
    #[serde(rename = "acsTransID")]
    acs_trans_id: Option<Secret<String>>,
    #[serde(rename = "dsTransID")]
    ds_trans_id: Option<Secret<String>>,
    #[serde(rename = "3DSRequestorName")]
    three_ds_requestor_name: Option<Secret<String>>,
    #[serde(rename = "3DSAuthDate")]
    three_ds_auth_date: Option<String>,
    #[serde(rename = "3DSAuthAmt")]
    three_ds_auth_amt: Option<MinorUnit>,
    #[serde(rename = "3DSAuthStatus")]
    three_ds_auth_status: Option<String>,
    #[serde(rename = "3DSMaxSupportedVersion")]
    three_ds_max_supported_version: String,
    #[serde(rename = "3DSVersion")]
    three_ds_version: Option<common_utils::types::SemanticVersion>,
    authentication_value: Secret<String>,
    authentication_method: Option<Secret<String>>,
    eci: Option<String>,
}

impl From<AuthenticationData> for Archipel3DS {
    fn from(three_ds_data: AuthenticationData) -> Self {
        let now = date_time::date_as_yyyymmddthhmmssmmmz().ok();
        Self {
            acs_trans_id: None,
            ds_trans_id: three_ds_data.ds_trans_id.map(Secret::new),
            three_ds_requestor_name: None,
            three_ds_auth_date: now,
            three_ds_auth_amt: None,
            three_ds_auth_status: None,
            three_ds_max_supported_version: THREE_DS_MAX_SUPPORTED_VERSION.into(),
            three_ds_version: three_ds_data.message_version,
            authentication_value: three_ds_data.cavv.unwrap_or(Secret::new(String::new())),
            authentication_method: None,
            eci: three_ds_data.eci,
        }
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelCardHolder {
    billing_address: Option<ArchipelBillingAddress>,
}

impl From<Option<ArchipelBillingAddress>> for ArchipelCardHolder {
    fn from(value: Option<ArchipelBillingAddress>) -> Self {
        Self {
            billing_address: value,
        }
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelBillingAddress {
    address: Option<Secret<String>>,
    postal_code: Option<Secret<String>>,
}

pub trait ToArchipelBillingAddress {
    fn to_archipel_billing_address(&self) -> Option<ArchipelBillingAddress>;
}

impl ToArchipelBillingAddress for AddressDetails {
    fn to_archipel_billing_address(&self) -> Option<ArchipelBillingAddress> {
        let address = self.get_combined_address_line().ok();
        let postal_code = self.get_optional_zip();

        match (address, postal_code) {
            (None, None) => None,
            (addr, zip) => Some(ArchipelBillingAddress {
                address: addr,
                postal_code: zip,
            }),
        }
    }
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum ArchipelCredentialIndicatorStatus {
    Initial,
    Subsequent,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelCredentialIndicator {
    status: ArchipelCredentialIndicatorStatus,
    recurring: Option<bool>,
    transaction_id: Option<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenizedCardData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card_data: ArchipelTokenizedCard<T>,
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelTokenizedCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    number: RawCardNumber<T>,
    expiry: CardExpiryDate,
    scheme: ArchipelCardScheme,
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    number: RawCardNumber<T>,
    expiry: CardExpiryDate,
    security_code: Option<Secret<String>>,
    card_holder_name: Option<Secret<String>>,
    application_selection_indicator: ApplicationSelectionIndicator,
    scheme: ArchipelCardScheme,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(Option<Secret<String>>, Option<ArchipelCardHolder>, &Card<T>)> for ArchipelCard<T>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        (card_holder_name, card_holder_billing, ccard): (
            Option<Secret<String>>,
            Option<ArchipelCardHolder>,
            &Card<T>,
        ),
    ) -> Result<Self, Self::Error> {
        // NOTE: Archipel does not accept `card.card_holder_name` field without `cardholder` field.
        // So if `card_holder` is None, `card.card_holder_name` must also be None.
        // However, the reverse is allowed — the `cardholder` field can exist without `card.card_holder_name`.
        let card_holder_name = card_holder_billing
            .as_ref()
            .and_then(|_| ccard.card_holder_name.clone().or(card_holder_name.clone()));

        let raw_card = serde_json::to_string(&ccard.card_number.0)
            .change_context(ConnectorError::RequestEncodingFailed)
            .attach_printable("Failed to serialize card number")?
            .trim_matches('"')
            .to_string();
        let card_issuer = domain_types::utils::get_card_issuer(&raw_card).ok();
        let scheme = ArchipelCardScheme::from(card_issuer);

        Ok(Self {
            number: ccard.card_number.clone(),
            expiry: CardExpiryDate {
                month: ccard.card_exp_month.clone(),
                year: ccard.get_card_expiry_year_2_digit()?,
            },
            security_code: Some(ccard.card_cvc.clone()),
            application_selection_indicator: ApplicationSelectionIndicator::ByDefault,
            card_holder_name,
            scheme,
        })
    }
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelPaymentInformation {
    order: ArchipelOrderRequest,
    cardholder: Option<ArchipelCardHolder>,
    card_holder_name: Option<Secret<String>>,
    credential_indicator: Option<ArchipelCredentialIndicator>,
    stored_on_file: bool,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelCardAuthorizationRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    order: ArchipelOrderRequest,
    card: ArchipelCard<T>,
    cardholder: Option<ArchipelCardHolder>,
    #[serde(rename = "3DS")]
    three_ds: Option<Archipel3DS>,
    credential_indicator: Option<ArchipelCredentialIndicator>,
    stored_on_file: bool,
    tenant_id: ArchipelTenantId,
}

// PaymentsResponse

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ArchipelCardScheme {
    Amex,
    Mastercard,
    Visa,
    Discover,
    Diners,
    Unknown,
}

impl From<Option<CardIssuer>> for ArchipelCardScheme {
    fn from(card_issuer: Option<CardIssuer>) -> Self {
        match card_issuer {
            Some(CardIssuer::Visa) => Self::Visa,
            Some(CardIssuer::Master | CardIssuer::Maestro) => Self::Mastercard,
            Some(CardIssuer::AmericanExpress) => Self::Amex,
            Some(CardIssuer::Discover) => Self::Discover,
            Some(CardIssuer::DinersClub) => Self::Diners,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ArchipelPaymentStatus {
    #[default]
    Succeeded,
    Failed,
}

impl TryFrom<(AttemptStatus, CaptureMethod)> for ArchipelPaymentFlow {
    type Error = ConnectorError;

    fn try_from(
        (status, capture_method): (AttemptStatus, CaptureMethod),
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = matches!(capture_method, CaptureMethod::Automatic);

        match status {
            AttemptStatus::AuthenticationFailed => Ok(Self::Verify),
            AttemptStatus::Authorizing
            | AttemptStatus::Authorized
            | AttemptStatus::AuthorizationFailed => Ok(Self::Authorize),
            AttemptStatus::Voided | AttemptStatus::VoidInitiated | AttemptStatus::VoidFailed => {
                Ok(Self::Cancel)
            }
            AttemptStatus::CaptureInitiated | AttemptStatus::CaptureFailed => {
                if is_auto_capture {
                    Ok(Self::Pay)
                } else {
                    Ok(Self::Capture)
                }
            }
            AttemptStatus::PaymentMethodAwaited | AttemptStatus::ConfirmationAwaited => {
                if is_auto_capture {
                    Ok(Self::Pay)
                } else {
                    Ok(Self::Authorize)
                }
            }
            _ => Err(ConnectorError::ProcessingStepFailed(Some(
                Bytes::from_static(
                    "Impossible to determine Archipel flow from AttemptStatus".as_bytes(),
                ),
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ArchipelPaymentFlow {
    Verify,
    Authorize,
    Pay,
    Capture,
    Cancel,
}

struct ArchipelFlowStatus {
    status: ArchipelPaymentStatus,
    flow: ArchipelPaymentFlow,
}
impl ArchipelFlowStatus {
    fn new(status: ArchipelPaymentStatus, flow: ArchipelPaymentFlow) -> Self {
        Self { status, flow }
    }
}

impl From<ArchipelFlowStatus> for AttemptStatus {
    fn from(ArchipelFlowStatus { status, flow }: ArchipelFlowStatus) -> Self {
        match status {
            ArchipelPaymentStatus::Succeeded => match flow {
                ArchipelPaymentFlow::Authorize => Self::Authorized,
                ArchipelPaymentFlow::Pay
                | ArchipelPaymentFlow::Verify
                | ArchipelPaymentFlow::Capture => Self::Charged,
                ArchipelPaymentFlow::Cancel => Self::Voided,
            },
            ArchipelPaymentStatus::Failed => match flow {
                ArchipelPaymentFlow::Authorize | ArchipelPaymentFlow::Pay => {
                    Self::AuthorizationFailed
                }
                ArchipelPaymentFlow::Verify => Self::AuthenticationFailed,
                ArchipelPaymentFlow::Capture => Self::CaptureFailed,
                ArchipelPaymentFlow::Cancel => Self::VoidFailed,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelOrderResponse {
    id: String,
    amount: Option<MinorUnit>,
    currency: Option<Currency>,
    captured_amount: Option<MinorUnit>,
    authorized_amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchipelErrorMessage {
    pub code: String,
    pub description: Option<String>,
}

impl Default for ArchipelErrorMessage {
    fn default() -> Self {
        Self {
            code: consts::NO_ERROR_CODE.to_string(),
            description: Some(consts::NO_ERROR_MESSAGE.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchipelErrorMessageWithHttpCode {
    pub error_message: ArchipelErrorMessage,
    pub http_code: u16,
}

impl ArchipelErrorMessageWithHttpCode {
    pub fn new(error_message: ArchipelErrorMessage, http_code: u16) -> Self {
        Self {
            error_message,
            http_code,
        }
    }

    pub fn from_response(response: &[u8], http_code: u16) -> Self {
        let error_message = if response.is_empty() {
            ArchipelErrorMessage::default()
        } else {
            serde_json::from_slice::<ArchipelErrorMessage>(response).unwrap_or_else(|error| {
                tracing::warn!(
                    error = ?error,
                    "failed to deserialize ArchipelErrorMessage, using default"
                );
                ArchipelErrorMessage::default()
            })
        };

        Self {
            error_message,
            http_code,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelTransactionMetadata {
    #[serde(alias = "transaction_id")]
    pub transaction_id: String,
    #[serde(alias = "transaction_date")]
    pub transaction_date: String,
    #[serde(alias = "financial_network_code")]
    pub financial_network_code: Option<String>,
    #[serde(alias = "issuer_transaction_id")]
    pub issuer_transaction_id: Option<String>,
    #[serde(alias = "response_code")]
    pub response_code: Option<String>,
    #[serde(alias = "authorization_code")]
    pub authorization_code: Option<String>,
    #[serde(alias = "payment_account_reference")]
    pub payment_account_reference: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelPaymentsResponse {
    order: ArchipelOrderResponse,
    transaction_id: String,
    transaction_date: String,
    transaction_result: ArchipelPaymentStatus,
    error: Option<ArchipelErrorMessage>,
    financial_network_code: Option<String>,
    issuer_transaction_id: Option<String>,
    response_code: Option<String>,
    authorization_code: Option<String>,
    payment_account_reference: Option<Secret<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ArchipelPSyncResponse(ArchipelPaymentsResponse);

impl std::ops::Deref for ArchipelPSyncResponse {
    type Target = ArchipelPaymentsResponse;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&ArchipelPaymentsResponse> for ArchipelTransactionMetadata {
    fn from(payment_response: &ArchipelPaymentsResponse) -> Self {
        Self {
            transaction_id: payment_response.transaction_id.clone(),
            transaction_date: payment_response.transaction_date.clone(),
            financial_network_code: payment_response.financial_network_code.clone(),
            issuer_transaction_id: payment_response.issuer_transaction_id.clone(),
            response_code: payment_response.response_code.clone(),
            authorization_code: payment_response.authorization_code.clone(),
            payment_account_reference: payment_response.payment_account_reference.clone(),
        }
    }
}

// CAPTURE FLOW
#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct ArchipelCaptureRequest {
    order: ArchipelCaptureOrderRequest,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct ArchipelCaptureOrderRequest {
    amount: MinorUnit,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ArchipelCaptureResponse(ArchipelPaymentsResponse);

impl std::ops::Deref for ArchipelCaptureResponse {
    type Target = ArchipelPaymentsResponse;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ArchipelRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for ArchipelCaptureRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ArchipelRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            order: ArchipelCaptureOrderRequest {
                amount: MinorUnit::new(item.router_data.request.amount_to_capture),
            },
        })
    }
}

impl TryFrom<ResponseRouterData<ArchipelCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ArchipelCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(error) = item.response.0.error {
            return Ok(Self {
                response: Err(ArchipelErrorMessageWithHttpCode::new(error, item.http_code).into()),
                ..item.router_data
            });
        };

        let connector_metadata: Option<Value> = ArchipelTransactionMetadata::from(&item.response.0)
            .encode_to_value()
            .ok();

        let status: AttemptStatus = ArchipelFlowStatus::new(
            item.response.0.transaction_result,
            ArchipelPaymentFlow::Capture,
        )
        .into();

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.0.order.id),
                status_code: item.http_code,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
            }),
            ..item.router_data
        })
    }
}

// AUTHORIZATION FLOW
impl<T: PaymentMethodDataTypes>
    TryFrom<(
        MinorUnit,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for ArchipelPaymentInformation
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        (amount, router_data): (
            MinorUnit,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        let is_recurring_payment = router_data
            .request
            .mandate_id
            .as_ref()
            .and_then(|mandate_ids| mandate_ids.mandate_id.as_ref())
            .is_some();

        let is_subsequent_trx = router_data
            .request
            .mandate_id
            .as_ref()
            .and_then(|mandate_ids| mandate_ids.mandate_reference_id.as_ref())
            .is_some();

        let is_saved_card_payment = (router_data.request.is_mandate_payment())
            | (router_data.request.setup_future_usage == Some(FutureUsage::OnSession));

        let certainty = if router_data
            .request
            .request_incremental_authorization
            .unwrap_or(false)
        {
            if is_recurring_payment {
                ArchipelPaymentCertainty::Final
            } else {
                ArchipelPaymentCertainty::Estimated
            }
        } else {
            ArchipelPaymentCertainty::Final
        };

        let transaction_initiator = if is_recurring_payment {
            ArchipelPaymentInitiator::Merchant
        } else {
            ArchipelPaymentInitiator::Customer
        };

        let order = ArchipelOrderRequest {
            amount,
            currency: router_data.request.currency.to_string(),
            certainty,
            initiator: transaction_initiator.clone(),
        };

        let cardholder = router_data
            .resource_common_data
            .get_billing_address()
            .ok()
            .and_then(|address| address.to_archipel_billing_address())
            .map(|billing_address| ArchipelCardHolder {
                billing_address: Some(billing_address),
            });

        // NOTE: Archipel does not accept `card.card_holder_name` field without `cardholder` field.
        // So if `card_holder` is None, `card.card_holder_name` must also be None.
        // However, the reverse is allowed — the `cardholder` field can exist without `card.card_holder_name`.
        let card_holder_name = cardholder.as_ref().and_then(|_| {
            router_data
                .resource_common_data
                .get_billing()
                .ok()
                .and_then(|billing| billing.get_optional_full_name())
        });

        let indicator_status = if is_subsequent_trx {
            ArchipelCredentialIndicatorStatus::Subsequent
        } else {
            ArchipelCredentialIndicatorStatus::Initial
        };

        let stored_on_file =
            is_saved_card_payment | router_data.request.is_customer_initiated_mandate_payment();

        let credential_indicator = stored_on_file.then(|| ArchipelCredentialIndicator {
            status: indicator_status.clone(),
            recurring: Some(is_recurring_payment),
            transaction_id: match indicator_status {
                ArchipelCredentialIndicatorStatus::Initial => None,
                ArchipelCredentialIndicatorStatus::Subsequent => {
                    router_data.request.get_optional_network_transaction_id()
                }
            },
        });

        Ok(Self {
            order,
            cardholder,
            card_holder_name,
            credential_indicator,
            stored_on_file,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ArchipelRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ArchipelCardAuthorizationRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ArchipelRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_information: ArchipelPaymentInformation = ArchipelPaymentInformation::try_from(
            (item.router_data.request.amount, &item.router_data),
        )?;
        let payment_method_data = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ccard) => ArchipelCard::try_from((
                payment_information.card_holder_name,
                payment_information.cardholder.clone(),
                ccard,
            ))?,
            PaymentMethodData::CardDetailsForNetworkTransactionId(..)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(..)
            | PaymentMethodData::CardRedirect(..)
            | PaymentMethodData::Wallet(..)
            | PaymentMethodData::PayLater(..)
            | PaymentMethodData::BankRedirect(..)
            | PaymentMethodData::BankDebit(..)
            | PaymentMethodData::BankTransfer(..)
            | PaymentMethodData::Crypto(..)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(..)
            | PaymentMethodData::Upi(..)
            | PaymentMethodData::Voucher(..)
            | PaymentMethodData::GiftCard(..)
            | PaymentMethodData::CardToken(..)
            | PaymentMethodData::OpenBanking(..)
            | PaymentMethodData::NetworkToken(..)
            | PaymentMethodData::MobilePayment(..) => Err(ConnectorError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Archipel"),
            ))?,
        };

        // Extract 3DS authentication data if available
        // 3DS data comes from completed authentication flow and is stored in metadata
        let three_ds: Option<Archipel3DS> =
            item.router_data
                .request
                .metadata
                .as_ref()
                .and_then(|metadata| {
                    // Expose the Secret<Value> to get the inner Value
                    let exposed_metadata = metadata.clone().expose();
                    // Extract individual 3DS fields from metadata
                    let auth_data = exposed_metadata.get("authentication_data")?;

                    // Extract CAVV (required field for 3DS)
                    let cavv = auth_data
                        .get("cavv")
                        .and_then(|v: &Value| v.as_str())
                        .map(|s: &str| Secret::new(s.to_string()))?;

                    // Extract optional fields
                    let eci = auth_data
                        .get("eci")
                        .and_then(|v: &Value| v.as_str())
                        .map(String::from);
                    let ds_trans_id = auth_data
                        .get("ds_trans_id")
                        .and_then(|v: &Value| v.as_str())
                        .map(String::from);
                    let message_version = auth_data
                        .get("message_version")
                        .and_then(|v: &Value| v.as_str())
                        .and_then(|s: &str| s.parse::<common_utils::types::SemanticVersion>().ok());

                    let now = date_time::date_as_yyyymmddthhmmssmmmz().ok();

                    Some(Archipel3DS {
                        acs_trans_id: None,
                        ds_trans_id: ds_trans_id.map(Secret::new),
                        three_ds_requestor_name: None,
                        three_ds_auth_date: now,
                        three_ds_auth_amt: None,
                        three_ds_auth_status: None,
                        three_ds_max_supported_version: THREE_DS_MAX_SUPPORTED_VERSION.into(),
                        three_ds_version: message_version,
                        authentication_value: cavv,
                        authentication_method: None,
                        eci,
                    })
                });

        let connector_metadata =
            ArchipelConfigData::try_from(&item.router_data.request.connector_feature_data)?;

        Ok(Self {
            order: payment_information.order,
            cardholder: payment_information.cardholder,
            card: payment_method_data,
            three_ds,
            credential_indicator: payment_information.credential_indicator,
            stored_on_file: payment_information.stored_on_file,
            tenant_id: connector_metadata.tenant_id,
        })
    }
}

// Responses for AUTHORIZATION FLOW
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ArchipelPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ArchipelPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(error) = item.response.error {
            return Ok(Self {
                response: Err(ArchipelErrorMessageWithHttpCode::new(error, item.http_code).into()),
                ..item.router_data
            });
        };

        let capture_method = item
            .router_data
            .request
            .capture_method
            .ok_or_else(|| ConnectorError::CaptureMethodNotSupported)?;

        let (archipel_flow, is_incremental_allowed) = match capture_method {
            CaptureMethod::Automatic => (ArchipelPaymentFlow::Pay, false),
            _ => (
                ArchipelPaymentFlow::Authorize,
                item.router_data
                    .request
                    .request_incremental_authorization
                    .unwrap_or(false),
            ),
        };

        let connector_metadata: Option<Value> = ArchipelTransactionMetadata::from(&item.response)
            .encode_to_value()
            .ok();

        let status: AttemptStatus =
            ArchipelFlowStatus::new(item.response.transaction_result, archipel_flow).into();

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order.id),
                status_code: item.http_code,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                // Save archipel initial transaction uuid for network transaction mit/cit
                network_txn_id: item
                    .router_data
                    .request
                    .is_customer_initiated_mandate_payment()
                    .then_some(item.response.transaction_id),
                connector_response_reference_id: None,
                incremental_authorization_allowed: Some(is_incremental_allowed),
            }),
            ..item.router_data
        })
    }
}

// PSYNC FLOW
impl<F> TryFrom<ResponseRouterData<ArchipelPSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ArchipelPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(error) = item.response.0.error {
            return Ok(Self {
                response: Err(ArchipelErrorMessageWithHttpCode::new(error, item.http_code).into()),
                ..item.router_data
            });
        };

        let connector_metadata: Option<Value> = ArchipelTransactionMetadata::from(&item.response.0)
            .encode_to_value()
            .ok();

        let capture_method = item.router_data.request.capture_method.ok_or(
            ConnectorError::MissingRequiredField {
                field_name: "capture_method",
            },
        )?;

        let archipel_flow = match capture_method {
            CaptureMethod::Automatic => ArchipelPaymentFlow::Pay,
            _ => ArchipelPaymentFlow::Authorize,
        };

        let status: AttemptStatus =
            ArchipelFlowStatus::new(item.response.0.transaction_result.clone(), archipel_flow)
                .into();

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.0.order.id.clone()),
                status_code: item.http_code,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
            }),
            ..item.router_data
        })
    }
}

// VOID FLOW (Cancel Payment)
#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelVoidRequest {
    tenant_id: ArchipelTenantId,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ArchipelVoidResponse(ArchipelPaymentsResponse);

impl std::ops::Deref for ArchipelVoidResponse {
    type Target = ArchipelPaymentsResponse;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ArchipelRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for ArchipelVoidRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ArchipelRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract the value from Secret wrapper
        let metadata_value = item
            .router_data
            .request
            .connector_feature_data
            .as_ref()
            .map(|secret: &Secret<Value>| secret.clone().expose());

        let connector_metadata = ArchipelConfigData::try_from(&metadata_value)?;

        Ok(Self {
            tenant_id: connector_metadata.tenant_id,
        })
    }
}

impl<F> TryFrom<ResponseRouterData<ArchipelVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<ArchipelVoidResponse, Self>) -> Result<Self, Self::Error> {
        if let Some(error) = item.response.0.error {
            return Ok(Self {
                response: Err(ArchipelErrorMessageWithHttpCode::new(error, item.http_code).into()),
                ..item.router_data
            });
        };

        let connector_metadata: Option<Value> = ArchipelTransactionMetadata::from(&item.response.0)
            .encode_to_value()
            .ok();

        let status: AttemptStatus = ArchipelFlowStatus::new(
            item.response.0.transaction_result,
            ArchipelPaymentFlow::Cancel,
        )
        .into();

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.0.order.id),
                status_code: item.http_code,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
            }),
            ..item.router_data
        })
    }
}

fn get_error_attempt_status(error_code: &str, http_code: u16) -> AttemptStatus {
    match http_code {
        // 4xx Client errors
        400 => match error_code {
            // Card-related errors
            code if code.contains("CARD") || code.contains("card") => AttemptStatus::Failure,
            // Authentication errors
            code if code.contains("AUTH") || code.contains("auth") => {
                AttemptStatus::AuthenticationFailed
            }
            // Validation errors
            _ => AttemptStatus::Failure,
        },
        401 | 403 => AttemptStatus::AuthenticationFailed,
        404 => AttemptStatus::Failure,
        422 => AttemptStatus::Failure,
        429 => AttemptStatus::Pending,

        // 5xx Server errors
        500..=599 => AttemptStatus::Pending,

        // Default for other status codes
        _ => AttemptStatus::Failure,
    }
}

impl From<ArchipelErrorMessageWithHttpCode> for ErrorResponse {
    fn from(
        ArchipelErrorMessageWithHttpCode {
            error_message,
            http_code,
        }: ArchipelErrorMessageWithHttpCode,
    ) -> Self {
        let attempt_status = get_error_attempt_status(&error_message.code, http_code);
        let error_message_text = error_message
            .description
            .clone()
            .unwrap_or(consts::NO_ERROR_MESSAGE.to_string());

        Self {
            status_code: http_code,
            code: error_message.code.clone(),
            attempt_status: Some(attempt_status),
            connector_transaction_id: None,
            message: error_message_text.clone(),
            reason: error_message.description,
            network_decline_code: Some(error_message.code),
            network_advice_code: None,
            network_error_message: Some(error_message_text),
        }
    }
}

// REFUND FLOW
#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelRefundRequest {
    order: ArchipelRefundOrderRequest,
    tenant_id: ArchipelTenantId,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelRefundOrderRequest {
    amount: MinorUnit,
    currency: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ArchipelRefundStatus {
    Succeeded,
    Failed,
    Pending,
}

impl From<ArchipelRefundStatus> for common_enums::RefundStatus {
    fn from(status: ArchipelRefundStatus) -> Self {
        match status {
            ArchipelRefundStatus::Succeeded => Self::Success,
            ArchipelRefundStatus::Failed => Self::Failure,
            ArchipelRefundStatus::Pending => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelRefundResponse {
    transaction_id: String,
    transaction_date: String,
    transaction_result: ArchipelRefundStatus,
    error: Option<ArchipelErrorMessage>,
    order: ArchipelOrderResponse,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ArchipelRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for ArchipelRefundRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ArchipelRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let connector_metadata =
            ArchipelConfigData::try_from(&item.router_data.request.refund_connector_metadata)?;

        Ok(Self {
            order: ArchipelRefundOrderRequest {
                amount: item.router_data.request.minor_refund_amount,
                currency: item.router_data.request.currency.to_string(),
            },
            tenant_id: connector_metadata.tenant_id,
        })
    }
}

impl TryFrom<ResponseRouterData<ArchipelRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ArchipelRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(error) = item.response.error {
            return Ok(Self {
                response: Err(ArchipelErrorMessageWithHttpCode::new(error, item.http_code).into()),
                ..item.router_data
            });
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id,
                refund_status: common_enums::RefundStatus::from(item.response.transaction_result),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// RSYNC FLOW
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ArchipelRSyncResponse(ArchipelRefundResponse);

impl std::ops::Deref for ArchipelRSyncResponse {
    type Target = ArchipelRefundResponse;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F> TryFrom<ResponseRouterData<ArchipelRSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ArchipelRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(error) = item.response.0.error.clone() {
            return Ok(Self {
                response: Err(ArchipelErrorMessageWithHttpCode::new(error, item.http_code).into()),
                ..item.router_data
            });
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.0.transaction_id.clone(),
                refund_status: common_enums::RefundStatus::from(
                    item.response.0.transaction_result.clone(),
                ),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// SETUP MANDATE FLOW
#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct ArchipelSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(ArchipelCardAuthorizationRequest<T>);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ArchipelSetupMandateResponse(ArchipelPaymentsResponse);

impl std::ops::Deref for ArchipelSetupMandateResponse {
    type Target = ArchipelPaymentsResponse;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ArchipelRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ArchipelSetupMandateRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ArchipelRouterData<
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

        // For setup mandate, we use a minimal amount (1 in minor units)
        let amount = MinorUnit::new(1);

        let payment_method_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(ccard) => {
                let cardholder = router_data
                    .resource_common_data
                    .get_billing_address()
                    .ok()
                    .and_then(|address| address.to_archipel_billing_address())
                    .map(|billing_address| ArchipelCardHolder {
                        billing_address: Some(billing_address),
                    });

                let card_holder_name = cardholder.as_ref().and_then(|_| {
                    router_data
                        .resource_common_data
                        .get_billing()
                        .ok()
                        .and_then(|billing| billing.get_optional_full_name())
                });

                ArchipelCard::try_from((card_holder_name, cardholder, ccard))?
            }
            PaymentMethodData::CardDetailsForNetworkTransactionId(..)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(..)
            | PaymentMethodData::CardRedirect(..)
            | PaymentMethodData::Wallet(..)
            | PaymentMethodData::PayLater(..)
            | PaymentMethodData::BankRedirect(..)
            | PaymentMethodData::BankDebit(..)
            | PaymentMethodData::BankTransfer(..)
            | PaymentMethodData::Crypto(..)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(..)
            | PaymentMethodData::Upi(..)
            | PaymentMethodData::Voucher(..)
            | PaymentMethodData::GiftCard(..)
            | PaymentMethodData::CardToken(..)
            | PaymentMethodData::OpenBanking(..)
            | PaymentMethodData::NetworkToken(..)
            | PaymentMethodData::MobilePayment(..) => Err(ConnectorError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Archipel"),
            ))?,
        };

        let connector_metadata = ArchipelConfigData::try_from(&router_data.request.metadata)?;

        let cardholder = router_data
            .resource_common_data
            .get_billing_address()
            .ok()
            .and_then(|address| address.to_archipel_billing_address())
            .map(|billing_address| ArchipelCardHolder {
                billing_address: Some(billing_address),
            });

        let order = ArchipelOrderRequest {
            amount,
            currency: router_data.request.currency.to_string(),
            certainty: ArchipelPaymentCertainty::Final,
            initiator: ArchipelPaymentInitiator::Customer,
        };

        // For setup mandate, we always set stored_on_file to true and mark as initial credential indicator
        let credential_indicator = Some(ArchipelCredentialIndicator {
            status: ArchipelCredentialIndicatorStatus::Initial,
            recurring: Some(true),
            transaction_id: None,
        });

        // Extract 3DS authentication data if available
        // 3DS data comes from completed authentication flow and is stored in metadata
        let three_ds: Option<Archipel3DS> =
            router_data.request.metadata.as_ref().and_then(|metadata| {
                let exposed_metadata = metadata.clone().expose();
                // Extract individual 3DS fields from metadata
                let auth_data = exposed_metadata.get("authentication_data")?;

                // Extract CAVV (required field for 3DS)
                let cavv = auth_data
                    .get("cavv")
                    .and_then(|v: &Value| v.as_str())
                    .map(|s: &str| Secret::new(s.to_string()))?;

                // Extract optional fields
                let eci = auth_data
                    .get("eci")
                    .and_then(|v: &Value| v.as_str())
                    .map(String::from);
                let ds_trans_id = auth_data
                    .get("ds_trans_id")
                    .and_then(|v: &Value| v.as_str())
                    .map(String::from);
                let message_version = auth_data
                    .get("message_version")
                    .and_then(|v: &Value| v.as_str())
                    .and_then(|s: &str| s.parse::<common_utils::types::SemanticVersion>().ok());

                let now = date_time::date_as_yyyymmddthhmmssmmmz().ok();

                Some(Archipel3DS {
                    acs_trans_id: None,
                    ds_trans_id: ds_trans_id.map(Secret::new),
                    three_ds_requestor_name: None,
                    three_ds_auth_date: now,
                    three_ds_auth_amt: None,
                    three_ds_auth_status: None,
                    three_ds_max_supported_version: THREE_DS_MAX_SUPPORTED_VERSION.into(),
                    three_ds_version: message_version,
                    authentication_value: cavv,
                    authentication_method: None,
                    eci,
                })
            });

        Ok(Self(ArchipelCardAuthorizationRequest {
            order,
            card: payment_method_data,
            cardholder,
            three_ds,
            credential_indicator,
            stored_on_file: true,
            tenant_id: connector_metadata.tenant_id,
        }))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ArchipelSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ArchipelSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(error) = item.response.0.error {
            return Ok(Self {
                response: Err(ArchipelErrorMessageWithHttpCode::new(error, item.http_code).into()),
                ..item.router_data
            });
        };

        let connector_metadata: Option<Value> = ArchipelTransactionMetadata::from(&item.response.0)
            .encode_to_value()
            .ok();

        // Setup mandate always uses Authorize flow
        let status: AttemptStatus = ArchipelFlowStatus::new(
            item.response.0.transaction_result,
            ArchipelPaymentFlow::Authorize,
        )
        .into();

        // For mandate reference, we use the payment_account_reference which is the tokenized card reference
        let mandate_reference = item
            .response
            .0
            .payment_account_reference
            .clone()
            .map(|par| {
                Box::new(domain_types::connector_types::MandateReference {
                    connector_mandate_id: Some(par.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                })
            });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.0.order.id),
                status_code: item.http_code,
                redirection_data: None,
                mandate_reference,
                connector_metadata,
                network_txn_id: Some(item.response.0.transaction_id),
                connector_response_reference_id: None,
                incremental_authorization_allowed: Some(false),
            }),
            ..item.router_data
        })
    }
}
