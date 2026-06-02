use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::{Send, Sync};

use common_enums::{AttemptStatus, CaptureMethod, Currency};
use common_utils::types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        EventType, PaymentFlowData, PaymentVoidData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundWebhookReference, RefundsData, RefundsResponseData,
        ResponseId, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::AsiapayRouterData;
use crate::types::ResponseRouterData;
use domain_types::router_data::ErrorResponse;

pub struct AsiaPayAuthType {
    pub(super) merchant_id: Secret<String>,
    pub(super) secure_hash_secret: Secret<String>,
    pub(super) login_id: Secret<String>,
    pub(super) password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AsiaPayAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Asiapay {
                merchant_id,
                secure_hash_secret,
                login_id,
                password,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                secure_hash_secret: secure_hash_secret.to_owned(),
                login_id: login_id.to_owned(),
                password: password.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

fn get_currency_code(
    currency: Currency,
) -> Result<&'static str, error_stack::Report<IntegrationError>> {
    match currency {
        Currency::HKD => Ok("344"),
        Currency::SGD => Ok("702"),
        Currency::USD => Ok("840"),
        Currency::CNY => Ok("156"),
        Currency::JPY => Ok("392"),
        Currency::TWD => Ok("901"),
        Currency::AUD => Ok("036"),
        Currency::EUR => Ok("978"),
        Currency::GBP => Ok("826"),
        Currency::CAD => Ok("124"),
        Currency::AED => Ok("784"),
        Currency::THB => Ok("764"),
        Currency::MYR => Ok("458"),
        Currency::PHP => Ok("608"),
        Currency::INR => Ok("356"),
        Currency::IDR => Ok("360"),
        Currency::NZD => Ok("554"),
        Currency::VND => Ok("704"),
        _ => Err(IntegrationError::CurrencyNotSupported {
            message: format!("Currency {currency} is not supported by AsiaPay"),
            connector: "asiapay",
            context: Default::default(),
        }
        .into()),
    }
}

fn compute_sha256_hex(input: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    let digest = ring::digest::digest(&ring::digest::SHA256, input.as_bytes());
    Ok(hex::encode(digest.as_ref()))
}

fn stringify_amount(
    amount: &StringMajorUnit,
) -> Result<String, error_stack::Report<IntegrationError>> {
    serde_json::to_value(amount)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
        })
}

fn map_order_status(status: &str) -> AttemptStatus {
    match status {
        "Accepted" | "Captured" | "Accepted_Adj" => AttemptStatus::Charged,
        "Authorized" => AttemptStatus::Authorized,
        "Pending" => AttemptStatus::Pending,
        "Pending_3D" => AttemptStatus::AuthenticationPending,
        "Capturing" => AttemptStatus::CaptureInitiated,
        "Rejected" => AttemptStatus::Failure,
        "Cancelled" | "Voided" | "Reverse Auth" | "Reversal Void" | "Reversal-CB" => {
            AttemptStatus::Voided
        }
        "Refunded"
        | "Partial Refunded"
        | "RequestRefund"
        | "RequestPartialRefund"
        | "ChargeBack"
        | "Partial ChargeBack" => AttemptStatus::AutoRefunded,
        _ => AttemptStatus::Failure,
    }
}

// ============================================================================
// AUTHORIZE TYPES
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsiaPayPaymentRequest {
    pub merchant_id: Secret<String>,
    pub order_ref: String,
    pub amount: StringMajorUnit,
    pub curr_code: String,
    pub pay_type: String,
    pub pay_method: String,
    pub lang: String,
    pub success_url: String,
    pub fail_url: String,
    pub cancel_url: String,
    pub secure_hash: Secret<String>,
}

