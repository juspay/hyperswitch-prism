use common_enums::AttemptStatus;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    request::Method,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        EventType, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors,
    payment_address::Address,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::betterpayment::BetterpaymentRouterData, types::ResponseRouterData};

pub const PAYMENT_DOC_URL: &str =
    "https://docs.betterpayment.de/docs/api-reference/api-reference-payment-api";
pub const REFUND_DOC_URL: &str =
    "https://docs.betterpayment.de/docs/api-reference/api-reference-refund-api";

pub fn integration_error_context(
    doc_url: &str,
    suggested_action: &str,
    additional_context: &str,
) -> errors::IntegrationErrorContext {
    errors::IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_owned()),
        doc_url: Some(doc_url.to_owned()),
        additional_context: Some(additional_context.to_owned()),
    }
}

#[derive(Debug, Clone)]
pub struct BetterpaymentAuthType {
    /// API key — used as the HTTP Basic Auth username.
    pub api_key: Secret<String>,
    /// API password — used as the HTTP Basic Auth password.
    pub api_password: Secret<String>,
    /// Outgoing key — used only for webhook signature verification.
    pub webhook_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for BetterpaymentAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Betterpayment {
                api_key,
                api_password,
                webhook_key,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_password: api_password.to_owned(),
                webhook_key: webhook_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: integration_error_context(
                        PAYMENT_DOC_URL,
                        "Configure Betterpayment api_key, api_password (API password), and webhook_key (webhook key).",
                        "Betterpayment requires its connector-specific credentials and uses HTTP Basic authentication.",
                    )
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetterpaymentErrorResponse {
    pub error_code: Option<u64>,
    pub message: Option<String>,
}

/// Betterpayment's `payment_type` wire values.
/// To add a new payment method: add a variant here and a matching arm in the
/// `TryFrom` authorize block.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BetterpaymentPaymentType {
    Wero,
}

#[derive(Debug, Serialize)]
pub struct BetterpaymentAuthorizeRequest {
    pub payment_type: BetterpaymentPaymentType,
    pub order_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wero_checkout_url: Option<String>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub billing: Option<BetterpaymentBillingAddress>,
}

#[derive(Debug, Serialize)]
pub struct BetterpaymentBillingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    pub address: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<Secret<String>>,
    pub city: Secret<String>,
    pub postal_code: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    pub country: common_enums::CountryAlpha2,
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
}

fn missing_billing_field(field_name: &'static str) -> errors::IntegrationError {
    errors::IntegrationError::MissingRequiredField {
        field_name,
        context: integration_error_context(
            PAYMENT_DOC_URL,
            "Provide all required billing fields when supplying a billing address, or omit billing for Wero.",
            "Betterpayment requires address, city, postal_code, country, first_name, and last_name when billing is supplied.",
        ),
    }
}

impl TryFrom<&Address> for BetterpaymentBillingAddress {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(billing: &Address) -> Result<Self, Self::Error> {
        let address = billing
            .address
            .as_ref()
            .ok_or_else(|| missing_billing_field("billing.address"))?;

