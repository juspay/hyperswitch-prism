use common_enums::Currency;
use common_utils::{ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PaymentMethodToken, RepeatPayment, ServerAuthenticationToken,
        ServerSessionAuthenticationToken, SetupMandate, Void,
    },
    connector_types::{
        EventType, MandateReference, MandateReferenceId, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundWebhookReference, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::hyperswitch::HyperswitchRouterData, types::ResponseRouterData,
    utils::get_unimplemented_payment_method_error_message,
};

// =============================================================================
// AUTH
// =============================================================================
pub struct HyperswitchAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for HyperswitchAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Hyperswitch { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// =============================================================================
// ENUMS
// =============================================================================
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperswitchCaptureMethod {
    Automatic,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperswitchAuthenticationType {
    ThreeDs,
    NoThreeDs,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperswitchPaymentMethod {
    Card,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperswitchIntentStatus {
    Succeeded,
    Failed,
    Cancelled,
    CancelledPostCapture,
    Processing,
    RequiresCustomerAction,
    RequiresMerchantAction,
    RequiresPaymentMethod,
    RequiresConfirmation,
    RequiresCapture,
    PartiallyCaptured,
    PartiallyCapturedAndCapturable,
    PartiallyAuthorizedAndRequiresCapture,
    PartiallyCapturedAndProcessing,
    Conflicted,
    Expired,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperswitchRefundStatus {
    Succeeded,
    Failed,
    Pending,
    Review,
}

// =============================================================================
// REQUEST: AUTHORIZE
// =============================================================================
#[derive(Debug, Serialize)]
pub struct HyperswitchCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    pub card_exp_month: Secret<String>,
    pub card_exp_year: Secret<String>,
    pub card_holder_name: Secret<String>,
    pub card_cvc: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct HyperswitchPaymentMethodData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card: HyperswitchCard<T>,
}

/// Raw-card Authorize body. Serialized verbatim (byte-identical to the
/// original card-only request) when chosen by the untagged enum below.
#[derive(Debug, Serialize)]
pub struct HyperswitchCardPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub confirm: bool,
    pub capture_method: HyperswitchCaptureMethod,
    pub authentication_type: HyperswitchAuthenticationType,
    pub payment_method: HyperswitchPaymentMethod,
    pub payment_method_data: HyperswitchPaymentMethodData<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
    // Hyperswitch is a payment orchestrator: the `/payments` create+confirm call
    // must identify which business profile routes the payment. The caller supplies
    // `profile_id` (or `business_country`/`business_label`) via the request metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_label: Option<String>,
}

// Token (recurring / MIT off-session) Authorize body. See tech spec section 7.
#[derive(Debug, Serialize)]
pub struct HyperswitchRecurringDetailsData {
    pub processor_payment_token: String,
    pub merchant_connector_id: String,
}

#[derive(Debug, Serialize)]
pub struct HyperswitchRecurringDetails {
    #[serde(rename = "type")]
    pub recurring_type: String,
    pub data: HyperswitchRecurringDetailsData,
}

#[derive(Debug, Serialize)]
pub struct HyperswitchTokenPaymentsRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub confirm: bool,
    pub off_session: bool,
    pub payment_method: common_enums::PaymentMethod,
    pub recurring_details: HyperswitchRecurringDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<serde_json::Value>,
}

/// Authorize request body. Untagged so the card variant serializes exactly as
/// the original card-only struct did (byte-identical), and the token variant
/// serializes to the `recurring_details` shape from tech spec section 7.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum HyperswitchPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(HyperswitchCardPaymentsRequest<T>),
    Token(HyperswitchTokenPaymentsRequest),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HyperswitchPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
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
        let capture_method = match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => HyperswitchCaptureMethod::Manual,
            Some(common_enums::CaptureMethod::Automatic) | None => {
                HyperswitchCaptureMethod::Automatic
            }
            Some(_) => {
                return Err(IntegrationError::CaptureMethodNotSupported {
                    context: Default::default(),
                }
                .into())
            }
        };

        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Hyperswitch routes payments through a business profile. Pull the
        // routing identifiers from the request metadata object if present:
        // `{ "profile_id": "pro_..." }` or
        // `{ "business_country": "US", "business_label": "default" }`.
        let metadata = router_data
            .request
            .metadata
            .as_ref()
            .and_then(|m| m.peek().as_object());
        let metadata_string = |key: &str| {
            metadata
                .and_then(|obj| obj.get(key))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };
        let profile_id = metadata_string("profile_id");
        let business_country = metadata_string("business_country");
        let business_label = metadata_string("business_label");

