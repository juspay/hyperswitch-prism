use crate::types::ResponseRouterData;
use base64::Engine;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::MinorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::travelhub::TravelhubRouterData;

// Authentication Types

#[derive(Debug, Clone)]
pub struct TravelhubAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TravelhubAuthType {
    pub fn generate_authorization_header(&self) -> String {
        let credentials = format!("{}:{}", self.username.peek(), self.password.peek());
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes())
        )
    }

    pub fn get_merchant_id(&self) -> String {
        self.merchant_id.peek().to_string()
    }
}

impl TryFrom<&ConnectorSpecificConfig> for TravelhubAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Travelhub {
                username,
                password,
                merchant_id,
                ..
            } => Ok(Self {
                username: username.to_owned(),
                password: password.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Ensure the connector account is configured with Travelhub credentials (merchant_id, username, password)"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "ConnectorSpecificConfig variant mismatch: expected Travelhub credentials but received credentials for a different connector; the request may have been routed to the wrong connector"
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

// Error Response Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelhubErrorResponse {
    pub timestamp: Option<i64>,
    pub status: Option<i32>,
    pub error: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<String>,
    pub path: Option<String>,
}

// Authorize Request Types

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubRequest3DS {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv_algorithm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_transaction_id: Option<String>,
    #[serde(
        rename = "threeDSecureVersion",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_secure_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acs_transaction_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubPaymentCard<T: PaymentMethodDataTypes> {
    pub card_name: Secret<String>,
    pub card_number: RawCardNumber<T>,
    pub expiry_date: Secret<String>,
    pub cvc: Secret<String>,
    #[serde(rename = "request3DS", skip_serializing_if = "Option::is_none")]
    pub request3ds: Option<TravelhubRequest3DS>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TravelhubPaymentMethod {
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubPayment<T: PaymentMethodDataTypes> {
    pub payment_method: TravelhubPaymentMethod,
    pub payment_card: TravelhubPaymentCard<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubPaymentsRequest<T: PaymentMethodDataTypes> {
    pub merchant_id: String,
    pub order_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    pub capture: bool,
    pub payment: TravelhubPayment<T>,
}

fn get_card_payment_method_code<T: PaymentMethodDataTypes>(
    card: &domain_types::payment_method_data::Card<T>,
) -> Result<&str, IntegrationError> {
    match card.card_network {
        Some(common_enums::CardNetwork::Visa) => Ok("108"),
        Some(common_enums::CardNetwork::Mastercard) => Ok("102"),
        Some(common_enums::CardNetwork::AmericanExpress) => Ok("117"),
        Some(common_enums::CardNetwork::Discover) => Ok("159"),
        Some(common_enums::CardNetwork::DinersClub) => Ok("115"),
        Some(common_enums::CardNetwork::JCB) => Ok("123"),
        Some(common_enums::CardNetwork::CartesBancaires) => Ok("130"),
        Some(common_enums::CardNetwork::UnionPay) => Ok("197"),
        _ => Err(IntegrationError::NotSupported {
            message: "card network".to_string(),
            connector: "travelhub",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Use a card from a supported network: Visa, Mastercard, American Express, Discover, Diners Club, JCB, Cartes Bancaires, or UnionPay"
                        .to_string(),
                ),
                doc_url: None,
                additional_context: Some(
                    "The provided card network is not supported by travelhub card payments"
                        .to_string(),
                ),
            },
        }),
    }
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for TravelhubPaymentsRequest<T>
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
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        let payment_method_data = &item.request.payment_method_data;
        let card_data = match payment_method_data {
            PaymentMethodData::Card(card_data) => card_data,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "travelhub",
                    context: IntegrationErrorContext {
                        suggested_action: Some("Use card as the payment method".to_string()),
                        doc_url: None,
                        additional_context: Some(
                            "Travelhub currently supports only card payments".to_string(),
                        ),
                    },
                }
                .into());
            }
        };

        let cardholder_name = card_data
            .card_holder_name
            .clone()
            .or_else(|| item.resource_common_data.get_optional_billing_first_name())
            .or_else(|| item.resource_common_data.get_optional_shipping_first_name())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "card.card_holder_name or billing.first_name or shipping.first_name",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide the cardholder name via card.card_holder_name, billing.first_name, or shipping.first_name"
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Travelhub requires the cardholder name (card_name) for card payments"
                            .to_string(),
                    ),
                },
            })?;

        let expiry_date = crate::utils::format_card_expiry_mmyy(card_data)?;

        let payment_method_code = get_card_payment_method_code(card_data)?.to_string();

        let is_auto_capture = !crate::utils::is_manual_capture(item.request.capture_method);

        let is_already_authenticated = item.request.authentication_data.is_some();
        let authentication = if is_already_authenticated {
            Some(false)
        } else {
            match item.resource_common_data.auth_type {
                common_enums::AuthenticationType::ThreeDs => Some(true),
                common_enums::AuthenticationType::NoThreeDs => Some(false),
            }
        };

        let request3ds = item.request.authentication_data.as_ref().map(|auth_data| {
            let cavv_algorithm = auth_data
                .network_params
                .as_ref()
                .and_then(|np| np.cartes_bancaires.as_ref())
                .map(|cb| crate::utils::cavv_algorithm_to_str(cb.cavv_algorithm.clone()).to_string());
            TravelhubRequest3DS {
                cavv: auth_data.cavv.as_ref().map(|c| c.peek().to_string()),
                cavv_algorithm,
                eci: auth_data.eci.clone(),
                xid: None,
                ds_transaction_id: auth_data.ds_trans_id.clone(),
                three_ds_secure_version: auth_data.message_version.as_ref().map(|v| v.to_string()),
                acs_transaction_id: None,
            }
        });

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.request.minor_amount,
            currency: item.request.currency,
            capture: is_auto_capture,
            payment: TravelhubPayment {
                payment_method: TravelhubPaymentMethod {
                    code: payment_method_code,
                },
                payment_card: TravelhubPaymentCard {
                    card_name: cardholder_name,
                    card_number: card_data.card_number.clone(),
                    expiry_date,
                    cvc: card_data.card_cvc.clone(),
                    request3ds,
                    authentication,
                },
            },
        })
    }
}