        Ok(Self {
            email: billing.email.clone(),
            address: address
                .get_line1()
                .change_context(missing_billing_field("billing.address.line1"))?
                .clone(),
            address2: address.line2.clone(),
            city: address
                .get_city()
                .change_context(missing_billing_field("billing.address.city"))?
                .clone(),
            postal_code: address
                .get_zip()
                .change_context(missing_billing_field("billing.address.zip"))?
                .clone(),
            state: address.state.clone(),
            country: *address
                .get_country()
                .change_context(missing_billing_field("billing.address.country"))?,
            first_name: address
                .get_first_name()
                .change_context(missing_billing_field("billing.address.first_name"))?
                .clone(),
            last_name: address
                .get_last_name()
                .change_context(missing_billing_field("billing.address.last_name"))?
                .clone(),
            phone: billing
                .phone
                .as_ref()
                .and_then(|phone| phone.get_e123_phone_number()),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BetterpaymentRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BetterpaymentAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: BetterpaymentRouterData<
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
        let request = &router_data.request;

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            request.amount.amount,
            request.amount.currency,
        )
        .change_context(errors::IntegrationError::AmountConversionFailed {
            context: integration_error_context(
                PAYMENT_DOC_URL,
                "Provide a valid payment amount in minor units and its currency.",
                "Betterpayment expects the payment amount in major units.",
            ),
        })?;

        // A compact UUIDv7 fits Betterpayment's 35-character order_id limit.
        let order_id = common_utils::fp_utils::generate_uuid_v7().replace('-', "");

        match &request.payment_method_data {
            PaymentMethodData::Wallet(WalletData::Wero(_)) => {
                if !matches!(
                    request.capture_method,
                    None | Some(common_enums::CaptureMethod::Automatic)
                ) {
                    return Err(errors::IntegrationError::CaptureMethodNotSupported {
                        context: integration_error_context(
                            PAYMENT_DOC_URL,
                            "Use automatic capture for Betterpayment Wero payments.",
                            "The Payment API authorizes and captures in a single step.",
                        ),
                    }
                    .into());
                }
                let billing_details = router_data.resource_common_data.get_optional_billing();
                let billing = billing_details
                    .filter(|billing| billing.address.is_some())
                    .map(BetterpaymentBillingAddress::try_from)
                    .transpose()?;
                let postback_url = request.webhook_url.clone().ok_or_else(|| {
                    errors::IntegrationError::MissingRequiredField {
                        field_name: "webhook_url",
                        context: integration_error_context(
                            PAYMENT_DOC_URL,
                            "Provide an HTTPS webhook URL for Betterpayment postbacks.",
                            "Required when creating a Betterpayment Wero payment.",
                        ),
                    }
                })?;
                let return_url = request
                    .router_return_url
                    .clone()
                    .ok_or_else(|| errors::IntegrationError::MissingRequiredField {
                        field_name: "router_return_url",
                        context: integration_error_context(
                            PAYMENT_DOC_URL,
                            "Provide a return URL for the Wero success, error, and checkout redirects.",
                            "Required when creating a Betterpayment Wero payment.",
                        ),
                    })?;

                Ok(Self {
                    payment_type: BetterpaymentPaymentType::Wero,
                    order_id,
                    amount,
                    currency: request.amount.currency,
                    merchant_reference: request.merchant_order_id.clone(),
                    postback_url: Some(postback_url),
                    success_url: Some(return_url.clone()),
                    error_url: Some(return_url.clone()),
                    wero_checkout_url: Some(return_url),
                    billing,
                })
            }
            _ => Err(error_stack::report!(
                errors::IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Betterpayment"),
                    integration_error_context(
                        PAYMENT_DOC_URL,
                        "Use the Wero wallet payment method with this connector.",
                        "This Betterpayment integration currently supports Wero only.",
                    ),
                )
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BetterpaymentPaymentStatus {
    Started,
    Pending,
    Completed,
    Declined,
    Canceled,
    Error,
    #[default]
    #[serde(other)]
    Unknown,
}

impl From<BetterpaymentPaymentStatus> for AttemptStatus {
    fn from(status: BetterpaymentPaymentStatus) -> Self {
        match status {
            BetterpaymentPaymentStatus::Started => Self::AuthenticationPending,
            BetterpaymentPaymentStatus::Pending => Self::Pending,
            BetterpaymentPaymentStatus::Completed => Self::Charged,
            BetterpaymentPaymentStatus::Declined => Self::Failure,
            BetterpaymentPaymentStatus::Canceled => Self::Voided,
            BetterpaymentPaymentStatus::Error => Self::Failure,
            BetterpaymentPaymentStatus::Unknown => Self::Pending,
        }
    }
}

/// Typed wire value for the `client_action` field.
/// Extend with new variants as Betterpayment adds more response flows
/// (e.g. `QrCode`, `PresentToShopper`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BetterpaymentClientAction {
    Redirect,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BetterpaymentActionData {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BetterpaymentAuthorizeResponse {
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub status: BetterpaymentPaymentStatus,
    pub order_id: Option<String>,
    pub error_code: Option<i64>,
    pub message: Option<String>,
    pub error_message: Option<String>,
    pub client_action: Option<BetterpaymentClientAction>,
    pub action_data: Option<BetterpaymentActionData>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<BetterpaymentAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BetterpaymentAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: connector_response,
            router_data,
            http_code,
        } = item;
        let status = AttemptStatus::from(connector_response.status);

        // IN-BAND 2xx FAILURE: Betterpayment returns HTTP 200 with `declined`
        // or `error` status. Turn those into Err(ErrorResponse) here — the
        // shared `build_error_response` never sees 2xx bodies.
        let response = if utils::is_payment_failure(status) {
            Err(ErrorResponse {
                code: connector_response
                    .error_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: connector_response
                    .message
                    .clone()
                    .or_else(|| connector_response.error_message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: connector_response
                    .message
                    .or(connector_response.error_message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: connector_response.transaction_id,
                ..Default::default()
            })
        } else {
            let redirection_data = match connector_response.client_action {
                Some(BetterpaymentClientAction::Redirect) => connector_response
                    .action_data
                    .and_then(|ad| ad.url)
                    .map(|url| {
                        Box::new(RedirectForm::Form {
                            endpoint: url,
                            method: Method::Get,
                            form_fields: Default::default(),
                        })
                    }),
                None | Some(BetterpaymentClientAction::Unknown) => None,
            };
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    connector_response
                        .transaction_id
                        .ok_or_else(utils::missing_field_err("transaction_id"))
                        .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                            context: errors::ResponseTransformationErrorContext {
                                http_status_code: Some(http_code),
                                additional_context: Some(format!(
                                    "Betterpayment payment response is missing transaction_id. See {PAYMENT_DOC_URL}#payment-api-response"
                                )),
                            },
                        })?,
                ),
                redirection_data,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: connector_response.order_id,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BetterpaymentPSyncResponse {
    pub transaction_id: Option<String>,
    pub status: BetterpaymentPaymentStatus,
    pub order_id: Option<String>,
    pub amount: Option<f64>,
    pub error_code: Option<u64>,
    pub message: Option<String>,
}

impl TryFrom<ResponseRouterData<BetterpaymentPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BetterpaymentPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: connector_response,
            router_data,
            http_code,
        } = item;
        let status = AttemptStatus::from(connector_response.status);

        let response = if utils::is_payment_failure(status) {
            Err(ErrorResponse {
                code: connector_response
                    .error_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: connector_response
                    .message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: connector_response.message,
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: connector_response.transaction_id,
                ..Default::default()
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    connector_response
                        .transaction_id
                        .ok_or_else(utils::missing_field_err("transaction_id"))
                        .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                            context: errors::ResponseTransformationErrorContext {
                                http_status_code: Some(http_code),
                                additional_context: Some(format!(
                                    "Betterpayment payment response is missing transaction_id. See {PAYMENT_DOC_URL}#payment-api-response"
                                )),
                            },
                        })?,
                ),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: connector_response.order_id,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response,
            ..router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BetterpaymentRefundStatus {
    Started,
    Successful,
    Error,
    #[serde(other)]
    Unknown,
}

impl From<BetterpaymentRefundStatus> for common_enums::RefundStatus {
    fn from(status: BetterpaymentRefundStatus) -> Self {
        match status {
            BetterpaymentRefundStatus::Started => Self::Pending,
            BetterpaymentRefundStatus::Successful => Self::Success,
            BetterpaymentRefundStatus::Error => Self::Failure,
            BetterpaymentRefundStatus::Unknown => Self::Pending,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BetterpaymentRefundRequest {
    pub transaction_id: String,
    pub amount: StringMajorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BetterpaymentRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for BetterpaymentRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: BetterpaymentRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.refund_amount.amount,
            item.router_data.request.refund_amount.currency,
        )
        .change_context(errors::IntegrationError::AmountConversionFailed {
            context: integration_error_context(
                REFUND_DOC_URL,
                "Provide a valid refund amount in minor units and its currency.",
                "Betterpayment expects the refund amount in major units.",
            ),
        })?;

        Ok(Self {
            transaction_id: item.router_data.request.connector_transaction_id.clone(),
            amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BetterpaymentRefundResponse {
    pub transaction_id: Option<String>,
    pub refund_id: Option<String>,
    pub status: BetterpaymentRefundStatus,
    pub status_code: Option<i64>,
    pub error_code: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct BetterpaymentRSyncResponse(pub Vec<BetterpaymentRefundResponse>);

impl BetterpaymentRSyncResponse {
    fn into_refund(
        self,
        refund_id: &str,
        http_code: u16,
    ) -> Result<BetterpaymentRefundResponse, error_stack::Report<errors::ConnectorError>> {
        self.0
            .into_iter()
            .find(|refund| refund.refund_id.as_deref() == Some(refund_id) && !refund_id.is_empty())
            .ok_or_else(|| {
                error_stack::report!(errors::ConnectorError::UnexpectedResponseError {
                    context: errors::ResponseTransformationErrorContext {
                        http_status_code: Some(http_code),
                        additional_context: Some(format!(
                            "Requested refund was absent from Betterpayment's refund list. Verify the connector refund ID. See {REFUND_DOC_URL}"
                        )),
                    },
                })
                .attach_printable("Requested refund was absent from Betterpayment's refund list")
            })
    }
}

impl TryFrom<ResponseRouterData<BetterpaymentRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BetterpaymentRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let refund = response.into_refund(&router_data.request.connector_refund_id, http_code)?;
        Self::try_from(ResponseRouterData {
            response: refund,
            router_data,
            http_code,
        })
    }
}

impl<F, Req> TryFrom<ResponseRouterData<BetterpaymentRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, Req, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BetterpaymentRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: refund,
            router_data,
            http_code,
        } = item;
        let refund_status = common_enums::RefundStatus::from(refund.status);

        let response = if matches!(refund_status, common_enums::RefundStatus::Failure) {
            Err(ErrorResponse {
                code: refund
                    .error_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: refund
                    .message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: refund.message,
                status_code: http_code,
                attempt_status: Some(FlowStatus::Refund(refund_status)),
                connector_transaction_id: refund.transaction_id,
                ..Default::default()
            })
        } else {
            let connector_refund_id =
                refund
                    .refund_id
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(errors::ConnectorError::UnexpectedResponseError {
                            context: errors::ResponseTransformationErrorContext {
                                http_status_code: Some(http_code),
                                additional_context: Some(format!(
                                    "Betterpayment refund response is missing a non-empty refund_id. See {REFUND_DOC_URL}#refund-api-response"
                                )),
                            },
                        })
                        .attach_printable("Missing Betterpayment refund_id")
                    })?;

            Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: http_code,
                acquirer_reference_number: None,
            })
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            response,
            ..router_data
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BetterpaymentWebhookBody {
    pub transaction_id: String,
    pub payment_type: Option<String>,
    pub status_code: Option<i64>,
    pub status: BetterpaymentWebhookStatus,
    pub order_id: String,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub message: Option<String>,
    pub reason_code: Option<String>,
    pub additional_transaction_data: Option<String>,
    pub remittance_info: Option<String>,
    pub wallet_activity_reference: Option<String>,
    pub wallet_account_reference: Option<String>,
    pub payer_location: Option<String>,
    // Refund-only fields; absent on payment postbacks.
    pub refund_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_refund_webhook_status")]
    pub refund_status_code: Option<BetterpaymentRefundWebhookStatus>,
    pub refund_reason_code: Option<String>,
    pub refund_wallet_activity_reference: Option<String>,
    pub refund_remittance_info: Option<String>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetterpaymentWebhookStatus {
    Started,
    Pending,
    Completed,
    Declined,
    Canceled,
    Error,
    #[serde(other)]
    Unknown,
}

impl From<BetterpaymentWebhookStatus> for EventType {
    fn from(status: BetterpaymentWebhookStatus) -> Self {
        match status {
            BetterpaymentWebhookStatus::Completed => Self::PaymentIntentSuccess,
            BetterpaymentWebhookStatus::Declined | BetterpaymentWebhookStatus::Error => {
                Self::PaymentIntentFailure
            }
            BetterpaymentWebhookStatus::Started | BetterpaymentWebhookStatus::Pending => {
                Self::PaymentIntentProcessing
            }
            BetterpaymentWebhookStatus::Canceled => Self::PaymentIntentCancelled,
            BetterpaymentWebhookStatus::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

impl From<BetterpaymentWebhookStatus> for AttemptStatus {
    fn from(status: BetterpaymentWebhookStatus) -> Self {
        match status {
            BetterpaymentWebhookStatus::Completed => Self::Charged,
            BetterpaymentWebhookStatus::Declined | BetterpaymentWebhookStatus::Error => {
                Self::Failure
            }
            BetterpaymentWebhookStatus::Started | BetterpaymentWebhookStatus::Pending => {
                Self::Pending
            }
            BetterpaymentWebhookStatus::Canceled => Self::Voided,
            BetterpaymentWebhookStatus::Unknown => Self::Pending,
        }
    }
}

/// Derives the UCS EventType from a parsed webhook body.
/// Refund postbacks are identified by the presence of `refund_id`.
pub fn get_webhook_event_type(body: &BetterpaymentWebhookBody) -> EventType {
    if body.refund_id.is_some() {
        return get_refund_webhook_event_type(body.refund_status_code);
    }
    EventType::from(body.status.clone())
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BetterpaymentRefundWebhookStatus {
    #[serde(alias = "0")]
    Started,
    #[serde(alias = "1")]
    Success,
    #[serde(alias = "2")]
    Error,
    #[serde(alias = "3")]
    Local,
    #[serde(alias = "4")]
    Reversed,
    #[serde(alias = "5")]
    Declined,
    #[serde(other)]
    Unknown,
}

fn deserialize_refund_webhook_status<'de, D>(
    deserializer: D,
) -> Result<Option<BetterpaymentRefundWebhookStatus>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let code = Option::<i64>::deserialize(deserializer)?;
    code.map(|code| {
        // Serde enum aliases are strings; the API sends an integer code.
        serde_json::from_value(serde_json::Value::String(code.to_string()))
            .map_err(serde::de::Error::custom)
    })
    .transpose()
}

pub fn get_refund_webhook_event_type(
    status: Option<BetterpaymentRefundWebhookStatus>,
) -> EventType {
    match status {
        Some(BetterpaymentRefundWebhookStatus::Started) => EventType::RefundProcessing,
        Some(BetterpaymentRefundWebhookStatus::Success) => EventType::RefundSuccess,
        Some(
            BetterpaymentRefundWebhookStatus::Error
            | BetterpaymentRefundWebhookStatus::Reversed
            | BetterpaymentRefundWebhookStatus::Declined,
        ) => EventType::RefundFailure,
        Some(
            BetterpaymentRefundWebhookStatus::Local | BetterpaymentRefundWebhookStatus::Unknown,
        )
        | None => EventType::RefundProcessing,
    }
}

pub fn get_refund_webhook_status(
    status: Option<BetterpaymentRefundWebhookStatus>,
) -> Result<common_enums::RefundStatus, error_stack::Report<errors::WebhookError>> {
    match status {
        Some(BetterpaymentRefundWebhookStatus::Started) => Ok(common_enums::RefundStatus::Pending),
        Some(BetterpaymentRefundWebhookStatus::Success) => Ok(common_enums::RefundStatus::Success),
        Some(
            BetterpaymentRefundWebhookStatus::Error
            | BetterpaymentRefundWebhookStatus::Reversed
            | BetterpaymentRefundWebhookStatus::Declined,
        ) => Ok(common_enums::RefundStatus::Failure),
        Some(
            BetterpaymentRefundWebhookStatus::Local | BetterpaymentRefundWebhookStatus::Unknown,
        )
        | None => Ok(common_enums::RefundStatus::Pending),
    }
}