        // Token (recurring / MIT) source resolution. The processor payment token
        // comes either from a connector mandate id (if the Authorize request
        // carries one) or from a wire-supplied `PaymentMethodData::PaymentMethodToken`.
        // See tech spec section 7.
        let processor_payment_token = router_data.request.connector_mandate_id().or_else(|| {
            match &router_data.request.payment_method_data {
                PaymentMethodData::PaymentMethodToken(token) => {
                    Some(token.token.peek().to_string())
                }
                _ => None,
            }
        });

        if let Some(processor_payment_token) = processor_payment_token {
            // `merchant_connector_id` (required) and optional `connector` (for
            // single-connector routing) are read from the request metadata object.
            let merchant_connector_id = metadata_string("merchant_connector_id").ok_or(
                IntegrationError::NotImplemented(
                    "hyperswitch token authorize requires `merchant_connector_id` in metadata"
                        .to_string(),
                    Default::default(),
                ),
            )?;
            let routing = metadata_string("connector").map(|connector| {
                serde_json::json!({
                    "type": "single",
                    "data": {
                        "connector": connector,
                        "merchant_connector_id": merchant_connector_id,
                    }
                })
            });
            return Ok(Self::Token(HyperswitchTokenPaymentsRequest {
                amount,
                currency: router_data.request.currency,
                confirm: true,
                off_session: true,
                payment_method: router_data.resource_common_data.payment_method,
                recurring_details: HyperswitchRecurringDetails {
                    recurring_type: "processor_payment_token".to_string(),
                    data: HyperswitchRecurringDetailsData {
                        processor_payment_token,
                        merchant_connector_id,
                    },
                },
                routing,
            }));
        }

        match router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card_data) => Ok(Self::Card(HyperswitchCardPaymentsRequest {
                amount,
                currency: router_data.request.currency,
                confirm: true,
                capture_method,
                authentication_type: HyperswitchAuthenticationType::NoThreeDs,
                payment_method: HyperswitchPaymentMethod::Card,
                payment_method_data: HyperswitchPaymentMethodData {
                    card: HyperswitchCard {
                        card_number: card_data.card_number.clone(),
                        card_exp_month: card_data.card_exp_month.clone(),
                        card_exp_year: card_data.card_exp_year.clone(),
                        card_holder_name: card_data
                            .card_holder_name
                            .clone()
                            .unwrap_or_else(|| Secret::new(String::new())),
                        card_cvc: card_data.card_cvc.clone(),
                    },
                },
                description: router_data.resource_common_data.description.clone(),
                return_url: router_data.request.router_return_url.clone(),
                profile_id,
                business_country,
                business_label,
            })),
            _ => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message("hyperswitch"),
                Default::default(),
            )
            .into()),
        }
    }
}

// =============================================================================
// REQUEST: SETUP MANDATE  (POST /payments + setup_future_usage + customer_acceptance)
// =============================================================================
#[derive(Debug, Serialize)]
pub struct HyperswitchOnlineMandate {
    pub ip_address: String,
    pub user_agent: String,
}

#[derive(Debug, Serialize)]
pub struct HyperswitchCustomerAcceptance {
    pub acceptance_type: String,
    pub online: HyperswitchOnlineMandate,
}

