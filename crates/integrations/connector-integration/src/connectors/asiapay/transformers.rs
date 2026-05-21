use std::fmt::Debug;
use std::marker::{Send, Sync};

use common_enums::{AttemptStatus, CardNetwork, Currency, RefundStatus};
use domain_types::{
    connector_flow::*,
    connector_types::*,
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_data::ErrorResponse,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

use super::AsiapayRouterData;

// ============================================================================
// AUTHENTICATION
// ============================================================================

#[derive(Debug)]
pub struct AsiapayAuthType {
    pub merchant_id: Secret<String>,
    pub secure_hash_secret: Secret<String>,
    pub login_id: Secret<String>,
    pub password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AsiapayAuthType {
    type Error = Report<IntegrationError>;

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

// ============================================================================
// CURRENCY CODE HELPER
// ============================================================================

fn get_asiapay_currency_code(currency: Currency) -> Result<String, Report<IntegrationError>> {
    let code = match currency {
        Currency::USD => "840",
        Currency::HKD => "344",
        Currency::SGD => "702",
        Currency::CNY => "156",
        Currency::JPY => "392",
        Currency::TWD => "901",
        Currency::AUD => "036",
        Currency::EUR => "978",
        Currency::GBP => "826",
        Currency::CAD => "124",
        Currency::MYR => "458",
        Currency::THB => "764",
        Currency::PHP => "608",
        Currency::IDR => "360",
        Currency::INR => "356",
        Currency::VND => "704",
        Currency::NZD => "554",
        _ => {
            return Err(IntegrationError::NotSupported {
                message: format!("Currency {:?} is not supported by AsiaPay", currency),
                connector: "Asiapay",
                context: Default::default(),
            }
            .into())
        }
    };
    Ok(code.to_string())
}

// ============================================================================
// PAY TYPE ENUM
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub enum AsiapayPayType {
    #[serde(rename = "VISA")]
    Visa,
    #[serde(rename = "Master")]
    Master,
    #[serde(rename = "AMEX")]
    Amex,
    #[serde(rename = "JCB")]
    Jcb,
    #[serde(rename = "DINERS")]
    Diners,
    #[serde(rename = "UnionPay")]
    UnionPay,
}

impl AsiapayPayType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Visa => "VISA",
            Self::Master => "Master",
            Self::Amex => "AMEX",
            Self::Jcb => "JCB",
            Self::Diners => "DINERS",
            Self::UnionPay => "UnionPay",
        }
    }
}

impl TryFrom<Option<CardNetwork>> for AsiapayPayType {
    type Error = Report<IntegrationError>;

    fn try_from(network: Option<CardNetwork>) -> Result<Self, Self::Error> {
        match network {
            Some(CardNetwork::Visa) => Ok(Self::Visa),
            Some(CardNetwork::Mastercard) => Ok(Self::Master),
            Some(CardNetwork::AmericanExpress) => Ok(Self::Amex),
            Some(CardNetwork::JCB) => Ok(Self::Jcb),
            Some(CardNetwork::DinersClub) => Ok(Self::Diners),
            Some(CardNetwork::UnionPay) => Ok(Self::UnionPay),
            _ => Err(IntegrationError::NotSupported {
                message: "Card network is not supported by AsiaPay. Supported: Visa, Mastercard, AmericanExpress, JCB, DinersClub, UnionPay".to_string(),
                connector: "Asiapay",
                context: Default::default(),
            }
            .into()),
        }
    }
}

// ============================================================================
// CREDIT CARD DATA
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AsiapayCreditCardData {
    #[serde(rename = "payType")]
    pub pay_type: AsiapayPayType,
    #[serde(rename = "cardNo")]
    pub card_no: Secret<String>,
    #[serde(rename = "cardHolder")]
    pub card_holder: Secret<String>,
    #[serde(rename = "expireMonth")]
    pub expire_month: Secret<String>,
    #[serde(rename = "expireYear")]
    pub expire_year: Secret<String>,
    pub cvv2: Secret<String>,
}

// ============================================================================
// PAYMENT INFORMATION ENUM
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AsiapayPaymentInformation {
    Credit(Box<AsiapayCreditCardData>),
}

