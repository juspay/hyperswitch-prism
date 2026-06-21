use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::StringMajorUnit;
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        EventType, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::AsiapayRouterData;

// ===== AUTH TYPE =====
#[derive(Debug, Clone)]
pub struct AsiapayAuthType {
    pub merchant_id: Secret<String>,
    // Not required in SandBox/Local env will have to check for prod
    // The field is kept for compatibility but not currently used in request signing.
    pub secure_hash_secret: Secret<String>,
    pub login_id: Secret<String>,
    pub password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AsiapayAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Asiapay {
            merchant_id,
            secure_hash_secret,
            login_id,
            password,
            ..
        } = item
        {
            Ok(Self {
                merchant_id: merchant_id.clone(),
                secure_hash_secret: secure_hash_secret.clone(),
                login_id: login_id.clone(),
                password: password.clone(),
            })
        } else {
            Err(IntegrationError::InvalidConnectorConfig {
                config: "Asiapay",
                context: Default::default(),
            }
            .into())
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct AsiapayErrorResponse {
    #[serde(alias = "successCode", alias = "SuccessCode")]
    pub success_code: Option<String>,
    #[serde(alias = "errMsg", alias = "ErrMsg")]
    pub err_msg: Option<String>,
    #[serde(alias = "prc", alias = "Prc")]
    pub prc: Option<String>,
    #[serde(alias = "src", alias = "Src")]
    pub src: Option<String>,
}

impl AsiapayErrorResponse {
    pub fn get_error_message(&self) -> String {
        self.err_msg
            .clone()
            .unwrap_or_else(|| "Unknown error".to_string())
    }
}

// ===== DIRECT PAY (AUTHORIZE) REQUEST =====

/// Convert a `Currency` to AsiaPay's numeric currency code.
pub fn get_asiapay_currency_code(
    currency: common_enums::Currency,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let code = match currency {
        common_enums::Currency::AED => "784",
        common_enums::Currency::AUD => "036",
        common_enums::Currency::CAD => "124",
        common_enums::Currency::CHF => "756",
        common_enums::Currency::CNY => "156",
        common_enums::Currency::EUR => "978",
        common_enums::Currency::GBP => "826",
        common_enums::Currency::HKD => "344",
        common_enums::Currency::IDR => "360",
        common_enums::Currency::INR => "356",
        common_enums::Currency::JPY => "392",
        common_enums::Currency::KRW => "410",
        common_enums::Currency::MYR => "458",
        common_enums::Currency::NZD => "554",
        common_enums::Currency::PHP => "608",
        common_enums::Currency::SGD => "702",
        common_enums::Currency::THB => "764",
        common_enums::Currency::TWD => "901",
        common_enums::Currency::USD => "840",
        common_enums::Currency::VND => "704",
        _ => {
            return Err(IntegrationError::NotImplemented(
                format!("Currency {:?} is not supported by AsiaPay", currency),
                Default::default(),
            )
            .into());
        }
    };
    Ok(code.to_string())
}

/// Reverse lookup: convert AsiaPay numeric currency code to `Currency`.
pub fn get_currency_from_asiapay_code(code: &str) -> Option<common_enums::Currency> {
    match code {
        "784" => Some(common_enums::Currency::AED),
        "036" => Some(common_enums::Currency::AUD),
        "124" => Some(common_enums::Currency::CAD),
        "756" => Some(common_enums::Currency::CHF),
        "156" => Some(common_enums::Currency::CNY),
        "978" => Some(common_enums::Currency::EUR),
        "826" => Some(common_enums::Currency::GBP),
        "344" => Some(common_enums::Currency::HKD),
        "360" => Some(common_enums::Currency::IDR),
        "356" => Some(common_enums::Currency::INR),
        "392" => Some(common_enums::Currency::JPY),
        "410" => Some(common_enums::Currency::KRW),
        "458" => Some(common_enums::Currency::MYR),
        "554" => Some(common_enums::Currency::NZD),
        "608" => Some(common_enums::Currency::PHP),
        "702" => Some(common_enums::Currency::SGD),
        "764" => Some(common_enums::Currency::THB),
        "901" => Some(common_enums::Currency::TWD),
        "840" => Some(common_enums::Currency::USD),
        "704" => Some(common_enums::Currency::VND),
        _ => None,
    }
}

/// AsiaPay pay type for capture method.
#[derive(Debug, Serialize)]
pub enum AsiapayPayType {
    /// Normal / Automatic capture
    N,
    /// Hold / Manual capture
    H,
}

impl AsiapayPayType {
    pub fn from_capture_method(method: Option<common_enums::CaptureMethod>) -> Self {
        match method {
            Some(common_enums::CaptureMethod::Automatic) => Self::N,
            _ => Self::H,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsiapayDirectPayRequest {
    pub merchant_id: Secret<String>,
    pub order_ref: String,
    pub amount: StringMajorUnit,
    pub curr_code: String,
    pub pay_type: AsiapayPayType,
    pub lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_method: Option<String>,
    pub ep_month: Secret<String>,
    pub ep_year: Secret<String>,
    pub card_no: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder: Option<Secret<String>>,
    pub security_code: Secret<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AsiapayDirectPayRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AsiapayAuthType::try_from(&item.router_data.connector_config)?;
        let req = &item.router_data.request;

        let (card_number, exp_month, exp_year, cvc, card_holder_name) =
            match &req.payment_method_data {
                PaymentMethodData::Card(card) => (
                    card.card_number.clone(),
                    card.card_exp_month.clone(),
                    card.card_exp_year.clone(),
                    card.card_cvc.clone(),
                    card.card_holder_name.clone(),
                ),
                _ => {
                    return Err(IntegrationError::NotImplemented(
                        "Only card payments are supported for Asiapay".to_string(),
                        Default::default(),
                    )
                    .into())
                }
            };

        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        let curr_code = get_asiapay_currency_code(req.currency)?;

        let pay_type = AsiapayPayType::from_capture_method(req.capture_method);

        Ok(Self {
            merchant_id: auth.merchant_id,
            order_ref: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            curr_code,
            pay_type,
            lang: "E".to_string(),
            p_method: None,
            ep_month: exp_month,
            ep_year: exp_year,
            card_no: Secret::new(card_number.peek().to_string()),
            card_holder: card_holder_name,
            security_code: cvc,
        })
    }
}

// ===== DIRECT PAY (AUTHORIZE) RESPONSE =====
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct AsiapayDirectPayResponse {
    #[serde(alias = "successCode", alias = "SuccessCode")]
    pub success_code: String,
    #[serde(alias = "ref", alias = "Ref", alias = "orderRef", alias = "OrderRef")]
    pub order_ref: Option<String>,
    #[serde(alias = "payRef", alias = "PayRef")]
    pub pay_ref: Option<String>,
    #[serde(alias = "amt", alias = "Amt")]
    pub amt: Option<String>,
    #[serde(alias = "cur", alias = "Cur")]
    pub cur: Option<String>,
    #[serde(alias = "errMsg", alias = "ErrMsg")]
    pub err_msg: Option<String>,
    #[serde(alias = "orderStatus", alias = "OrderStatus")]
    pub order_status: Option<String>,
    #[serde(alias = "prc", alias = "Prc")]
    pub prc: String,
    #[serde(alias = "src", alias = "Src")]
    pub src: String,
    #[serde(alias = "authId", alias = "AuthId")]
    pub auth_id: Option<String>,
    #[serde(alias = "Holder", alias = "holder")]
    pub holder: Option<String>,
    #[serde(alias = "authDate", alias = "AuthDate")]
    pub auth_date: Option<String>,
    #[serde(alias = "captureDate", alias = "CaptureDate")]
    pub capture_date: Option<String>,
    #[serde(alias = "batchId", alias = "BatchId")]
    pub batch_id: Option<String>,
    #[serde(alias = "settleDate", alias = "SettleDate")]
    pub settle_date: Option<String>,
    #[serde(alias = "merRef", alias = "MerRef")]
    pub mer_ref: Option<String>,
    #[serde(alias = "surcharge", alias = "Surcharge")]
    pub surcharge: Option<String>,
    #[serde(alias = "merRequestAmt", alias = "MerRequestAmt")]
    pub mer_request_amt: Option<String>,
    #[serde(alias = "terminal", alias = "Terminal")]
    pub terminal: Option<String>,
    #[serde(alias = "bankMid", alias = "BankMid")]
    pub bank_mid: Option<String>,
    #[serde(alias = "settleFlag", alias = "SettleFlag")]
    pub settle_flag: Option<String>,
    #[serde(alias = "bank", alias = "Bank")]
    pub bank: Option<String>,
    #[serde(alias = "bankRef", alias = "BankRef")]
    pub bank_ref: Option<String>,
    #[serde(alias = "traceNo", alias = "TraceNo")]
    pub trace_no: Option<String>,
    #[serde(alias = "accountNo", alias = "AccountNo")]
    pub account_no: Option<String>,
    #[serde(alias = "currency", alias = "Currency")]
    pub currency: Option<String>,
    #[serde(alias = "remark", alias = "Remark")]
    pub remark: Option<String>,
    #[serde(alias = "originalAmt", alias = "OriginalAmt")]
    pub original_amt: Option<String>,
    #[serde(alias = "txTime", alias = "TxTime")]
    pub tx_time: Option<String>,
}

/// Connector metadata extracted from AsiaPay responses.
/// Both `AsiapayDirectPayResponse` and `AsiapayPSyncResponse` produce this struct.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AsiapayConnectorMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pay_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mer_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surcharge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mer_request_amt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_mid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_flag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_amt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_time: Option<String>,
}

impl AsiapayConnectorMetadata {
    fn into_json_value(self) -> Option<serde_json::Value> {
        match serde_json::to_value(self).ok()? {
            serde_json::Value::Object(map) if map.is_empty() => None,
            val => Some(val),
        }
    }
}

impl From<&AsiapayDirectPayResponse> for AsiapayConnectorMetadata {
    fn from(response: &AsiapayDirectPayResponse) -> Self {
        Self {
            pay_ref: response.pay_ref.clone(),
            auth_id: response.auth_id.clone(),
            holder: response.holder.clone(),
            auth_date: response.auth_date.clone(),
            capture_date: response.capture_date.clone(),
            batch_id: response.batch_id.clone(),
            settle_date: response.settle_date.clone(),
            mer_ref: response.mer_ref.clone(),
            surcharge: response.surcharge.clone(),
            mer_request_amt: response.mer_request_amt.clone(),
            terminal: response.terminal.clone(),
            bank_mid: response.bank_mid.clone(),
            settle_flag: response.settle_flag.clone(),
            bank: response.bank.clone(),
            bank_ref: response.bank_ref.clone(),
            trace_no: response.trace_no.clone(),
            account_no: response.account_no.clone(),
            currency: response.currency.clone(),
            remark: response.remark.clone(),
            original_amt: response.original_amt.clone(),
            tx_time: response.tx_time.clone(),
        }
    }
}

impl From<&AsiapayPSyncResponse> for AsiapayConnectorMetadata {
    fn from(response: &AsiapayPSyncResponse) -> Self {
        Self {
            pay_ref: response.pay_ref.clone(),
            auth_id: response.auth_id.clone(),
            holder: response.holder.clone(),
            auth_date: response.auth_date.clone(),
            capture_date: response.capture_date.clone(),
            batch_id: response.batch_id.clone(),
            settle_date: response.settle_date.clone(),
            mer_ref: response.mer_ref.clone(),
            surcharge: response.surcharge.clone(),
            mer_request_amt: response.mer_request_amt.clone(),
            terminal: response.terminal.clone(),
            bank_mid: response.bank_mid.clone(),
            settle_flag: response.settle_flag.clone(),
            bank: response.bank.clone(),
            bank_ref: response.bank_ref.clone(),
            trace_no: response.trace_no.clone(),
            account_no: response.account_no.clone(),
            currency: response.currency.clone(),
            remark: response.remark.clone(),
            original_amt: response.original_amt.clone(),
            tx_time: response.tx_time.clone(),
        }
    }
}

impl AsiapayDirectPayResponse {
    pub fn build_connector_metadata(&self) -> Option<serde_json::Value> {
        AsiapayConnectorMetadata::from(self).into_json_value()
    }

    pub fn is_successful(&self) -> bool {
        (&self.prc == "0" && &self.src == "0") || &self.success_code == "0"
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AsiapayDirectPayResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<AsiapayDirectPayResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let payments_response_data = if !response.is_successful() {
            PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_ref.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }
        } else {
            PaymentsResponseData::TransactionResponse {
                resource_id: response
                    .pay_ref
                    .as_ref()
                    .cloned()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.build_connector_metadata(),
                network_txn_id: response.auth_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: response.order_ref.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status: if response.is_successful() {
                    if let Some(ref order_status) = response.order_status {
                        map_order_status(order_status)
                    } else {
                        AttemptStatus::Charged
                    }
                } else {
                    AttemptStatus::Failure
                },
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND REQUEST =====
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsiapayRefundRequest {
    pub merchant_id: Secret<String>,
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    pub action_type: String,
    pub pay_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AsiapayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AsiapayAuthType::try_from(&item.router_data.connector_config)?;
        let req = &item.router_data.request;

        let amount = item
            .connector
            .amount_converter
            .convert(req.minor_refund_amount, req.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: "RequestRefund".to_string(),
            pay_ref: req.connector_transaction_id.clone(),
            amount: Some(amount),
        })
    }
}

// ===== REFUND RESPONSE =====
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct AsiapayRefundResponse {
    #[serde(alias = "resultCode", alias = "ResultCode")]
    pub result_code: Option<String>,
    #[serde(alias = "orderStatus", alias = "OrderStatus")]
    pub order_status: Option<String>,
    #[serde(alias = "ref", alias = "Ref")]
    pub order_ref: Option<String>,
    #[serde(alias = "payRef", alias = "PayRef")]
    pub pay_ref: Option<String>,
    #[serde(alias = "amt", alias = "Amt")]
    pub amt: Option<String>,
    #[serde(alias = "cur", alias = "Cur")]
    pub cur: Option<String>,
    #[serde(alias = "errMsg", alias = "ErrMsg")]
    pub err_msg: Option<String>,
    #[serde(alias = "successCode", alias = "Successcode")]
    pub success_code: Option<String>,
}

impl AsiapayRefundResponse {
    pub fn is_successful(&self) -> bool {
        self.result_code.as_deref() == Some("0")
            || self.success_code.as_deref() == Some("0")
            // Query API omits result_code on success and only populates order_status.
            || (self.result_code.is_none() && self.order_status.is_some())
    }
}

impl TryFrom<ResponseRouterData<AsiapayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<AsiapayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let refunds_response_data = if !response.is_successful() {
            RefundsResponseData {
                connector_refund_id: response.pay_ref.clone().unwrap_or_default(),
                refund_status: RefundStatus::Failure,
                status_code: item.http_code,
            }
        } else {
            let refund_status = response
                .order_status
                .as_deref()
                .map(map_refund_status)
                .unwrap_or(RefundStatus::Pending);

            RefundsResponseData {
                connector_refund_id: response.pay_ref.clone().unwrap_or_default(),
                refund_status,
                status_code: item.http_code,
            }
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            resource_common_data: RefundFlowData {
                status: if response.is_successful() {
                    response
                        .order_status
                        .as_deref()
                        .map(map_refund_status)
                        .unwrap_or(RefundStatus::Pending)
                } else {
                    RefundStatus::Failure
                },
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PSYNC REQUEST =====
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsiapayPSyncRequest {
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
    > for AsiapayPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AsiapayAuthType::try_from(&item.router_data.connector_config)?;
        let req = &item.router_data.request;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: "Query".to_string(),
            pay_ref: req
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?
                .to_string(),
        })
    }
}

// ===== PSYNC RESPONSE =====
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct AsiapayPSyncResponse {
    #[serde(alias = "resultCode", alias = "ResultCode")]
    pub result_code: Option<String>,
    #[serde(alias = "orderStatus", alias = "OrderStatus")]
    pub order_status: Option<String>,
    #[serde(alias = "ref", alias = "Ref")]
    pub order_ref: Option<String>,
    #[serde(alias = "payRef", alias = "PayRef")]
    pub pay_ref: Option<String>,
    #[serde(alias = "amt", alias = "Amt")]
    pub amt: Option<String>,
    #[serde(alias = "cur", alias = "Cur")]
    pub cur: Option<String>,
    #[serde(alias = "errMsg", alias = "ErrMsg")]
    pub err_msg: Option<String>,
    #[serde(alias = "successCode", alias = "Successcode")]
    pub success_code: Option<String>,
    #[serde(alias = "authId", alias = "AuthId")]
    pub auth_id: Option<String>,
    #[serde(alias = "prc", alias = "Prc")]
    pub prc: Option<String>,
    #[serde(alias = "src", alias = "Src")]
    pub src: Option<String>,
    #[serde(alias = "Holder", alias = "holder")]
    pub holder: Option<String>,
    #[serde(alias = "authDate", alias = "AuthDate")]
    pub auth_date: Option<String>,
    #[serde(alias = "captureDate", alias = "CaptureDate")]
    pub capture_date: Option<String>,
    #[serde(alias = "batchId", alias = "BatchId")]
    pub batch_id: Option<String>,
    #[serde(alias = "settleDate", alias = "SettleDate")]
    pub settle_date: Option<String>,
    #[serde(alias = "merRef", alias = "MerRef")]
    pub mer_ref: Option<String>,
    #[serde(alias = "surcharge", alias = "Surcharge")]
    pub surcharge: Option<String>,
    #[serde(alias = "merRequestAmt", alias = "MerRequestAmt")]
    pub mer_request_amt: Option<String>,
    #[serde(alias = "terminal", alias = "Terminal")]
    pub terminal: Option<String>,
    #[serde(alias = "bankMid", alias = "BankMid")]
    pub bank_mid: Option<String>,
    #[serde(alias = "settleFlag", alias = "SettleFlag")]
    pub settle_flag: Option<String>,
    #[serde(alias = "bank", alias = "Bank")]
    pub bank: Option<String>,
    #[serde(alias = "bankRef", alias = "BankRef")]
    pub bank_ref: Option<String>,
    #[serde(alias = "traceNo", alias = "TraceNo")]
    pub trace_no: Option<String>,
    #[serde(alias = "accountNo", alias = "AccountNo")]
    pub account_no: Option<String>,
    #[serde(alias = "currency", alias = "Currency")]
    pub currency: Option<String>,
    #[serde(alias = "remark", alias = "Remark")]
    pub remark: Option<String>,
    #[serde(alias = "originalAmt", alias = "OriginalAmt")]
    pub original_amt: Option<String>,
    #[serde(alias = "txTime", alias = "TxTime")]
    pub tx_time: Option<String>,
}

impl AsiapayPSyncResponse {
    pub fn build_connector_metadata(&self) -> Option<serde_json::Value> {
        AsiapayConnectorMetadata::from(self).into_json_value()
    }

    pub fn is_successful(&self) -> bool {
        self.result_code.as_deref() == Some("0") || self.success_code.as_deref() == Some("0")
    }
}

impl TryFrom<ResponseRouterData<AsiapayPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<AsiapayPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;

        // Convert base-unit amount to minor units using the numeric currency code.
        let (amount_captured, minor_amount_captured) = match (&response.amt, &response.cur) {
            (Some(amt_str), Some(cur_code)) => match get_currency_from_asiapay_code(cur_code) {
                Some(currency) => match currency.to_currency_lower_unit(amt_str.clone()) {
                    Ok(lower) => match lower.parse::<i64>() {
                        Ok(minor) => (
                            Some(minor),
                            Some(common_utils::types::MinorUnit::new(minor)),
                        ),
                        Err(e) => {
                            tracing::warn!(
                                "AsiaPay PSync: failed to parse amount '{}' to i64: {}",
                                amt_str,
                                e
                            );
                            (None, None)
                        }
                    },
                    Err(e) => {
                        tracing::warn!("AsiaPay PSync: failed to convert amount '{}' to lower unit for currency {:?}: {:?}", amt_str, currency, e);
                        (None, None)
                    }
                },
                None => {
                    tracing::warn!("AsiaPay PSync: unknown currency code '{}'", cur_code);
                    (None, None)
                }
            },
            _ => (None, None),
        };

        let payments_response_data = if !response.is_successful() {
            PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_ref.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }
        } else {
            PaymentsResponseData::TransactionResponse {
                resource_id: response
                    .pay_ref
                    .as_ref()
                    .cloned()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.build_connector_metadata(),
                network_txn_id: response.auth_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: response.order_ref.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status: if response.is_successful() {
                    response
                        .order_status
                        .as_deref()
                        .map(map_order_status)
                        .unwrap_or(AttemptStatus::Pending)
                } else {
                    AttemptStatus::Failure
                },
                amount_captured,
                minor_amount_captured,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== RSYNC REQUEST =====
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsiapayRSyncRequest {
    pub merchant_id: Secret<String>,
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    pub action_type: String,
    pub pay_ref: String,
    // AsiaPay's Query API does not accept a refund-specific ID.
    // Refund status is queried using the original payRef only.
    // This field is kept for trait compatibility but is intentionally None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for AsiapayRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AsiapayAuthType::try_from(&item.router_data.connector_config)?;
        let req = &item.router_data.request;

        Ok(Self {
            merchant_id: auth.merchant_id,
            login_id: auth.login_id,
            password: auth.password,
            action_type: "Query".to_string(),
            pay_ref: req.connector_transaction_id.to_string(),
            refund_id: None,
        })
    }
}

pub type AsiapayRSyncResponse = AsiapayRefundResponse;

impl TryFrom<ResponseRouterData<AsiapayRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<AsiapayRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;

        let refunds_response_data = if !response.is_successful() {
            RefundsResponseData {
                connector_refund_id: response.pay_ref.clone().unwrap_or_default(),
                refund_status: RefundStatus::Failure,
                status_code: item.http_code,
            }
        } else {
            let refund_status = response
                .order_status
                .as_deref()
                .map(map_refund_status)
                .unwrap_or(RefundStatus::Pending);

            RefundsResponseData {
                connector_refund_id: response.pay_ref.clone().unwrap_or_default(),
                refund_status,
                status_code: item.http_code,
            }
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            resource_common_data: RefundFlowData {
                status: if response.is_successful() {
                    response
                        .order_status
                        .as_deref()
                        .map(map_refund_status)
                        .unwrap_or(RefundStatus::Pending)
                } else {
                    RefundStatus::Failure
                },
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== STATUS MAPPING =====
pub fn map_order_status(status: &str) -> AttemptStatus {
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

pub fn map_refund_status(status: &str) -> RefundStatus {
    match status {
        "Refunded" | "Partial Refunded" | "Voided" | "Cancelled" => RefundStatus::Success,
        "Pending" | "RequestRefund" | "RequestPartialRefund" => RefundStatus::Pending,
        _ => RefundStatus::Failure,
    }
}

// ===== WEBHOOK TYPES =====

/// Compute AsiaPay secure hash for webhook verification.
///
/// Hash input format (pipe-separated):
/// ```
/// {src}|{prc}|{successcode}|{ref}|{payRef}|{cur}|{amt}|{payerAuth}|{secret}
/// ```
pub fn compute_asiapay_webhook_hash(
    body: &AsiapayWebhookBody,
    secret: &Secret<String>,
) -> Result<String, error_stack::Report<ConnectorError>> {
    use common_utils::crypto::GenerateDigest;

    let parts = vec![
        body.src.clone().unwrap_or_default(),
        body.prc.clone().unwrap_or_default(),
        body.success_code.clone().unwrap_or_default(),
        body.order_ref.clone().unwrap_or_default(),
        body.pay_ref.clone().unwrap_or_default(),
        body.cur.clone().unwrap_or_default(),
        body.amt.clone().unwrap_or_default(),
        body.payer_auth.clone().unwrap_or_default(),
        secret.peek().to_string(),
    ];

    let input = parts.join("|");

    let digest = common_utils::crypto::Sha256
        .generate_digest(input.as_bytes())
        .change_context(ConnectorError::ResponseDeserializationFailed {
            context: Default::default(),
        })?;

    Ok(hex::encode(digest).to_lowercase())
}

/// Map AsiaPay webhook success code to EventType.
pub fn map_asiapay_webhook_event_type(
    body: &AsiapayWebhookBody,
) -> Result<EventType, error_stack::Report<ConnectorError>> {
    let success_code = body.success_code.as_deref().unwrap_or("");
    let order_status = body.order_status.as_deref();

    let event_type = match success_code {
        "0" => match order_status {
            Some("Refunded" | "Partial Refunded") => EventType::RefundSuccess,
            Some("Authorized") => EventType::PaymentIntentAuthorizationSuccess,
            _ => EventType::PaymentIntentSuccess,
        },
        "1" => EventType::PaymentIntentFailure,
        "2" => EventType::IncomingWebhookEventUnspecified,
        _ => EventType::IncomingWebhookEventUnspecified,
    };

    Ok(event_type)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AsiapayWebhookBody {
    #[serde(alias = "successCode")]
    pub success_code: Option<String>,
    #[serde(alias = "Ref", alias = "ref")]
    pub order_ref: Option<String>,
    #[serde(alias = "PayRef", alias = "payRef", alias = "payref")]
    pub pay_ref: Option<String>,
    #[serde(alias = "Amt", alias = "amt")]
    pub amt: Option<String>,
    #[serde(alias = "Cur", alias = "cur")]
    pub cur: Option<String>,
    #[serde(alias = "prc", alias = "Prc")]
    pub prc: Option<String>,
    #[serde(alias = "src", alias = "Src")]
    pub src: Option<String>,
    #[serde(alias = "orderStatus", alias = "OrderStatus")]
    pub order_status: Option<String>,
    #[serde(alias = "secureHash", alias = "securehash")]
    pub secure_hash: Option<String>,
    #[serde(alias = "payerAuth", alias = "payerauth")]
    pub payer_auth: Option<String>,
}