#[derive(Debug, Serialize)]
pub struct HyperswitchSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub confirm: bool,
    pub capture_method: HyperswitchCaptureMethod,
    pub authentication_type: HyperswitchAuthenticationType,
    pub payment_method: HyperswitchPaymentMethod,
    pub payment_method_data: HyperswitchPaymentMethodData<T>,
    pub setup_future_usage: String,
    pub customer_acceptance: HyperswitchCustomerAcceptance,
    // Hyperswitch requires `customer_id` whenever `setup_future_usage` is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HyperswitchSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
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

        // Mandate setup may be zero-amount; fall back to 0 when no amount given.
        let amount = router_data
            .request
            .minor_amount
            .unwrap_or(MinorUnit::new(0));

        let metadata = router_data
            .request
            .metadata
            .as_ref()
            .and_then(|m| m.peek().as_object());
        let profile_id = metadata
            .and_then(|obj| obj.get("profile_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Hyperswitch mandates `customer_id` when `setup_future_usage` is set.
        // Prefer the request's customer_id, then the flow-level customer_id.
        let customer_id = router_data
            .request
            .customer_id
            .as_ref()
            .or(router_data.resource_common_data.customer_id.as_ref())
            .map(|id| id.get_string_repr().to_string());

        // Online customer acceptance (consent) is required to store the payment
        // method for future off-session use. See tech spec section 8.
        let online = router_data
            .request
            .customer_acceptance
            .as_ref()
            .and_then(|ca| ca.online.as_ref());
        let customer_acceptance = HyperswitchCustomerAcceptance {
            acceptance_type: "online".to_string(),
            online: HyperswitchOnlineMandate {
                ip_address: online
                    .and_then(|o| o.ip_address.as_ref())
                    .map(|ip| ip.peek().to_string())
                    .unwrap_or_else(|| "127.0.0.1".to_string()),
                user_agent: online
                    .map(|o| o.user_agent.clone())
                    .unwrap_or_else(|| "UCS".to_string()),
            },
        };

        match router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card_data) => Ok(Self {
                amount,
                currency: router_data.request.currency,
                confirm: true,
                capture_method: HyperswitchCaptureMethod::Automatic,
                authentication_type: HyperswitchAuthenticationType::NoThreeDs,
                payment_method: HyperswitchPaymentMethod::Card,
                payment_method_data: HyperswitchPaymentMethodData {
                    card: HyperswitchCard {
                        card_number: card_data.card_number.clone(),
                        card_exp_month: card_data.card_exp_month.clone(),
                        card_exp_year: card_data.card_exp_year.clone(),
                        card_holder_name: card_data
                            .card_holder_name
                            .clone()
                            .unwrap_or_else(|| Secret::new(String::new())),
                        card_cvc: card_data.card_cvc.clone(),
                    },
                },
                setup_future_usage: "off_session".to_string(),
                customer_acceptance,
                customer_id,
                profile_id,
            }),
            _ => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message("hyperswitch"),
                Default::default(),
            )
            .into()),
        }
    }
}

// =============================================================================
// REQUEST: REPEAT PAYMENT (MIT recurring charge)  (POST /payments + recurring_details)
// =============================================================================
// RepeatPayment charges off-session against a payment method vaulted by a prior
// SetupMandate. Hyperswitch references its own vaulted payment method by id, so
// the request uses `recurring_details { type: "payment_method_id", data: "pm_..." }`
// together with the owning `customer_id` — NOT the `processor_payment_token`
// object (that form is for raw processor tokens and leaves Hyperswitch with no
// card data, which the underlying gateway rejects). See tech spec
// "RepeatPayment — Merchant-Initiated Recurring Charge (MIT)".
#[derive(Debug, Serialize)]
pub struct HyperswitchPaymentMethodIdRecurringDetails {
    #[serde(rename = "type")]
    pub recurring_type: String,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct HyperswitchRepeatPaymentRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub confirm: bool,
    pub off_session: bool,
    pub customer_id: String,
    pub recurring_details: HyperswitchPaymentMethodIdRecurringDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HyperswitchRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
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

        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // The vaulted payment-method id comes from the mandate reference
        // established by a prior SetupMandate (the connector maps Hyperswitch's
        // `payment_method_id` into `connector_mandate_id`). Only a connector
        // mandate id is supported here.
        let payment_method_id = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate) => connector_mandate
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotImplemented(
                    "hyperswitch repeat payment only supports connector mandate id references"
                        .to_string(),
                    Default::default(),
                )
                .into())
            }
        };

        // Hyperswitch requires the owning `customer_id` to charge a vaulted
        // payment method off-session. Mirror SetupMandate's resolution.
        let customer_id = router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "customer_id",
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            confirm: true,
            off_session: true,
            customer_id,
            recurring_details: HyperswitchPaymentMethodIdRecurringDetails {
                recurring_type: "payment_method_id".to_string(),
                data: payment_method_id,
            },
        })
    }
}