// Response Types

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubResponse3DS {
    #[serde(rename = "acsURL", default, skip_serializing_if = "Option::is_none")]
    pub acs_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pa_req: Option<Secret<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md: Option<Secret<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub three_ds_secure_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory_server_transaction_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub three_d_server_transaction_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TravelhubRedirect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TravelhubResult {
    Approved,
    Captured,
    Settled,
    Declined,
    Error,
    Invalid,
    Redirected,
    Cancelled,
    Refunded,
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TravelhubPaymentsResponse {
    #[serde(rename = "merchantId", default)]
    pub merchant_id: Option<String>,
    #[serde(rename = "orderId", default)]
    pub order_id: Option<String>,
    #[serde(rename = "transactionId", default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub amount: Option<MinorUnit>,
    #[serde(default)]
    pub currency: Option<Currency>,
    #[serde(default)]
    pub result: Option<TravelhubResult>,
    #[serde(
        rename = "authorizationCode",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub authorization_code: Option<String>,
    #[serde(
        rename = "paymentMethodCode",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub payment_method_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect: Option<TravelhubRedirect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(
        rename = "response3DS",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub response3ds: Option<TravelhubResponse3DS>,
}

pub type TravelhubAuthorizeResponse = TravelhubPaymentsResponse;
pub type TravelhubCaptureResponse = TravelhubPaymentsResponse;
pub type TravelhubPSyncResponse = TravelhubPaymentsResponse;
pub type TravelhubVoidResponse = TravelhubPaymentsResponse;
pub type TravelhubRefundResponse = TravelhubPaymentsResponse;
pub type TravelhubRSyncResponse = TravelhubPaymentsResponse;

fn map_travelhub_status(result: &TravelhubResult) -> AttemptStatus {
    match result {
        TravelhubResult::Approved => AttemptStatus::Authorized,
        TravelhubResult::Captured | TravelhubResult::Settled => AttemptStatus::Charged,
        TravelhubResult::Declined | TravelhubResult::Error | TravelhubResult::Invalid => {
            AttemptStatus::Failure
        }
        TravelhubResult::Redirected => AttemptStatus::AuthenticationPending,
        TravelhubResult::Cancelled => AttemptStatus::Voided,
        TravelhubResult::Pending | TravelhubResult::Unknown => AttemptStatus::Pending,
        TravelhubResult::Refunded => AttemptStatus::Charged,
    }
}

fn map_travelhub_refund_status(result: &TravelhubResult) -> RefundStatus {
    match result {
        TravelhubResult::Approved | TravelhubResult::Refunded | TravelhubResult::Settled => {
            RefundStatus::Success
        }
        TravelhubResult::Declined | TravelhubResult::Error | TravelhubResult::Invalid => {
            RefundStatus::Failure
        }
        TravelhubResult::Pending | TravelhubResult::Unknown => RefundStatus::Pending,
        _ => RefundStatus::Pending,
    }
}

// Authorize Response Transformation

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TravelhubPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);

        let redirection_data = item.response.response3ds.as_ref().and_then(|r3ds| {
            r3ds.acs_url.as_ref().map(|acs_url| {
                let mut form_fields = std::collections::HashMap::new();
                if let Some(pa_req) = &r3ds.pa_req {
                    form_fields.insert("PaReq".to_string(), pa_req.peek().to_owned());
                }
                if let Some(md) = &r3ds.md {
                    form_fields.insert("MD".to_string(), md.peek().to_owned());
                }
                Box::new(RedirectForm::Form {
                    endpoint: acs_url.clone(),
                    method: common_utils::Method::Post,
                    form_fields,
                })
            })
        });

        let status = if redirection_data.is_some() {
            AttemptStatus::AuthenticationPending
        } else if result == &TravelhubResult::Approved {
            let is_auto_capture =
                !crate::utils::is_manual_capture(item.router_data.request.capture_method);
            if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        } else {
            map_travelhub_status(result)
        };

        let resource_id = item
            .response
            .transaction_id
            .clone()
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
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

// Capture Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubCaptureRequest {
    pub merchant_id: String,
    pub order_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
}

impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for TravelhubCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.request.minor_amount_to_capture,
            currency: item.request.currency,
        })
    }
}