#[derive(Debug, Deserialize)]
pub struct AsiaPayRedirectResponse {
    #[serde(alias = "successCode", alias = "successcode")]
    pub success_code: Option<String>,
    #[serde(alias = "ref", alias = "orderRef")]
    pub order_ref: Option<String>,
    #[serde(alias = "payRef", alias = "payref")]
    pub pay_ref: Option<String>,
    #[serde(alias = "amt")]
    pub amt: Option<String>,
    #[serde(alias = "cur")]
    pub cur: Option<String>,
    #[serde(alias = "errMsg", alias = "errmsg")]
    pub err_msg: Option<String>,
    #[serde(alias = "orderStatus", alias = "orderstatus")]
    pub order_status: Option<String>,
    #[serde(alias = "prc")]
    pub prc: Option<String>,
    #[serde(alias = "src")]
    pub src: Option<String>,
    #[serde(alias = "authId", alias = "authid")]
    pub auth_id: Option<String>,
    #[serde(alias = "secureHash", alias = "securehash")]
    pub secure_hash: Option<String>,
    #[serde(alias = "payerAuthStatus")]
    pub payer_auth_status: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AsiaPayRedirectResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiaPayRedirectResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let auth = AsiaPayAuthType::try_from(&router_data.connector_config).change_context(
            ConnectorError::ResponseHandlingFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some("Failed to extract auth type".to_string()),
                },
            },
        )?;

        let base_url = &router_data.resource_common_data.connectors.asiapay.base_url;
        let payment_url = format!("{}/payment/payForm.jsp", base_url);

        let currency_code = get_currency_code(router_data.request.currency).change_context(
            ConnectorError::ResponseHandlingFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some("Unsupported currency".to_string()),
                },
            },
        )?;

        let amount = StringMajorUnitForConnector
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some("Amount conversion failed".to_string()),
                },
            })?;
        let amount_str =
            stringify_amount(&amount).change_context(ConnectorError::ResponseHandlingFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some("Failed to serialize amount to string".to_string()),
                },
            })?;

        let pay_type = match router_data.request.capture_method {
            Some(CaptureMethod::Manual) => "H",
            _ => "N",
        };

        let return_url = router_data
            .resource_common_data
            .return_url
            .clone()
            .unwrap_or_default();

        let order_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let merchant_id_str = auth.merchant_id.peek().to_string();

        let hash_input = format!(
            "{}|{}|{}|{}|{}|{}",
            merchant_id_str,
            order_ref,
            currency_code,
            amount_str,
            pay_type,
            auth.secure_hash_secret.peek()
        );
        let secure_hash = compute_sha256_hex(&hash_input).change_context(
            ConnectorError::ResponseHandlingFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some("Failed to compute secure hash".to_string()),
                },
            },
        )?;

        let mut form_fields = HashMap::new();
        form_fields.insert("merchantId".to_string(), merchant_id_str);
        form_fields.insert("orderRef".to_string(), order_ref.clone());
        form_fields.insert("amount".to_string(), amount_str);
        form_fields.insert("currCode".to_string(), currency_code.to_string());
        form_fields.insert("payType".to_string(), pay_type.to_string());
        form_fields.insert("payMethod".to_string(), "CC".to_string());
        form_fields.insert("lang".to_string(), "E".to_string());
        form_fields.insert("successUrl".to_string(), return_url.clone());
        form_fields.insert("failUrl".to_string(), return_url.clone());
        form_fields.insert("cancelUrl".to_string(), return_url);
        form_fields.insert("secureHash".to_string(), secure_hash);

        let redirection_data = Some(Box::new(RedirectForm::Form {
            endpoint: payment_url,
            method: common_utils::request::Method::Post,
            form_fields,
        }));

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(order_ref),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// ============================================================================
// MERCHANT API REQUEST (Capture, Void, Refund)
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsiaPayMerchantApiRequest {
    pub merchant_id: Secret<String>,
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    pub action_type: String,
    pub pay_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

// Capture
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AsiaPayMerchantApiRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let connector = &item.connector;
        let router_data = &item.router_data;
        let auth = AsiaPayAuthType::try_from(&router_data.connector_config)?;

        let pay_ref = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
                .into())
            }
        };

        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: "Capture".to_string(),
            pay_ref,
            amount: Some(amount),
        })
    }
}

// Void
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AsiaPayMerchantApiRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AsiaPayAuthType::try_from(&router_data.connector_config)?;
        let pay_ref = router_data.request.connector_transaction_id.clone();

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: "Void".to_string(),
            pay_ref,
            amount: None,
        })
    }
}

// Refund
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AsiaPayMerchantApiRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let connector = &item.connector;
        let router_data = &item.router_data;
        let auth = AsiaPayAuthType::try_from(&router_data.connector_config)?;
        let pay_ref = router_data.request.connector_transaction_id.clone();

        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: "Refund".to_string(),
            pay_ref,
            amount: Some(amount),
        })
    }
}

// ============================================================================
// PSYNC
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsiaPaySyncRequest {
    pub merchant_id: Secret<String>,
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    pub action_type: String,
    pub pay_ref: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for AsiaPaySyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AsiaPayAuthType::try_from(&router_data.connector_config)?;
        let pay_ref = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: "Query".to_string(),
            pay_ref,
        })
    }
}

// ============================================================================
// RSYNC
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsiaPayRSyncRequest {
    pub merchant_id: Secret<String>,
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    pub action_type: String,
    pub pay_ref: String,
    pub refund_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for AsiaPayRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AsiaPayAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: "Query".to_string(),
            pay_ref: router_data.request.connector_transaction_id.clone(),
            refund_id: router_data.request.connector_refund_id.clone(),
        })
    }
}