// =============================================================================
// REQUEST: PAYMENT METHOD TOKEN (Tokenization)  (POST /payment_methods)
// =============================================================================
#[derive(Debug, Serialize)]
pub struct HyperswitchTokenizationRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub payment_method: HyperswitchPaymentMethod,
    pub card: HyperswitchCard<T>,
    // Hyperswitch's POST /payment_methods requires the customer the token is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for HyperswitchTokenizationRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let customer_id = item
            .router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string());
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card_data) => Ok(Self {
                payment_method: HyperswitchPaymentMethod::Card,
                card: HyperswitchCard {
                    card_number: card_data.card_number.clone(),
                    card_exp_month: card_data.card_exp_month.clone(),
                    card_exp_year: card_data.card_exp_year.clone(),
                    card_holder_name: card_data
                        .card_holder_name
                        .clone()
                        .unwrap_or_else(|| Secret::new(String::new())),
                    card_cvc: card_data.card_cvc.clone(),
                },
                customer_id,
            }),
            _ => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message("hyperswitch"),
                Default::default(),
            )
            .into()),
        }
    }
}

// =============================================================================
// REQUEST: SESSION (ServerSessionAuthenticationToken)  (POST /payments)
// =============================================================================
#[derive(Debug, Serialize)]
pub struct HyperswitchSessionRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for HyperswitchSessionRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.amount,
            currency: item.router_data.request.currency,
        })
    }
}

// =============================================================================
// REQUEST: CAPTURE
// =============================================================================
#[derive(Debug, Serialize)]
pub struct HyperswitchCaptureRequest {
    pub amount_to_capture: MinorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for HyperswitchCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount_to_capture = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self { amount_to_capture })
    }
}

// =============================================================================
// REQUEST: VOID
// =============================================================================
#[derive(Debug, Serialize)]
pub struct HyperswitchVoidRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_reason: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for HyperswitchVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            cancellation_reason: item.router_data.request.cancellation_reason.clone(),
        })
    }
}

// =============================================================================
// REQUEST: REFUND
// =============================================================================
#[derive(Debug, Serialize)]
pub struct HyperswitchRefundRequest {
    pub payment_id: String,
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for HyperswitchRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            payment_id: item.router_data.request.connector_transaction_id.clone(),
            amount,
            reason: item.router_data.request.reason.clone(),
        })
    }
}

// =============================================================================
// RESPONSE: PAYMENTS (Authorize / PSync / Capture / Void)
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchPaymentsResponse {
    pub payment_id: String,
    pub status: HyperswitchIntentStatus,
    #[serde(default)]
    pub connector_transaction_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    // Present on SetupMandate / mandate-creating responses. `#[serde(default)]`
    // keeps Authorize/PSync/Capture/Void parsing unchanged.
    #[serde(default)]
    pub payment_method_id: Option<String>,
    #[serde(default)]
    pub mandate_id: Option<String>,
}

// Distinct identifiers per flow so the macro generates unique `*Templating`
// marker types; all resolve to the same concrete response struct, so the
// `TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, _>>` impls apply.
pub type HyperswitchPSyncResponse = HyperswitchPaymentsResponse;
pub type HyperswitchCaptureResponse = HyperswitchPaymentsResponse;
pub type HyperswitchVoidResponse = HyperswitchPaymentsResponse;
pub type HyperswitchRepeatPaymentResponse = HyperswitchPaymentsResponse;

