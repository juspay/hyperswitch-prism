use common_enums::{self, AttemptStatus, CaptureMethod, Currency};
use common_utils::{consts, ext_traits::Encode, types::MinorUnit, CustomResult};
use domain_types::{
    connector_flow::{Authorize, IncrementalAuthorization},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsIncrementalAuthorizationData,
        PaymentsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils::CardIssuer,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::ResponseRouterData;

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
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Archipel { api_key, .. } => {
                let raw = api_key.peek().trim().to_string();
                // Accept either a raw PEM or a PEM whose newlines were escaped
                // as literal `\n` during transport (common when the cert is
                // passed through a gRPC metadata header that disallows newlines).
                let normalized = if raw.contains("\\n") && !raw.contains('\n') {
                    raw.replace("\\n", "\n")
                } else {
                    raw
                };
                let ca_certificate = if normalized.starts_with("-----BEGIN") {
                    Some(Secret::new(normalized))
                } else {
                    None
                };
                Ok(Self { ca_certificate })
            }
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ArchipelConfigData {
    pub tenant_id: ArchipelTenantId,
    pub platform_url: String,
}

impl TryFrom<&Option<Value>> for ArchipelConfigData {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(connector_metadata: &Option<Value>) -> Result<Self, Self::Error> {
        to_connector_meta(connector_metadata.clone())
    }
}

fn to_connector_meta(
    connector_meta: Option<Value>,
) -> CustomResult<ArchipelConfigData, IntegrationError> {
    let meta_obj = connector_meta.ok_or_else(|| IntegrationError::NoConnectorMetaData {
        context: Default::default(),
    })?;

    // Case A: Direct struct format
    if let Ok(cfg) = serde_json::from_value::<ArchipelConfigData>(meta_obj.clone()) {
        return Ok(cfg);
    }

    // Case B: Stringified JSON directly or via nested connector_meta_data key
    let meta_str = if let Some(s) = meta_obj.as_str() {
        s.to_string()
    } else {
        meta_obj
            .get("connector_meta_data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IntegrationError::InvalidDataFormat {
                field_name: "connector_meta_data",
                context: Default::default(),
            })?
            .to_string()
    };

    serde_json::from_str::<ArchipelConfigData>(&meta_str).change_context(
        IntegrationError::InvalidDataFormat {
            field_name: "ArchipelConfigData",
            context: Default::default(),
        },
    )
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
}

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
    TryFrom<&Card<T>> for ArchipelCard<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(ccard: &Card<T>) -> Result<Self, Self::Error> {
        let raw_card = serde_json::to_string(&ccard.card_number.0)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
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
            card_holder_name: ccard.card_holder_name.clone(),
            scheme,
        })
    }
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelCardAuthorizationRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    order: ArchipelOrderRequest,
    card: ArchipelCard<T>,
    stored_on_file: bool,
    tenant_id: ArchipelTenantId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ArchipelPaymentStatus {
    #[default]
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ArchipelPaymentFlow {
    Authorize,
    Pay,
}

pub struct ArchipelFlowStatus {
    status: ArchipelPaymentStatus,
    flow: ArchipelPaymentFlow,
}

impl ArchipelFlowStatus {
    pub fn new(status: ArchipelPaymentStatus, flow: ArchipelPaymentFlow) -> Self {
        Self { status, flow }
    }
}

impl From<ArchipelFlowStatus> for AttemptStatus {
    fn from(ArchipelFlowStatus { status, flow }: ArchipelFlowStatus) -> Self {
        match status {
            ArchipelPaymentStatus::Succeeded => match flow {
                ArchipelPaymentFlow::Authorize => Self::Authorized,
                ArchipelPaymentFlow::Pay => Self::Charged,
            },
            ArchipelPaymentStatus::Failed => match flow {
                ArchipelPaymentFlow::Authorize | ArchipelPaymentFlow::Pay => {
                    Self::AuthorizationFailed
                }
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelOrderResponse {
    pub id: String,
    pub amount: Option<MinorUnit>,
    pub currency: Option<Currency>,
    pub captured_amount: Option<MinorUnit>,
    pub authorized_amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchipelErrorResponse {
    pub code: String,
    pub description: Option<String>,
}

impl Default for ArchipelErrorResponse {
    fn default() -> Self {
        Self {
            code: consts::NO_ERROR_CODE.to_string(),
            description: Some(consts::NO_ERROR_MESSAGE.to_string()),
        }
    }
}

impl ArchipelErrorResponse {
    pub fn default_with_code(_http_code: u16) -> Self {
        Self::default()
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelTransactionMetadata {
    pub transaction_id: String,
    pub transaction_date: String,
    pub financial_network_code: Option<String>,
    pub issuer_transaction_id: Option<String>,
    pub response_code: Option<String>,
    pub authorization_code: Option<String>,
    pub payment_account_reference: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelPaymentsResponse {
    pub order: ArchipelOrderResponse,
    pub transaction_id: String,
    pub transaction_date: String,
    pub transaction_result: ArchipelPaymentStatus,
    pub error: Option<ArchipelErrorResponse>,
    pub financial_network_code: Option<String>,
    pub issuer_transaction_id: Option<String>,
    pub response_code: Option<String>,
    pub authorization_code: Option<String>,
    pub payment_account_reference: Option<Secret<String>>,
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

// AUTHORIZATION FLOW - Request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::ArchipelRouterData<
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
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        input: super::ArchipelRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = input.router_data;
        let request_incremental_authorization = router_data
            .request
            .request_incremental_authorization
            .unwrap_or(false);
        let certainty = if request_incremental_authorization {
            ArchipelPaymentCertainty::Estimated
        } else {
            ArchipelPaymentCertainty::Final
        };

        let order = ArchipelOrderRequest {
            amount: router_data.request.amount,
            currency: router_data.request.currency.to_string(),
            certainty,
            initiator: ArchipelPaymentInitiator::Customer,
        };

        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(ccard) => ArchipelCard::try_from(ccard)?,
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Only Card payments are supported by Archipel for this flow".to_string(),
                    Default::default(),
                )
                .into());
            }
        };

        // Resolve tenant_id from the per-request metadata.
        let metadata_value = router_data
            .request
            .connector_feature_data
            .as_ref()
            .map(|s| s.clone().peek().clone());
        let cfg = ArchipelConfigData::try_from(&metadata_value)?;

        Ok(Self {
            order,
            card,
            stored_on_file: false,
            tenant_id: cfg.tenant_id,
        })
    }
}

// AUTHORIZATION FLOW - Response
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ArchipelPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ArchipelPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(err) = item.response.error.clone() {
            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.code,
                    message: err.description.clone().unwrap_or_default(),
                    reason: err.description,
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            });
        }

        let capture_method = item
            .router_data
            .request
            .capture_method
            .unwrap_or(CaptureMethod::Automatic);

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
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: Some(is_incremental_allowed),
            }),
            ..item.router_data
        })
    }
}