// ============================================================================
// SHA1 HASH HELPER
// ============================================================================

fn compute_sha1_hex(data: &str) -> String {
    use ring::digest;
    let hash = digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, data.as_bytes());
    hex::encode(hash.as_ref())
}

// ============================================================================
// AUTHORIZE FLOW - REQUEST
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AsiapayPaymentRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "orderRef")]
    pub order_ref: String,
    #[serde(rename = "currCode")]
    pub curr_code: String,
    pub amount: String,
    #[serde(flatten)]
    pub payment_info: AsiapayPaymentInformation,
    #[serde(rename = "successUrl")]
    pub success_url: String,
    #[serde(rename = "failUrl")]
    pub fail_url: String,
    #[serde(rename = "errorUrl")]
    pub error_url: String,
    #[serde(rename = "secureHash")]
    pub secure_hash: Secret<String>,
    #[serde(rename = "lang", skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "mobileNo", skip_serializing_if = "Option::is_none")]
    pub mobile_no: Option<String>,
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
    > for AsiapayPaymentRequest
{
    type Error = Report<IntegrationError>;

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
        let router_data = item.router_data;
        let connector = item.connector;

        let auth =
            AsiapayAuthType::try_from(&router_data.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;

        let merchant_id = auth.merchant_id.peek().to_string();

        let order_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .chars()
            .take(20)
            .collect::<String>();

        let currency = router_data.request.currency;
        let curr_code = get_asiapay_currency_code(currency)?;

        let float_amount = connector
            .amount_converter
            .convert(router_data.request.minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let amount = if currency.is_zero_decimal_currency() {
            format!("{:.0}", float_amount.0)
        } else {
            format!("{:.2}", float_amount.0)
        };

        let return_url = router_data
            .request
            .router_return_url
            .as_ref()
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            })?
            .clone();

        let card_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Only card payments are supported by AsiaPay".to_string(),
                    connector: "Asiapay",
                    context: Default::default(),
                }
                .into())
            }
        };

        let pay_type = AsiapayPayType::try_from(card_data.card_network.clone())?;
        let pay_type_str = pay_type.as_str().to_string();

        let card_holder = card_data
            .card_holder_name
            .clone()
            .unwrap_or_else(|| Secret::new("Card Holder".to_string()));

        let credit_card_data = AsiapayCreditCardData {
            pay_type,
            card_no: Secret::new(card_data.card_number.peek().to_string()),
            card_holder,
            expire_month: card_data.card_exp_month.clone(),
            expire_year: card_data.card_exp_year.clone(),
            cvv2: card_data.card_cvc.clone(),
        };

        let payment_info = AsiapayPaymentInformation::Credit(Box::new(credit_card_data));

        // Compute SHA1 secure hash
        let hash_input = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            merchant_id,
            order_ref,
            curr_code,
            amount,
            pay_type_str,
            return_url,
            return_url,
            return_url,
            auth.secure_hash_secret.peek()
        );
        let secure_hash = Secret::new(compute_sha1_hex(&hash_input));

        let email = router_data
            .request
            .email
            .as_ref()
            .map(|e| e.peek().to_string());

        Ok(Self {
            merchant_id,
            order_ref,
            curr_code,
            amount,
            payment_info,
            success_url: return_url.clone(),
            fail_url: return_url.clone(),
            error_url: return_url,
            secure_hash,
            lang: Some("E".to_string()),
            remark: None,
            email,
            mobile_no: None,
        })
    }
}