fn map_intent_status(
    status: &HyperswitchIntentStatus,
    _is_auto_capture: bool,
) -> common_enums::AttemptStatus {
    match status {
        // Hyperswitch returns `requires_capture` (not `succeeded`) for an
        // authorized-but-uncaptured manual-capture payment, so `succeeded`
        // always means a charge has completed.
        HyperswitchIntentStatus::Succeeded => common_enums::AttemptStatus::Charged,
        HyperswitchIntentStatus::Failed
        | HyperswitchIntentStatus::Conflicted
        | HyperswitchIntentStatus::Expired => common_enums::AttemptStatus::Failure,
        HyperswitchIntentStatus::Cancelled | HyperswitchIntentStatus::CancelledPostCapture => {
            common_enums::AttemptStatus::Voided
        }
        HyperswitchIntentStatus::Processing
        | HyperswitchIntentStatus::PartiallyCapturedAndProcessing
        | HyperswitchIntentStatus::RequiresMerchantAction => common_enums::AttemptStatus::Pending,
        HyperswitchIntentStatus::RequiresCapture
        | HyperswitchIntentStatus::PartiallyAuthorizedAndRequiresCapture => {
            common_enums::AttemptStatus::Authorized
        }
        HyperswitchIntentStatus::PartiallyCaptured => common_enums::AttemptStatus::PartialCharged,
        HyperswitchIntentStatus::PartiallyCapturedAndCapturable => {
            common_enums::AttemptStatus::PartialChargedAndChargeable
        }
        HyperswitchIntentStatus::RequiresPaymentMethod => {
            common_enums::AttemptStatus::PaymentMethodAwaited
        }
        HyperswitchIntentStatus::RequiresConfirmation => {
            common_enums::AttemptStatus::ConfirmationAwaited
        }
        HyperswitchIntentStatus::RequiresCustomerAction => {
            common_enums::AttemptStatus::AuthenticationPending
        }
        HyperswitchIntentStatus::Unknown => common_enums::AttemptStatus::Pending,
    }
}

fn build_payments_response(
    response: HyperswitchPaymentsResponse,
    http_code: u16,
    status: common_enums::AttemptStatus,
) -> Result<PaymentsResponseData, ErrorResponse> {
    if status == common_enums::AttemptStatus::Failure {
        Err(ErrorResponse {
            code: response
                .error_code
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
            message: response
                .error_message
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: response.error_message.clone(),
            attempt_status: None,
            connector_transaction_id: Some(response.payment_id.clone()),
            status_code: http_code,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    } else {
        Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: response.connector_transaction_id.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: Some(response.payment_id.clone()),
            incremental_authorization_allowed: None,
            status_code: http_code,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let is_auto_capture = !matches!(
            router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Manual)
        );
        let status = map_intent_status(&response.status, is_auto_capture);
        let payment_response = build_payments_response(response, http_code, status);
        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let is_auto_capture = router_data.request.is_auto_capture();
        let status = map_intent_status(&response.status, is_auto_capture);
        let payment_response = build_payments_response(response, http_code, status);
        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let is_auto_capture = !matches!(
            router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Manual)
        );
        let status = map_intent_status(&response.status, is_auto_capture);
        let payment_response = build_payments_response(response, http_code, status);
        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let status = map_intent_status(&response.status, true);
        let payment_response = build_payments_response(response, http_code, status);
        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let status = map_intent_status(&response.status, false);
        let payment_response = build_payments_response(response, http_code, status);
        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// =============================================================================
// RESPONSE: SETUP MANDATE
// =============================================================================
// Distinct alias so the macro generates a unique `*Templating` marker type.
pub type HyperswitchSetupMandateResponse = HyperswitchPaymentsResponse;

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let status = map_intent_status(&response.status, true);

        // Surface the reusable token as a mandate reference: prefer
        // `payment_method_id`, fall back to `mandate_id` (tech spec section 8).
        let mandate_id = response
            .payment_method_id
            .clone()
            .or_else(|| response.mandate_id.clone());
        let mandate_reference = mandate_id.map(|id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(id.clone()),
                payment_method_id: response.payment_method_id.clone(),
                connector_mandate_request_reference_id: None,
            })
        });

        let payment_response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: response
                    .error_code
                    .clone()
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                message: response
                    .error_message
                    .clone()
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                reason: response.error_message.clone(),
                attempt_status: None,
                connector_transaction_id: Some(response.payment_id.clone()),
                status_code: http_code,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: response.connector_transaction_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.payment_id.clone()),
                incremental_authorization_allowed: None,
                status_code: http_code,
            })
        };
        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// =============================================================================
// RESPONSE: PAYMENT METHOD TOKEN (Tokenization)
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchTokenResponse {
    pub payment_method_id: String,
}

impl<F, T> TryFrom<ResponseRouterData<HyperswitchTokenResponse, Self>>
    for RouterDataV2<
        F,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _,
        } = item;
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: response.payment_method_id,
            }),
            ..router_data
        })
    }
}

// =============================================================================
// RESPONSE: SESSION (ServerSessionAuthenticationToken)
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchSessionResponse {
    pub client_secret: String,
}