// Capture Response

impl TryFrom<ResponseRouterData<TravelhubCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);
        let status = map_travelhub_status(result);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item
                    .response
                    .transaction_id
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
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

// Void Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubVoidRequest {
    pub merchant_id: String,
    pub order_id: String,
}

impl TryFrom<&RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for TravelhubVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

// Void Response

impl TryFrom<ResponseRouterData<TravelhubVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);
        let status = map_travelhub_status(result);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item
                    .response
                    .transaction_id
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
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

// PSync Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubPSyncRequest {
    pub merchant_id: String,
    pub order_id: String,
}

impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for TravelhubPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

// PSync Response

impl TryFrom<ResponseRouterData<TravelhubPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);
        let status = if result == &TravelhubResult::Approved {
            let is_auto_capture =
                !crate::utils::is_manual_capture(item.router_data.request.capture_method);
            if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        } else {
            map_travelhub_status(result)
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item
                    .response
                    .transaction_id
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
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

// Refund Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubRefundRequest {
    pub merchant_id: String,
    pub order_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for TravelhubRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.request.minor_refund_amount,
            currency: item.request.currency,
        })
    }
}

// Refund Response

impl TryFrom<ResponseRouterData<TravelhubRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);
        let refund_status = map_travelhub_refund_status(result);

        let connector_refund_id = item.response.transaction_id.clone().ok_or_else(|| {
            ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in travelhub refund response".to_string()),
            )
        })?;

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// RSync Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubRSyncRequest {
    pub merchant_id: String,
    pub order_id: String,
}

impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for TravelhubRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

// RSync Response

impl TryFrom<ResponseRouterData<TravelhubRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);
        let refund_status = map_travelhub_refund_status(result);

        let connector_refund_id = item.response.transaction_id.clone().ok_or_else(|| {
            ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in travelhub refund sync response".to_string()),
            )
        })?;

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// Macro Wrapper Type Implementations

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TravelhubPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TravelhubCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TravelhubVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TravelhubRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for TravelhubPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for TravelhubRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}