// RSync response
impl TryFrom<ResponseRouterData<AsiaPayMerchantApiResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiaPayMerchantApiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let response = &item.response;

        let result_code = response.result_code.as_deref().unwrap_or("-1");

        if result_code == "0" {
            let order_status = response.order_status.as_deref().unwrap_or("Pending");
            let refund_status = match order_status {
                "Refunded" | "Partial Refunded" => common_enums::RefundStatus::Success,
                "Pending" | "RequestRefund" | "RequestPartialRefund" => {
                    common_enums::RefundStatus::Pending
                }
                _ => common_enums::RefundStatus::Failure,
            };

            let refund_id = response
                .pay_ref
                .clone()
                .unwrap_or_else(|| router_data.request.connector_refund_id.clone());

            Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: refund_id,
                    refund_status,
                    status_code: item.http_code,
                }),
                ..router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: result_code.to_string(),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| "Refund sync failed".to_string()),
                    reason: response.err_msg.clone(),
                    attempt_status: None,
                    connector_transaction_id: response.pay_ref.clone(),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..router_data
            })
        }
    }
}

// ============================================================================
// MERCHANT API RESPONSE (shared for Capture, Void, Refund, PSync)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct AsiaPayMerchantApiResponse {
    #[serde(alias = "resultCode", alias = "resultcode")]
    pub result_code: Option<String>,
    #[serde(alias = "orderStatus", alias = "orderstatus")]
    pub order_status: Option<String>,
    #[serde(alias = "ref")]
    pub order_ref: Option<String>,
    #[serde(alias = "payRef", alias = "payref")]
    pub pay_ref: Option<String>,
    #[serde(alias = "amt")]
    pub amt: Option<String>,
    #[serde(alias = "cur")]
    pub cur: Option<String>,
    #[serde(alias = "errMsg", alias = "errmsg")]
    pub err_msg: Option<String>,
    #[serde(alias = "successCode", alias = "successcode")]
    pub success_code: Option<String>,
}

// Capture response
impl TryFrom<ResponseRouterData<AsiaPayMerchantApiResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiaPayMerchantApiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let response = &item.response;

        let result_code = response.result_code.as_deref().unwrap_or("-1");
        let order_status = response.order_status.as_deref().unwrap_or("");

        if result_code == "0" {
            let status = map_order_status(order_status);
            let connector_transaction_id = response.pay_ref.clone().unwrap_or_else(|| {
                match &router_data.request.connector_transaction_id {
                    ResponseId::ConnectorTransactionId(id) => id.clone(),
                    _ => String::new(),
                }
            });

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: response.order_ref.clone(),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: result_code.to_string(),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| "Operation failed".to_string()),
                    reason: response.err_msg.clone(),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.pay_ref.clone(),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..router_data
            })
        }
    }
}

// Void response
impl TryFrom<ResponseRouterData<AsiaPayMerchantApiResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiaPayMerchantApiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let response = &item.response;

        let result_code = response.result_code.as_deref().unwrap_or("-1");
        let order_status = response.order_status.as_deref().unwrap_or("");

        if result_code == "0" {
            let status = map_order_status(order_status);
            let connector_transaction_id = response
                .pay_ref
                .clone()
                .unwrap_or_else(|| router_data.request.connector_transaction_id.clone());

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: response.order_ref.clone(),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: result_code.to_string(),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| "Void failed".to_string()),
                    reason: response.err_msg.clone(),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.pay_ref.clone(),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..router_data
            })
        }
    }
}

// PSync response
impl TryFrom<ResponseRouterData<AsiaPayMerchantApiResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiaPayMerchantApiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let response = &item.response;

        let result_code = response.result_code.as_deref().unwrap_or("-1");

        if result_code != "0" {
            return Ok(Self {
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: result_code.to_string(),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| "Sync failed".to_string()),
                    reason: response.err_msg.clone(),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.pay_ref.clone(),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..router_data
            });
        }

        let order_status = response.order_status.as_deref().unwrap_or("Pending");
        let status = map_order_status(order_status);

        let connector_transaction_id = response.pay_ref.clone().unwrap_or_else(|| {
            router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .unwrap_or_default()
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response.order_ref.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// Refund response
impl TryFrom<ResponseRouterData<AsiaPayMerchantApiResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiaPayMerchantApiResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let response = &item.response;

        let result_code = response.result_code.as_deref().unwrap_or("-1");

        if result_code == "0" {
            let refund_id = response
                .pay_ref
                .clone()
                .unwrap_or_else(|| router_data.request.connector_transaction_id.clone());

            Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: refund_id,
                    refund_status: common_enums::RefundStatus::Success,
                    status_code: item.http_code,
                }),
                ..router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: result_code.to_string(),
                    message: response
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| "Refund failed".to_string()),
                    reason: response.err_msg.clone(),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.pay_ref.clone(),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..router_data
            })
        }
    }
}