impl TryFrom<ResponseRouterData<HyperswitchSessionResponse, Self>>
    for RouterDataV2<
        ServerSessionAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchSessionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _,
        } = item;
        Ok(Self {
            response: Ok(ServerSessionAuthenticationTokenResponseData {
                session_token: response.client_secret,
            }),
            ..router_data
        })
    }
}

// =============================================================================
// ACCESS TOKEN (ServerAuthenticationToken) — DEGENERATE / non-functional
// =============================================================================
// Hyperswitch authenticates with a static `api-key` header; there is NO
// OAuth/token-mint endpoint (see tech spec section 11). This flow exists for
// surface parity only and is NOT for production token exchange. The macro
// requires an HTTP call, so we issue a benign POST to `/payments` (whose JSON
// body — success or error — is an object) and the permissive response struct
// below deserializes either; the configured `api-key` is then echoed back as
// the "access token".
#[derive(Debug, Serialize)]
pub struct HyperswitchAccessTokenRequest {
    pub grant_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for HyperswitchAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: item.router_data.request.grant_type.clone(),
        })
    }
}

// Permissive: deserializes from any JSON object (Hyperswitch success or error
// bodies) so the degenerate flow never fails on the wire body.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HyperswitchAccessTokenResponse {
    #[serde(default)]
    pub client_secret: Option<String>,
}

impl TryFrom<ResponseRouterData<HyperswitchAccessTokenResponse, Self>>
    for RouterDataV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: _,
            router_data,
            http_code: _,
        } = item;
        // Degenerate: echo the static api-key as the access token. Not a real
        // OAuth token — Hyperswitch uses static api-key auth (tech spec §11).
        let access_token = match &router_data.connector_config {
            ConnectorSpecificConfig::Hyperswitch { api_key, .. } => api_key.clone(),
            _ => Secret::new(String::new()),
        };
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token,
                token_type: None,
                expires_in: None,
            }),
            ..router_data
        })
    }
}