// ============================================================================
// AUTHORIZE FLOW - RESPONSE
// ============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct AsiapayPaymentResponse {
    pub prc: String,
    pub src: Option<String>,
    #[serde(rename = "Ref")]
    pub ref_: Option<String>,
    #[serde(rename = "PayRef")]
    pub pay_ref: Option<String>,
    pub successcode: String,
    #[serde(rename = "Ord")]
    pub ord: Option<String>,
    #[serde(rename = "Amt")]
    pub amt: Option<String>,
    #[serde(rename = "Cur")]
    pub cur: Option<String>,
    #[serde(rename = "payerAuth")]
    pub payer_auth: Option<String>,
    #[serde(rename = "secureHash")]
    pub secure_hash: Option<String>,
    #[serde(rename = "PayType")]
    pub pay_type: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<AsiapayPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiapayPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_success = item.response.prc == "0" && item.response.successcode == "0";

        if is_success {
            let is_auto_capture = item.router_data.request.is_auto_capture();
            let status = if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            };

            let connector_transaction_id = item
                .response
                .ref_
                .or(item.response.pay_ref)
                .unwrap_or_default();

            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        connector_transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: item.response.ord,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            let src = item.response.src.clone();
            let prc = item.response.prc.clone();
            let err_msg = item.response.err_msg.clone();

            Ok(Self {
                response: Err(ErrorResponse {
                    code: src.clone().unwrap_or_else(|| prc.clone()),
                    message: err_msg.clone().unwrap_or_else(|| prc.clone()),
                    reason: err_msg,
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: item.response.ref_.or(item.response.pay_ref),
                    network_decline_code: src,
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

// ============================================================================
// CAPTURE FLOW
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AsiapayCaptureRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    #[serde(rename = "orderRef")]
    pub order_ref: String,
    pub func: String,
    #[serde(rename = "currCode")]
    pub curr_code: String,
    pub amount: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AsiapayCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth =
            AsiapayAuthType::try_from(&router_data.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;

        let order_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .chars()
            .take(20)
            .collect::<String>();

        let currency = router_data.request.currency;
        let curr_code = get_asiapay_currency_code(currency)?;

        let float_amount = connector
            .amount_converter
            .convert(router_data.request.minor_amount_to_capture, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let amount = if currency.is_zero_decimal_currency() {
            format!("{:.0}", float_amount.0)
        } else {
            format!("{:.2}", float_amount.0)
        };

        Ok(Self {
            merchant_id: auth.merchant_id.peek().to_string(),
            login_id: auth.login_id,
            password: auth.password,
            order_ref,
            func: "capAuth".to_string(),
            curr_code,
            amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AsiapayCaptureResponse {
    pub successcode: String,
    #[serde(rename = "Ref")]
    pub ref_: Option<String>,
    #[serde(rename = "Ord")]
    pub ord: Option<String>,
    pub prc: Option<String>,
    pub src: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
}

impl TryFrom<ResponseRouterData<AsiapayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiapayCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.successcode == "0" {
            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.ref_.clone().unwrap_or_default(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: item.response.ord,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Charged,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            let err_msg = item.response.err_msg.clone();
            let code = item
                .response
                .src
                .clone()
                .or(item.response.prc.clone())
                .unwrap_or_else(|| item.response.successcode.clone());
            Ok(Self {
                response: Err(ErrorResponse {
                    code,
                    message: err_msg.clone().unwrap_or_default(),
                    reason: err_msg,
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: item.response.ref_,
                    network_decline_code: item.response.src,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// VOID FLOW
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AsiapayVoidRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    #[serde(rename = "orderRef")]
    pub order_ref: String,
    pub func: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AsiapayVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let auth =
            AsiapayAuthType::try_from(&router_data.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;

        let order_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .chars()
            .take(20)
            .collect::<String>();

        Ok(Self {
            merchant_id: auth.merchant_id.peek().to_string(),
            login_id: auth.login_id,
            password: auth.password,
            order_ref,
            func: "void".to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AsiapayVoidResponse {
    pub successcode: String,
    #[serde(rename = "Ref")]
    pub ref_: Option<String>,
    #[serde(rename = "Ord")]
    pub ord: Option<String>,
    pub prc: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
}

impl TryFrom<ResponseRouterData<AsiapayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiapayVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.successcode == "0" {
            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.ref_.clone().unwrap_or_default(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: item.response.ord,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Voided,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            let err_msg = item.response.err_msg.clone();
            let code = item
                .response
                .prc
                .clone()
                .unwrap_or_else(|| item.response.successcode.clone());
            Ok(Self {
                response: Err(ErrorResponse {
                    code,
                    message: err_msg.clone().unwrap_or_default(),
                    reason: err_msg,
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: item.response.ref_,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// REFUND FLOW
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AsiapayRefundRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    #[serde(rename = "orderRef")]
    pub order_ref: String,
    pub func: String,
    #[serde(rename = "refundAmount", skip_serializing_if = "Option::is_none")]
    pub refund_amount: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AsiapayRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth =
            AsiapayAuthType::try_from(&router_data.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;

        let order_ref = router_data.request.connector_transaction_id.clone();

        let currency = router_data.request.currency;
        let float_amount = connector
            .amount_converter
            .convert(router_data.request.minor_refund_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let refund_amount = if currency.is_zero_decimal_currency() {
            format!("{:.0}", float_amount.0)
        } else {
            format!("{:.2}", float_amount.0)
        };

        Ok(Self {
            merchant_id: auth.merchant_id.peek().to_string(),
            login_id: auth.login_id,
            password: auth.password,
            order_ref,
            func: "requestRefund".to_string(),
            refund_amount: Some(refund_amount),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AsiapayRefundResponse {
    pub successcode: String,
    #[serde(rename = "Ref")]
    pub ref_: Option<String>,
    #[serde(rename = "Ord")]
    pub ord: Option<String>,
    #[serde(rename = "refundAmt")]
    pub refund_amt: Option<String>,
    pub prc: Option<String>,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
}

impl TryFrom<ResponseRouterData<AsiapayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiapayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.successcode == "0" {
            Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: item
                        .response
                        .ref_
                        .clone()
                        .unwrap_or_else(|| item.response.successcode.clone()),
                    refund_status: RefundStatus::Success,
                    status_code: item.http_code,
                }),
                resource_common_data: RefundFlowData {
                    status: RefundStatus::Success,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            let err_msg = item.response.err_msg.clone();
            let code = item
                .response
                .prc
                .clone()
                .unwrap_or_else(|| item.response.successcode.clone());
            Ok(Self {
                response: Err(ErrorResponse {
                    code,
                    message: err_msg.clone().unwrap_or_default(),
                    reason: err_msg,
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: item.response.ref_,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// PSYNC FLOW
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AsiapaySyncRequest {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "loginId")]
    pub login_id: Secret<String>,
    pub password: Secret<String>,
    #[serde(rename = "orderRef")]
    pub order_ref: String,
    pub func: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AsiapayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for AsiapaySyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: AsiapayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let auth =
            AsiapayAuthType::try_from(&router_data.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;

        let order_ref = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
                .into())
            }
        };

        Ok(Self {
            merchant_id: auth.merchant_id.peek().to_string(),
            login_id: auth.login_id,
            password: auth.password,
            order_ref,
            func: "query".to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AsiapaySyncResponse {
    pub successcode: String,
    #[serde(rename = "Ref")]
    pub ref_: Option<String>,
    #[serde(rename = "Ord")]
    pub ord: Option<String>,
    #[serde(rename = "PayRef")]
    pub pay_ref: Option<String>,
    #[serde(rename = "Amt")]
    pub amt: Option<String>,
    #[serde(rename = "Cur")]
    pub cur: Option<String>,
    #[serde(rename = "PayType")]
    pub pay_type: Option<String>,
    pub status: Option<String>,
    pub prc: Option<String>,
    pub src: Option<String>,
}

impl TryFrom<ResponseRouterData<AsiapaySyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AsiapaySyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let connector_status = item.response.status.as_deref();
        let attempt_status = match connector_status {
            Some("accepted") => AttemptStatus::Charged,
            Some("pending") => AttemptStatus::Pending,
            Some("refused") => AttemptStatus::Failure,
            Some("cancelled") => AttemptStatus::Voided,
            _ => {
                if item.response.successcode == "0" {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Failure
                }
            }
        };

        let connector_transaction_id = item
            .response
            .ref_
            .or(item.response.pay_ref)
            .unwrap_or_default();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.ord,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================================
// ERROR RESPONSE
// ============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct AsiapayErrorResponse {
    pub successcode: String,
    #[serde(rename = "errMsg")]
    pub err_msg: Option<String>,
    pub prc: Option<String>,
    pub src: Option<String>,
}