// ============================================================================
// WEBHOOK
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AsiaPayWebhookBody {
    #[serde(alias = "successCode", alias = "successcode")]
    pub success_code: Option<String>,
    #[serde(alias = "ref", alias = "orderRef")]
    pub order_ref: Option<String>,
    #[serde(alias = "payRef", alias = "payref")]
    pub pay_ref: Option<String>,
    #[serde(alias = "amt")]
    pub amt: Option<String>,
    #[serde(alias = "cur")]
    pub cur: Option<String>,
    #[serde(alias = "prc")]
    pub prc: Option<String>,
    #[serde(alias = "src")]
    pub src: Option<String>,
    #[serde(alias = "errMsg", alias = "errmsg")]
    pub err_msg: Option<String>,
    #[serde(alias = "orderStatus", alias = "orderstatus")]
    pub order_status: Option<String>,
    #[serde(alias = "secureHash", alias = "securehash")]
    pub secure_hash: Option<String>,
    #[serde(alias = "payerAuthStatus")]
    pub payer_auth_status: Option<String>,
}

impl AsiaPayWebhookBody {
    pub fn get_webhook_event_type(&self) -> EventType {
        let order_status = self.order_status.as_deref().unwrap_or("");
        let success_code = self.success_code.as_deref().unwrap_or("");

        match (order_status, success_code) {
            ("Accepted" | "Captured", "0") => EventType::PaymentIntentSuccess,
            ("Authorized", "0") => EventType::PaymentIntentAuthorizationSuccess,
            ("Rejected", _) | (_, "1") => EventType::PaymentIntentFailure,
            ("Voided" | "Cancelled", _) => EventType::PaymentIntentFailure,
            ("Refunded" | "Partial Refunded", _) => EventType::RefundSuccess,
            _ => EventType::IncomingWebhookEventUnspecified,
        }
    }

    pub fn compute_expected_hash(
        &self,
        secret: &str,
    ) -> Result<String, error_stack::Report<IntegrationError>> {
        let src = self.src.as_deref().unwrap_or("");
        let prc = self.prc.as_deref().unwrap_or("");
        let success_code = self.success_code.as_deref().unwrap_or("");
        let order_ref = self.order_ref.as_deref().unwrap_or("");
        let pay_ref = self.pay_ref.as_deref().unwrap_or("");
        let cur = self.cur.as_deref().unwrap_or("");
        let amt = self.amt.as_deref().unwrap_or("");
        let payer_auth_status = self.payer_auth_status.as_deref().unwrap_or("");

        let hash_input = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            src, prc, success_code, order_ref, pay_ref, cur, amt, payer_auth_status, secret
        );
        compute_sha256_hex(&hash_input)
    }
}

pub fn get_webhook_resource_reference(webhook: &AsiaPayWebhookBody) -> WebhookResourceReference {
    let event_type = webhook.get_webhook_event_type();
    match event_type {
        EventType::RefundSuccess | EventType::RefundFailure => {
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: webhook.pay_ref.clone(),
                merchant_refund_id: None,
                connector_transaction_id: None,
            })
        }
        _ => WebhookResourceReference::Payment(PaymentWebhookReference {
            connector_transaction_id: webhook.pay_ref.clone(),
            merchant_transaction_id: webhook.order_ref.clone(),
        }),
    }
}

pub fn get_webhook_details_response(webhook: &AsiaPayWebhookBody) -> WebhookDetailsResponse {
    let order_status = webhook.order_status.as_deref().unwrap_or("Pending");
    let status = map_order_status(order_status);
    let connector_transaction_id = webhook.pay_ref.clone().unwrap_or_default();

    WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(connector_transaction_id)),
        status,
        connector_response_reference_id: webhook.order_ref.clone(),
        mandate_reference: None,
        error_code: None,
        error_message: None,
        error_reason: None,
        raw_connector_response: None,
        status_code: 200,
        response_headers: None,
        amount_captured: None,
        minor_amount_captured: None,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    }
}

// ============================================================================
// ERROR RESPONSE
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct AsiaPayErrorResponse {
    #[serde(
        alias = "resultCode",
        alias = "resultcode",
        alias = "successCode",
        alias = "successcode"
    )]
    pub error_code: Option<String>,
    #[serde(alias = "errMsg", alias = "errmsg")]
    pub error_message: Option<String>,
}