// =============================================================================
// RESPONSE: REFUND (Refund / RSync)
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchRefundResponse {
    pub refund_id: String,
    pub status: HyperswitchRefundStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

pub type HyperswitchRSyncResponse = HyperswitchRefundResponse;

impl From<HyperswitchRefundStatus> for common_enums::RefundStatus {
    fn from(item: HyperswitchRefundStatus) -> Self {
        match item {
            HyperswitchRefundStatus::Succeeded => Self::Success,
            HyperswitchRefundStatus::Failed => Self::Failure,
            HyperswitchRefundStatus::Pending | HyperswitchRefundStatus::Review => Self::Pending,
        }
    }
}

impl<F> TryFrom<ResponseRouterData<HyperswitchRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.refund_id,
                refund_status: common_enums::RefundStatus::from(response.status),
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<HyperswitchRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.refund_id,
                refund_status: common_enums::RefundStatus::from(response.status),
                status_code: http_code,
            }),
            ..router_data
        })
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchErrorResponse {
    pub error: HyperswitchErrorDetail,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchErrorDetail {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub message: Option<String>,
    pub code: Option<String>,
}

// =============================================================================
// INCOMING WEBHOOKS
// =============================================================================
// Hyperswitch (the orchestrator) sends outgoing webhooks shaped as:
//   { "merchant_id": "...", "event_id": "...", "event_type": "payment_succeeded",
//     "content": { "type": "payment_details", "object": { ...PaymentsResponse... } } }
// `content` is adjacently tagged on `type` + `object`. We model `object` as a raw
// JSON value and parse it lazily per content type, so unknown content/event types
// never fail deserialization of the envelope.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchWebhookBody {
    #[serde(default)]
    pub merchant_id: Option<String>,
    #[serde(default)]
    pub event_id: Option<String>,
    pub event_type: String,
    pub content: HyperswitchWebhookContent,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchWebhookContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub object: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchWebhookPayment {
    pub payment_id: String,
    pub status: HyperswitchIntentStatus,
    #[serde(default)]
    pub connector_transaction_id: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchWebhookRefund {
    pub refund_id: String,
    #[serde(default)]
    pub payment_id: Option<String>,
    #[serde(default)]
    pub connector_refund_id: Option<String>,
    pub status: HyperswitchRefundStatus,
}

const CONTENT_TYPE_PAYMENT: &str = "payment_details";
const CONTENT_TYPE_REFUND: &str = "refund_details";

/// Parse the webhook envelope from the raw body.
pub fn get_webhook_object_from_body(
    body: &[u8],
) -> Result<HyperswitchWebhookBody, error_stack::Report<WebhookError>> {
    body.parse_struct("HyperswitchWebhookBody")
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

/// Map a Hyperswitch `event_type` string to the UCS `EventType`.
/// Payments + refunds are mapped; everything else is left Unspecified rather than
/// force-mapped to a semantically different variant.
pub fn map_webhook_event_type(event_type: &str) -> EventType {
    match event_type {
        "payment_succeeded" => EventType::PaymentIntentSuccess,
        "payment_failed" => EventType::PaymentIntentFailure,
        "payment_processing" => EventType::PaymentIntentProcessing,
        "payment_cancelled" => EventType::PaymentIntentCancelled,
        "payment_authorized" => EventType::PaymentIntentAuthorizationSuccess,
        "payment_captured" => EventType::PaymentIntentCaptureSuccess,
        "payment_expired" => EventType::PaymentIntentExpired,
        "action_required" => EventType::PaymentActionRequired,
        "refund_succeeded" => EventType::RefundSuccess,
        "refund_failed" => EventType::RefundFailure,
        _ => EventType::IncomingWebhookEventUnspecified,
    }
}

fn parse_webhook_payment(
    content: &HyperswitchWebhookContent,
) -> Result<HyperswitchWebhookPayment, error_stack::Report<WebhookError>> {
    if content.content_type != CONTENT_TYPE_PAYMENT {
        return Err(report!(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("hyperswitch webhook content is not payment_details"));
    }
    serde_json::from_value(content.object.clone())
        .change_context(WebhookError::WebhookResourceObjectNotFound)
        .attach_printable("failed to parse hyperswitch payment webhook object")
}

fn parse_webhook_refund(
    content: &HyperswitchWebhookContent,
) -> Result<HyperswitchWebhookRefund, error_stack::Report<WebhookError>> {
    if content.content_type != CONTENT_TYPE_REFUND {
        return Err(report!(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("hyperswitch webhook content is not refund_details"));
    }
    serde_json::from_value(content.object.clone())
        .change_context(WebhookError::WebhookResourceObjectNotFound)
        .attach_printable("failed to parse hyperswitch refund webhook object")
}

/// Build the typed resource reference (payment / refund) for the ParseEvent phase.
pub fn get_webhook_reference(
    body: &HyperswitchWebhookBody,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
    match body.content.content_type.as_str() {
        CONTENT_TYPE_PAYMENT => {
            let payment = parse_webhook_payment(&body.content)?;
            Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: payment.connector_transaction_id,
                    merchant_transaction_id: Some(payment.payment_id),
                },
            )))
        }
        CONTENT_TYPE_REFUND => {
            let refund = parse_webhook_refund(&body.content)?;
            Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: refund.connector_refund_id,
                    merchant_refund_id: Some(refund.refund_id),
                    // The refund object echoes the original Hyperswitch payment_id.
                    connector_transaction_id: refund.payment_id,
                },
            )))
        }
        _ => Ok(None),
    }
}

/// Build the payment webhook response (HandleEvent → process_payment_webhook).
pub fn build_webhook_payment_response(
    body: &HyperswitchWebhookBody,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let payment = parse_webhook_payment(&body.content)?;
    // Webhooks are out-of-band terminal/async notifications; treat as auto-capture
    // for status mapping (succeeded -> Charged, requires_capture -> Authorized).
    let status = map_intent_status(&payment.status, true);
    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(
            payment.payment_id.clone(),
        )),
        status,
        connector_response_reference_id: Some(payment.payment_id),
        mandate_reference: None,
        error_code: payment.error_code,
        error_message: payment.error_message.clone(),
        error_reason: payment.error_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: None,
        minor_amount_captured: None,
        network_txn_id: payment.connector_transaction_id,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    })
}

/// Build the refund webhook response (HandleEvent → process_refund_webhook).
pub fn build_webhook_refund_response(
    body: &HyperswitchWebhookBody,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let refund = parse_webhook_refund(&body.content)?;
    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(refund.connector_refund_id.unwrap_or(refund.refund_id)),
        status: common_enums::RefundStatus::from(refund.status),
        connector_response_reference_id: None,
        error_code: None,
        error_message: None,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}
