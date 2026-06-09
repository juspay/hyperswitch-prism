use common_enums::AttemptStatus;
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// =============================================================================
// AUTH
// =============================================================================
#[derive(Debug, Clone)]
pub struct SquareAuthType {
    /// Square access token, used as the HTTP `Authorization: Bearer {token}` header.
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for SquareAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Square { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SquareErrorResponse {
    /// Square returns an array of error objects under `errors`.
    #[serde(default)]
    pub errors: Vec<SquareError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SquareError {
    /// High level category, e.g. `INVALID_REQUEST_ERROR`, `PAYMENT_METHOD_ERROR`.
    #[serde(default)]
    pub category: String,
    /// Specific error code, e.g. `CARD_DECLINED`, `IDEMPOTENCY_KEY_REUSED`.
    #[serde(default)]
    pub code: String,
    pub detail: Option<String>,
    pub field: Option<String>,
}

impl SquareErrorResponse {
    /// Returns the first error's code, or a generic fallback when none is present.
    pub fn first_code(&self) -> String {
        self.errors
            .first()
            .map(|e| e.code.clone())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "SQUARE_ERROR".to_string())
    }

    /// Returns the first error's human-readable detail (falling back to its code).
    pub fn first_message(&self) -> String {
        self.errors
            .first()
            .and_then(|e| e.detail.clone())
            .or_else(|| self.errors.first().map(|e| e.code.clone()))
            .unwrap_or_else(|| "Unknown error from Square".to_string())
    }
}

// =============================================================================
// CLIENT SDK SESSION TOKEN
// =============================================================================
// Square has NO backend REST endpoint for minting an SDK session. The Web
// Payments SDK is initialized client-side with `application_id` and
// `location_id` (`Square.payments(application_id, location_id)`), and the client
// tokenizes the card into a single-use `cnon:` token that is later submitted as
// `source_id` to the Authorize call. The structs below model the session
// configuration surfaced to the client SDK.

/// Configuration handed to the Square Web Payments SDK so it can initialize and
/// tokenize a payment instrument on the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SquareSdkSessionConfig {
    /// Square application id (`appId`) used by `Square.payments(appId, locationId)`.
    pub application_id: Secret<String>,
    /// Seller location id (`locationId`) used by `Square.payments(appId, locationId)`.
    pub location_id: Option<String>,
    /// Either `sandbox` or `production`, derived from the connector base URL.
    pub environment: SquareEnvironment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SquareEnvironment {
    Sandbox,
    Production,
}

impl SquareEnvironment {
    /// Derives the SDK environment from the connector base URL.
    pub fn from_base_url(base_url: &str) -> Self {
        if base_url.contains("squareupsandbox") {
            Self::Sandbox
        } else {
            Self::Production
        }
    }
}

// =============================================================================
// AUTHORIZE — request
// =============================================================================
#[derive(Debug, Serialize)]
pub struct SquareMoney {
    /// Amount in the smallest denomination of the currency (e.g. cents).
    pub amount: i64,
    /// ISO 4217 currency code.
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct SquarePaymentsRequest {
    /// Unique key per CreatePayment request (max 45 chars). Makes retries safe.
    pub idempotency_key: String,
    /// Source of funds — the single-use payment token (`cnon:`) from the SDK.
    pub source_id: Secret<String>,
    pub amount_money: SquareMoney,
    /// `false` = authorize only (manual capture); `true` = authorize + capture.
    pub autocomplete: bool,
    /// Merchant reference id (Hyperswitch connector_request_reference_id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    /// Seller location id, when configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::square::SquareRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for SquarePaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: crate::connectors::square::SquareRouterData<
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

        // Square requires a single-use payment token (`source_id`). In UCS the
        // SDK-produced `cnon:` token arrives as card data the client already
        // tokenized; we use the card number as the token source. Non-card
        // methods are unsupported (Square has no server-side tokenization).
        let source_id = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => Secret::new(card.card_number.peek().to_string()),
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "Square only supports card payments via the Web Payments SDK token"
                            .to_string(),
                        errors::IntegrationErrorContext::default(),
                    )
                ))
            }
        };

        // Manual capture => authorize only (autocomplete=false). Anything else
        // (including the default) => authorize + capture (autocomplete=true).
        let autocomplete = !matches!(
            router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Manual)
                | Some(common_enums::CaptureMethod::ManualMultiple)
        );

        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?
            .get_amount_as_i64();

        let reference_id = Some(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        Ok(Self {
            idempotency_key: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            source_id,
            amount_money: SquareMoney {
                amount,
                currency: router_data.request.currency.to_string(),
            },
            autocomplete,
            reference_id,
            location_id: None,
        })
    }
}

// =============================================================================
// AUTHORIZE — response
// =============================================================================
#[derive(Debug, Deserialize, Serialize)]
pub struct SquarePaymentsResponse {
    pub payment: Option<SquarePayment>,
    #[serde(default)]
    pub errors: Vec<SquareError>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SquarePayment {
    pub id: String,
    pub status: SquarePaymentStatus,
    pub card_details: Option<SquareCardDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SquareCardDetails {
    pub status: Option<SquareCardStatus>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SquarePaymentStatus {
    Approved,
    Completed,
    Pending,
    Canceled,
    Failed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SquareCardStatus {
    Authorized,
    Captured,
    Voided,
    Failed,
    #[serde(other)]
    Unknown,
}

impl From<SquarePaymentStatus> for AttemptStatus {
    fn from(status: SquarePaymentStatus) -> Self {
        match status {
            // APPROVED = authorized but not captured (manual capture path).
            SquarePaymentStatus::Approved => Self::Authorized,
            // COMPLETED = captured/settled.
            SquarePaymentStatus::Completed => Self::Charged,
            SquarePaymentStatus::Pending => Self::Pending,
            SquarePaymentStatus::Canceled => Self::Voided,
            SquarePaymentStatus::Failed => Self::Failure,
            SquarePaymentStatus::Unknown => Self::Pending,
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<SquarePaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SquarePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.payment {
            Some(payment) => {
                let status = AttemptStatus::from(payment.status);
                Ok(Self {
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(payment.id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
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
            None => {
                let error = SquareErrorResponse {
                    errors: item.response.errors,
                };
                Ok(Self {
                    response: Err(domain_types::router_data::ErrorResponse {
                        status_code: item.http_code,
                        code: error.first_code(),
                        message: error.first_message(),
                        reason: Some(error.first_message()),
                        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                            AttemptStatus::Failure,
                        )),
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}