// INCREMENTAL AUTHORIZATION FLOW
#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelIncrementalAuthRequest {
    pub order: ArchipelIncrementalAuthOrder,
    pub tenant_id: ArchipelTenantId,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelIncrementalAuthOrder {
    pub amount: MinorUnit,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchipelIncrementalAuthResponse {
    pub order: ArchipelOrderResponse,
    pub transaction_id: String,
    pub transaction_date: String,
    pub transaction_result: ArchipelPaymentStatus,
    pub error: Option<ArchipelErrorResponse>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::ArchipelRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ArchipelIncrementalAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        input: super::ArchipelRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = input.router_data;
        let metadata_value = router_data
            .request
            .connector_feature_data
            .as_ref()
            .map(|s| s.clone().peek().clone());
        let cfg = ArchipelConfigData::try_from(&metadata_value)?;
        Ok(Self {
            order: ArchipelIncrementalAuthOrder {
                amount: router_data.request.minor_amount,
                currency: router_data.request.currency.to_string(),
            },
            tenant_id: cfg.tenant_id,
        })
    }
}

impl TryFrom<ResponseRouterData<ArchipelIncrementalAuthResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ArchipelIncrementalAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(err) = item.response.error.clone() {
            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.code,
                    message: err.description.clone().unwrap_or_default(),
                    reason: err.description,
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            });
        }

        let authorization_status = match item.response.transaction_result {
            ArchipelPaymentStatus::Succeeded => common_enums::AuthorizationStatus::Success,
            ArchipelPaymentStatus::Failed => common_enums::AuthorizationStatus::Failure,
        };

        let attempt_status = match item.response.transaction_result {
            ArchipelPaymentStatus::Succeeded => AttemptStatus::Authorized,
            ArchipelPaymentStatus::Failed => AttemptStatus::AuthorizationFailed,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                status: authorization_status,
                connector_authorization_id: Some(item.response.transaction_id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
