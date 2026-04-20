use std::collections::HashMap;

use common_enums::{
    AttemptStatus as HyperswitchAttemptStatus, CaptureMethod as HyperswitchCaptureMethod, Currency,
    FutureUsage,
};
use common_utils::{
    consts::NO_ERROR_CODE,
    types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId as DomainResponseId,
    },
    payment_address::PaymentAddress,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret, WithoutType};
use serde::{
    de::{self, Deserializer},
    Deserialize, Serialize,
};
use serde_with::skip_serializing_none;

use super::ElavonRouterData;
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

#[derive(Debug, Clone, Serialize)]
pub struct ElavonAuthType {
    pub(super) ssl_merchant_id: Secret<String>,
    pub(super) ssl_user_id: Secret<String>,
    pub(super) ssl_pin: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for ElavonAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Elavon {
                ssl_merchant_id,
                ssl_user_id,
                ssl_pin,
                ..
            } => Ok(Self {
                ssl_merchant_id: ssl_merchant_id.clone(),
                ssl_user_id: ssl_user_id.clone(),
                ssl_pin: ssl_pin.clone(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: Default::default()
            })),
        }
    }
}
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    CcSale,
    CcAuthOnly,
    CcComplete,
    CcReturn,
    TxnQuery,
}

impl Serialize for TransactionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            Self::CcSale => "ccsale",
            Self::CcAuthOnly => "ccauthonly",
            Self::CcComplete => "cccomplete",
            Self::CcReturn => "ccreturn",
            Self::TxnQuery => "txnquery",
        };
        serializer.serialize_str(value)
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct CardPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_amount: StringMajorUnit,
    pub ssl_card_number: RawCardNumber<T>,
    pub ssl_exp_date: Secret<String>,
    pub ssl_cvv2cvc2: Option<Secret<String>>,
    pub ssl_cvv2cvc2_indicator: Option<i32>,
    pub ssl_email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_add_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_token_source: Option<String>,
    pub ssl_get_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_transaction_currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_avs_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_avs_zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_customer_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_invoice_number: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ElavonPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(CardPaymentRequest<T>),
}

fn get_avs_details_from_payment_address(
    payment_address: Option<&PaymentAddress>,
) -> (Option<Secret<String>>, Option<Secret<String>>) {
    payment_address
        .and_then(|addr| {
            addr.get_payment_billing()
                .as_ref()
                .and_then(|billing_api_address| {
                    billing_api_address
                        .address
                        .as_ref()
                        .map(|detailed_address| {
                            (detailed_address.line1.clone(), detailed_address.zip.clone())
                        })
                })
        })
        .unwrap_or((None, None))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ElavonPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ref card) => {
                let router_data = item.router_data.clone();
                let request_data = &router_data.request;
                let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;

                let transaction_type = match request_data.capture_method {
                    Some(HyperswitchCaptureMethod::Manual) => TransactionType::CcAuthOnly,
                    Some(HyperswitchCaptureMethod::Automatic) => TransactionType::CcSale,
                    None => TransactionType::CcSale,
                    Some(other_capture_method) => {
                        Err(report!(IntegrationError::FlowNotSupported {
                            flow: format!("Capture method: {other_capture_method:?}"),
                            connector: "Elavon".to_string(),
                            context: Default::default()
                        }))?
                    }
                };

                let exp_month = card.card_exp_month.peek().to_string();
                let formatted_exp_month = format!("{exp_month:0>2}");

                let exp_year = card.card_exp_year.peek().to_string();
                let formatted_exp_year = if exp_year.len() == 4 {
                    &exp_year[2..]
                } else {
                    &exp_year
                };
                let exp_date = format!("{formatted_exp_month}{formatted_exp_year}");

                let (avs_address, avs_zip) = get_avs_details_from_payment_address(Some(
                    &router_data.resource_common_data.address,
                ));

                let _cvv_indicator = if card.card_cvc.peek().is_empty() {
                    Some(0)
                } else {
                    Some(1)
                };

                let customer_id_str = request_data
                    .customer_id
                    .as_ref()
                    .map(|c| c.get_string_repr().to_string());

                let add_token =
                    request_data
                        .setup_future_usage
                        .as_ref()
                        .and_then(|sfu: &FutureUsage| {
                            if *sfu == FutureUsage::OnSession || *sfu == FutureUsage::OffSession {
                                Some("ADD".to_string())
                            } else {
                                None
                            }
                        });
                let token_source = add_token.as_ref().map(|_| "ECOMMERCE".to_string());

                // Manually convert to StringMajorUnit to avoid error handling issues
                let amount_converter = StringMajorUnitForConnector;
                let amount = amount_converter
                    .convert(request_data.minor_amount, request_data.currency)
                    .map_err(|e| {
                        report!(IntegrationError::AmountConversionFailed {
                            context: Default::default()
                        })
                        .attach_printable(format!("Failed to convert amount: {e}"))
                    })?;
                let card_req = CardPaymentRequest {
                    ssl_transaction_type: transaction_type,
                    ssl_account_id: auth_type.ssl_merchant_id.clone(),
                    ssl_user_id: auth_type.ssl_user_id.clone(),
                    ssl_pin: auth_type.ssl_pin.clone(),
                    ssl_amount: amount,
                    ssl_card_number: card.card_number.clone(),
                    ssl_exp_date: Secret::new(exp_date),
                    ssl_cvv2cvc2: Some(card.card_cvc.clone()),
                    ssl_cvv2cvc2_indicator: Some(1),
                    ssl_email: request_data.email.clone(),
                    ssl_add_token: add_token,
                    ssl_token_source: token_source,
                    ssl_get_token: None,
                    ssl_transaction_currency: request_data
                        .connector_feature_data
                        .as_ref()
                        .and_then(|d| d.peek().get("multi_currency_enabled"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        .then_some(request_data.currency),
                    ssl_avs_address: avs_address,
                    ssl_avs_zip: avs_zip,
                    ssl_customer_code: customer_id_str,
                    ssl_invoice_number: Some(router_data.resource_common_data.payment_id.clone()),
                };
                tracing::debug!(?card_req, "Elavon Card Payment Request");
                Ok(Self::Card(card_req))
            }
            _ => Err(report!(IntegrationError::not_implemented(
                "Only card payments are supported for Elavon".to_string()
            ))),
        }
    }
}

// Define structs to hold the XML request and response formats
#[derive(Debug, Serialize)]
pub struct XMLElavonRequest(pub HashMap<String, Secret<String, WithoutType>>);

// Define a dedicated type for PSync XML requests
#[derive(Debug, Serialize)]
pub struct XMLPSyncRequest(pub HashMap<String, Secret<String, WithoutType>>);

// Define dedicated types for Capture, Refund, and RSync XML requests
#[derive(Debug, Serialize)]
pub struct XMLCaptureRequest(pub HashMap<String, Secret<String, WithoutType>>);

#[derive(Debug, Serialize)]
pub struct XMLRefundRequest(pub HashMap<String, Secret<String, WithoutType>>);

#[derive(Debug, Serialize)]
pub struct XMLRSyncRequest(pub HashMap<String, Secret<String, WithoutType>>);

// TryFrom implementation to convert from the router data to XMLElavonRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XMLElavonRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Instead of using request_body which could cause a recursive call,
        // directly create the ElavonPaymentsRequest from data directly
        let request = ElavonPaymentsRequest::try_from(data)
            .attach_printable("Failed to create ElavonPaymentsRequest from ElavonRouterData")?;

        // Log that we're creating the XML request
        tracing::info!("Creating XML for Elavon request using direct implementation");

        // Generate XML content directly
        let xml_content = quick_xml::se::to_string_with_root("txn", &request).map_err(|err| {
            tracing::info!(error=?err, "XML serialization error");
            error_stack::report!(IntegrationError::RequestEncodingFailed {
                context: Default::default()
            })
        })?;

        // Log generated XML for debugging
        let raw_xml = xml_content.clone();
        tracing::info!(xml=?raw_xml, "Generated raw XML");

        let mut result = HashMap::new();
        result.insert(
            "xmldata".to_string(),
            Secret::<_, WithoutType>::new(xml_content),
        );

        // Log form data keys
        let keys = result.keys().collect::<Vec<_>>();
        tracing::info!(form_keys=?keys, "Form data keys");

        Ok(Self(result))
    }
}

// TryFrom implementation for PSync flow using XMLPSyncRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for XMLPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Direct implementation to avoid recursive calls
        let request = SyncRequest::try_from(&data.router_data)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to create SyncRequest from RouterData")?;

        // Log that we're creating the XML request for PSync
        tracing::info!("Creating XML for Elavon PSync request using direct implementation");

        // Generate XML content directly
        let xml_content = quick_xml::se::to_string_with_root("txn", &request).map_err(|err| {
            tracing::info!(error=?err, "XML serialization error");
            error_stack::report!(IntegrationError::RequestEncodingFailed {
                context: Default::default()
            })
        })?;

        // Log generated XML for debugging
        let raw_xml = xml_content.clone();
        tracing::info!(xml=?raw_xml, "Generated raw XML");

        let mut result = HashMap::new();
        result.insert(
            "xmldata".to_string(),
            Secret::<_, WithoutType>::new(xml_content),
        );

        // Log form data keys
        let keys = result.keys().collect::<Vec<_>>();
        tracing::info!(form_keys=?keys, "PSync XML data map keys");

        Ok(Self(result))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SslResult {
    #[serde(rename = "0")]
    Approved,
    #[serde(rename = "1")]
    Declined,
    Other(String),
}

impl TryFrom<String> for SslResult {
    type Error = ConnectorError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "0" => Ok(Self::Approved),
            "1" => Ok(Self::Declined),
            _ => Ok(Self::Other(value)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResponse {
    pub ssl_result: SslResult,
    pub ssl_txn_id: String,
    pub ssl_result_message: String,
    pub ssl_token: Option<Secret<String>>,
    pub ssl_approval_code: Option<String>,
    pub ssl_transaction_type: Option<String>,
    pub ssl_cvv2_response: Option<Secret<String>>,
    pub ssl_avs_response: Option<String>,
    pub ssl_token_response: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonErrorResponse {
    pub error_code: Option<String>,
    pub error_message: String,
    pub error_name: Option<String>,
    pub ssl_txn_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ElavonResult {
    Success(PaymentResponse),
    Error(ElavonErrorResponse),
}

#[derive(Debug, Clone, Serialize)]
pub struct ElavonPaymentsResponse {
    pub result: ElavonResult,
}

// Create distinct response types for Capture and Refund to avoid templating conflicts
#[derive(Debug, Clone, Serialize)]
pub struct ElavonCaptureResponse {
    pub result: ElavonResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElavonRefundResponse {
    pub result: ElavonResult,
}

// Implement the same deserialization logic for all response types
impl<'de> Deserialize<'de> for ElavonPaymentsResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        #[serde(rename = "txn")]
        struct XmlIshResponse {
            #[serde(default, rename = "errorCode")]
            error_code: Option<String>,
            #[serde(default, rename = "errorMessage")]
            error_message: Option<String>,
            #[serde(default, rename = "errorName")]
            error_name: Option<String>,
            #[serde(default)]
            ssl_result: Option<String>,
            #[serde(default)]
            ssl_txn_id: Option<String>,
            #[serde(default)]
            ssl_result_message: Option<String>,
            #[serde(default)]
            ssl_token: Option<Secret<String>>,
            #[serde(default)]
            ssl_token_response: Option<Secret<String>>,
            #[serde(default)]
            ssl_approval_code: Option<String>,
            #[serde(default)]
            ssl_transaction_type: Option<String>,
            #[serde(default)]
            ssl_cvv2_response: Option<Secret<String>>,
            #[serde(default)]
            ssl_avs_response: Option<String>,
        }

        let flat_res = XmlIshResponse::deserialize(deserializer)?;

        let result = {
            if flat_res.ssl_result.as_deref() == Some("0") {
                ElavonResult::Success(PaymentResponse {
                    ssl_result: SslResult::try_from(
                        flat_res
                            .ssl_result
                            .ok_or_else(|| de::Error::missing_field("ssl_result"))?,
                    )
                    .map_err(de::Error::custom)?,
                    ssl_txn_id: flat_res
                        .ssl_txn_id
                        .ok_or_else(|| de::Error::missing_field("ssl_txn_id"))?,
                    ssl_result_message: flat_res
                        .ssl_result_message
                        .ok_or_else(|| de::Error::missing_field("ssl_result_message"))?,
                    ssl_token: flat_res.ssl_token,
                    ssl_approval_code: flat_res.ssl_approval_code,
                    ssl_transaction_type: flat_res.ssl_transaction_type.clone(),
                    ssl_cvv2_response: flat_res.ssl_cvv2_response,
                    ssl_avs_response: flat_res.ssl_avs_response,
                    ssl_token_response: flat_res.ssl_token_response.map(|s| s.expose()),
                })
            } else if flat_res.error_message.is_some() {
                ElavonResult::Error(ElavonErrorResponse {
                    error_code: flat_res.error_code.or(flat_res.ssl_result.clone()),
                    error_message: flat_res
                        .error_message
                        .ok_or_else(|| de::Error::missing_field("error_message"))?,
                    error_name: flat_res.error_name,
                    ssl_txn_id: flat_res.ssl_txn_id,
                })
            } else if flat_res.ssl_result.is_some() {
                ElavonResult::Error(ElavonErrorResponse {
                    error_code: flat_res.ssl_result.clone(),
                    error_message: flat_res
                        .ssl_result_message
                        .unwrap_or_else(|| "Transaction resulted in an error".to_string()),
                    error_name: None,
                    ssl_txn_id: flat_res.ssl_txn_id,
                })
            } else {
                return Err(de::Error::custom(
                    "Invalid Response from Elavon - cannot determine success or error state, missing critical fields.",
                ));
            }
        };
        Ok(Self { result })
    }
}

impl<'de> Deserialize<'de> for ElavonCaptureResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        #[serde(rename = "txn")]
        struct XmlIshResponse {
            #[serde(default, rename = "errorCode")]
            error_code: Option<String>,
            #[serde(default, rename = "errorMessage")]
            error_message: Option<String>,
            #[serde(default, rename = "errorName")]
            error_name: Option<String>,
            #[serde(default)]
            ssl_result: Option<String>,
            #[serde(default)]
            ssl_txn_id: Option<String>,
            #[serde(default)]
            ssl_result_message: Option<String>,
            #[serde(default)]
            ssl_token: Option<Secret<String>>,
            #[serde(default)]
            ssl_token_response: Option<Secret<String>>,
            #[serde(default)]
            ssl_approval_code: Option<String>,
            #[serde(default)]
            ssl_transaction_type: Option<String>,
            #[serde(default)]
            ssl_cvv2_response: Option<Secret<String>>,
            #[serde(default)]
            ssl_avs_response: Option<String>,
        }

        let flat_res = XmlIshResponse::deserialize(deserializer)?;

        let result = {
            if flat_res.ssl_result.as_deref() == Some("0") {
                ElavonResult::Success(PaymentResponse {
                    ssl_result: SslResult::try_from(
                        flat_res
                            .ssl_result
                            .ok_or_else(|| de::Error::missing_field("ssl_result"))?,
                    )
                    .map_err(de::Error::custom)?,
                    ssl_txn_id: flat_res
                        .ssl_txn_id
                        .ok_or_else(|| de::Error::missing_field("ssl_txn_id"))?,
                    ssl_result_message: flat_res
                        .ssl_result_message
                        .ok_or_else(|| de::Error::missing_field("ssl_result_message"))?,
                    ssl_token: flat_res.ssl_token,
                    ssl_approval_code: flat_res.ssl_approval_code,
                    ssl_transaction_type: flat_res.ssl_transaction_type.clone(),
                    ssl_cvv2_response: flat_res.ssl_cvv2_response,
                    ssl_avs_response: flat_res.ssl_avs_response,
                    ssl_token_response: flat_res.ssl_token_response.map(|s| s.expose()),
                })
            } else if flat_res.error_message.is_some() {
                ElavonResult::Error(ElavonErrorResponse {
                    error_code: flat_res.error_code.or(flat_res.ssl_result.clone()),
                    error_message: flat_res
                        .error_message
                        .ok_or_else(|| de::Error::missing_field("error_message"))?,
                    error_name: flat_res.error_name,
                    ssl_txn_id: flat_res.ssl_txn_id,
                })
            } else if flat_res.ssl_result.is_some() {
                ElavonResult::Error(ElavonErrorResponse {
                    error_code: flat_res.ssl_result.clone(),
                    error_message: flat_res
                        .ssl_result_message
                        .unwrap_or_else(|| "Transaction resulted in an error".to_string()),
                    error_name: None,
                    ssl_txn_id: flat_res.ssl_txn_id,
                })
            } else {
                return Err(de::Error::custom(
                    "Invalid Response from Elavon - cannot determine success or error state, missing critical fields.",
                ));
            }
        };
        Ok(Self { result })
    }
}

impl<'de> Deserialize<'de> for ElavonRefundResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        #[serde(rename = "txn")]
        struct XmlIshResponse {
            #[serde(default, rename = "errorCode")]
            error_code: Option<String>,
            #[serde(default, rename = "errorMessage")]
            error_message: Option<String>,
            #[serde(default, rename = "errorName")]
            error_name: Option<String>,
            #[serde(default)]
            ssl_result: Option<String>,
            #[serde(default)]
            ssl_txn_id: Option<String>,
            #[serde(default)]
            ssl_result_message: Option<String>,
            #[serde(default)]
            ssl_token: Option<Secret<String>>,
            #[serde(default)]
            ssl_token_response: Option<Secret<String>>,
            #[serde(default)]
            ssl_approval_code: Option<String>,
            #[serde(default)]
            ssl_transaction_type: Option<String>,
            #[serde(default)]
            ssl_cvv2_response: Option<Secret<String>>,
            #[serde(default)]
            ssl_avs_response: Option<String>,
        }

        let flat_res = XmlIshResponse::deserialize(deserializer)?;

        let result = {
            if flat_res.ssl_result.as_deref() == Some("0") {
                ElavonResult::Success(PaymentResponse {
                    ssl_result: SslResult::try_from(
                        flat_res
                            .ssl_result
                            .ok_or_else(|| de::Error::missing_field("ssl_result"))?,
                    )
                    .map_err(de::Error::custom)?,
                    ssl_txn_id: flat_res
                        .ssl_txn_id
                        .ok_or_else(|| de::Error::missing_field("ssl_txn_id"))?,
                    ssl_result_message: flat_res
                        .ssl_result_message
                        .ok_or_else(|| de::Error::missing_field("ssl_result_message"))?,
                    ssl_token: flat_res.ssl_token,
                    ssl_approval_code: flat_res.ssl_approval_code,
                    ssl_transaction_type: flat_res.ssl_transaction_type.clone(),
                    ssl_cvv2_response: flat_res.ssl_cvv2_response,
                    ssl_avs_response: flat_res.ssl_avs_response,
                    ssl_token_response: flat_res.ssl_token_response.map(|s| s.expose()),
                })
            } else if flat_res.error_message.is_some() {
                ElavonResult::Error(ElavonErrorResponse {
                    error_code: flat_res.error_code.or(flat_res.ssl_result.clone()),
                    error_message: flat_res
                        .error_message
                        .ok_or_else(|| de::Error::missing_field("error_message"))?,
                    error_name: flat_res.error_name,
                    ssl_txn_id: flat_res.ssl_txn_id,
                })
            } else if flat_res.ssl_result.is_some() {
                ElavonResult::Error(ElavonErrorResponse {
                    error_code: flat_res.ssl_result.clone(),
                    error_message: flat_res
                        .ssl_result_message
                        .unwrap_or_else(|| "Transaction resulted in an error".to_string()),
                    error_name: None,
                    ssl_txn_id: flat_res.ssl_txn_id,
                })
            } else {
                return Err(de::Error::custom(
                    "Invalid Response from Elavon - cannot determine success or error state, missing critical fields.",
                ));
            }
        };
        Ok(Self { result })
    }
}

pub fn get_elavon_attempt_status(
    elavon_result: &ElavonResult,
    http_code: u16,
) -> (HyperswitchAttemptStatus, Option<ErrorResponse>) {
    match elavon_result {
        ElavonResult::Success(payment_response) => {
            let status = match payment_response.ssl_transaction_type.as_deref() {
                Some("ccauthonly") | Some("AUTHONLY") => HyperswitchAttemptStatus::Authorized,
                Some("ccsale") | Some("cccomplete") | Some("SALE") | Some("COMPLETE") => {
                    HyperswitchAttemptStatus::Charged
                }
                _ => match payment_response.ssl_result {
                    SslResult::Approved => HyperswitchAttemptStatus::Charged,
                    _ => HyperswitchAttemptStatus::Failure,
                },
            };
            (status, None)
        }
        ElavonResult::Error(error_resp) => (
            HyperswitchAttemptStatus::Failure,
            Some(ErrorResponse {
                status_code: http_code,
                code: error_resp
                    .error_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: error_resp.error_message.clone(),
                reason: error_resp.error_name.clone(),
                attempt_status: Some(HyperswitchAttemptStatus::Failure),
                connector_transaction_id: error_resp.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        ),
    }
}

impl<
        F,
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<ElavonPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<ElavonPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // Log the response for debugging
        tracing::info!(response=?response, "Processing Elavon response");

        let (attempt_status, error_response) =
            get_elavon_attempt_status(&response.result, http_code);

        let payments_response_data = match (&response.result, error_response) {
            (ElavonResult::Success(payment_resp_struct), None) => {
                let ssl_token =
                    if payment_resp_struct.ssl_token_response.as_deref() == Some("SUCCESS") {
                        payment_resp_struct
                            .ssl_token
                            .as_ref()
                            .map(|t| t.peek().to_string())
                    } else {
                        None
                    };
                let mandate_reference = ssl_token.map(|token| {
                    Box::new(MandateReference {
                        connector_mandate_id: None,
                        payment_method_id: Some(token),
                        connector_mandate_request_reference_id: None,
                    })
                });
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: DomainResponseId::ConnectorTransactionId(
                        payment_resp_struct.ssl_txn_id.clone(),
                    ),
                    redirection_data: None,
                    connector_metadata: None,
                    network_txn_id: payment_resp_struct.ssl_approval_code.clone(),
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    mandate_reference,
                    status_code: http_code,
                })
            }
            (_, Some(err_resp)) => Err(err_resp),
            (ElavonResult::Error(error_payload), None) => Err(ErrorResponse {
                status_code: http_code,
                code: error_payload
                    .error_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: error_payload.error_message.clone(),
                reason: error_payload.error_name.clone(),
                attempt_status: Some(HyperswitchAttemptStatus::Failure),
                connector_transaction_id: error_payload.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        Ok(Self {
            response: payments_response_data,
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename = "txn")]
pub struct SyncRequest {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_txn_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for SyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for SyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        router_data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let request_data = &router_data.request;
        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;

        let connector_txn_id = match &request_data.connector_transaction_id {
            DomainResponseId::ConnectorTransactionId(id) => id.clone(),

            _ => {
                return Err(report!(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default()
                }))
                .attach_printable("Missing connector_transaction_id for Elavon PSync")
            }
        };

        Ok(Self {
            ssl_transaction_type: TransactionType::TxnQuery,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_txn_id: connector_txn_id,
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct ElavonCaptureRequest {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_amount: StringMajorUnit,
    pub ssl_txn_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for ElavonCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;

        let previous_connector_txn_id = match &router_data.request.connector_transaction_id {
            DomainResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(report!(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default()
                }))
            }
        };

        // Convert amount for capture
        let amount_converter = StringMajorUnitForConnector;
        let amount = amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .map_err(|_| IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            ssl_transaction_type: TransactionType::CcComplete,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_amount: amount,
            ssl_txn_id: previous_connector_txn_id,
        })
    }
}

// Implementation for XMLCaptureRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for XMLCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Create the ElavonCaptureRequest
        let request = ElavonCaptureRequest::try_from(data)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to create ElavonCaptureRequest")?;

        // Generate XML content
        let xml_content = quick_xml::se::to_string_with_root("txn", &request).map_err(|_| {
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            }
        })?;

        // Create the form data HashMap
        let mut result = HashMap::new();
        result.insert(
            "xmldata".to_string(),
            Secret::<_, WithoutType>::new(xml_content),
        );

        Ok(Self(result))
    }
}

// Response handling for Capture flow
impl<F> TryFrom<ResponseRouterData<ElavonCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<ElavonCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let (attempt_status, error_response) =
            get_elavon_attempt_status(&response.result, http_code);

        // Determine final status based on the transaction type
        let final_status = match &response.result {
            ElavonResult::Success(success_payload) => {
                match success_payload.ssl_transaction_type.as_deref() {
                    Some("cccomplete") | Some("ccsale") => match success_payload.ssl_result {
                        SslResult::Approved => HyperswitchAttemptStatus::Charged,
                        _ => HyperswitchAttemptStatus::Failure,
                    },
                    _ => attempt_status,
                }
            }
            _ => attempt_status,
        };

        // Build the response data
        let response_data = match (&response.result, error_response) {
            (ElavonResult::Success(payment_resp_struct), None) => {
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: DomainResponseId::ConnectorTransactionId(
                        payment_resp_struct.ssl_txn_id.clone(),
                    ),
                    redirection_data: None,
                    connector_metadata: Some(
                        serde_json::to_value(payment_resp_struct.clone())
                            .unwrap_or(serde_json::Value::Null),
                    ),
                    network_txn_id: None,
                    connector_response_reference_id: payment_resp_struct.ssl_approval_code.clone(),
                    incremental_authorization_allowed: None,
                    mandate_reference: None,
                    status_code: http_code,
                })
            }
            (_, Some(err_resp)) => Err(err_resp),
            (ElavonResult::Error(error_payload), None) => Err(ErrorResponse {
                status_code: http_code,
                code: error_payload
                    .error_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: error_payload.error_message.clone(),
                reason: error_payload.error_name.clone(),
                attempt_status: Some(HyperswitchAttemptStatus::Failure),
                connector_transaction_id: error_payload.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status: final_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// Implementation for Refund
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct ElavonRefundRequest {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_amount: StringMajorUnit,
    pub ssl_txn_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for ElavonRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let request_data = &router_data.request;
        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;

        // Convert amount for refund
        let amount_converter = StringMajorUnitForConnector;
        let amount = amount_converter
            .convert(request_data.minor_refund_amount, request_data.currency)
            .map_err(|_| IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            ssl_transaction_type: TransactionType::CcReturn,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_amount: amount,
            ssl_txn_id: request_data.connector_transaction_id.clone(),
        })
    }
}

// Implementation for XMLRefundRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for XMLRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Create the ElavonRefundRequest
        let request = ElavonRefundRequest::try_from(data)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to create ElavonRefundRequest")?;

        // Generate XML content
        let xml_content = quick_xml::se::to_string_with_root("txn", &request).map_err(|_| {
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            }
        })?;

        // Create the form data HashMap
        let mut result = HashMap::new();
        result.insert(
            "xmldata".to_string(),
            Secret::<_, WithoutType>::new(xml_content),
        );

        Ok(Self(result))
    }
}

// Response handling for Refund flow
impl<F> TryFrom<ResponseRouterData<ElavonRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<ElavonRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let (attempt_status, error_response) =
            get_elavon_attempt_status(&response.result, http_code);

        // Determine refund status
        let refund_status = match &response.result {
            ElavonResult::Success(success_payload) => {
                match success_payload.ssl_transaction_type.as_deref() {
                    Some("RETURN") => match success_payload.ssl_result {
                        SslResult::Approved => common_enums::RefundStatus::Success,
                        SslResult::Declined => common_enums::RefundStatus::Failure,
                        SslResult::Other(_) => common_enums::RefundStatus::Pending,
                    },
                    _ => common_enums::RefundStatus::Pending,
                }
            }
            _ => common_enums::RefundStatus::Failure,
        };

        // Build the response data
        let response_data = match (&response.result, error_response) {
            (ElavonResult::Success(payment_resp_struct), None) => Ok(RefundsResponseData {
                connector_refund_id: payment_resp_struct.ssl_txn_id.clone(),
                refund_status,
                status_code: http_code,
            }),
            (_, Some(err_resp)) => Err(err_resp),
            (ElavonResult::Error(error_payload), None) => Err(ErrorResponse {
                status_code: http_code,
                code: error_payload
                    .error_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: error_payload.error_message.clone(),
                reason: error_payload.error_name.clone(),
                attempt_status: Some(attempt_status),
                connector_transaction_id: error_payload.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        Ok(Self {
            response: response_data,
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// Implementation for Refund Sync
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for SyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for SyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;
        let connector_refund_id = router_data.request.connector_refund_id.clone();

        Ok(Self {
            ssl_transaction_type: TransactionType::TxnQuery,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_txn_id: connector_refund_id,
        })
    }
}

// Implementation for XMLRSyncRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for XMLRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Create the SyncRequest
        let request = SyncRequest::try_from(data)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to create SyncRequest for RSync")?;

        // Generate XML content
        let xml_content = quick_xml::se::to_string_with_root("txn", &request).map_err(|_| {
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            }
        })?;

        // Create the form data HashMap
        let mut result = HashMap::new();
        result.insert(
            "xmldata".to_string(),
            Secret::<_, WithoutType>::new(xml_content),
        );

        Ok(Self(result))
    }
}

// Define RSync response type
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ElavonRSyncResponse {
    pub ssl_trans_status: TransactionSyncStatus,
    pub ssl_transaction_type: SyncTransactionType,
    pub ssl_txn_id: String,
}

// Function to determine refund status from RSync response
pub fn get_refund_status_from_elavon_sync_response(
    elavon_response: &ElavonRSyncResponse,
) -> common_enums::RefundStatus {
    match elavon_response.ssl_transaction_type {
        SyncTransactionType::Return => match elavon_response.ssl_trans_status {
            TransactionSyncStatus::STL => common_enums::RefundStatus::Success,
            TransactionSyncStatus::PEN => common_enums::RefundStatus::Pending,
            TransactionSyncStatus::OPN => common_enums::RefundStatus::Pending,
            TransactionSyncStatus::REV => common_enums::RefundStatus::ManualReview,
            TransactionSyncStatus::PST
            | TransactionSyncStatus::FPR
            | TransactionSyncStatus::PRE => common_enums::RefundStatus::Failure,
        },
        _ => common_enums::RefundStatus::Pending,
    }
}

// Response handling for RSync flow
impl<F> TryFrom<ResponseRouterData<ElavonRSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(value: ResponseRouterData<ElavonRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _,
        } = value;

        let refund_status = get_refund_status_from_elavon_sync_response(&response);

        let response_data = RefundsResponseData {
            connector_refund_id: response.ssl_txn_id.clone(),
            refund_status,
            status_code: value.http_code,
        };

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ElavonPSyncResponse {
    pub ssl_trans_status: TransactionSyncStatus,
    pub ssl_transaction_type: SyncTransactionType,
    pub ssl_txn_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TransactionSyncStatus {
    PEN,
    OPN,
    REV,
    STL,
    PST,
    FPR,
    PRE,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SyncTransactionType {
    Sale,
    AuthOnly,
    Return,
}

impl<F> TryFrom<ResponseRouterData<ElavonPSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(value: ResponseRouterData<ElavonPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _,
        } = value;

        let final_status = match response.ssl_trans_status {
            TransactionSyncStatus::STL => match response.ssl_transaction_type {
                SyncTransactionType::Sale => HyperswitchAttemptStatus::Charged,
                SyncTransactionType::AuthOnly => HyperswitchAttemptStatus::Charged,
                SyncTransactionType::Return => HyperswitchAttemptStatus::Pending,
            },
            TransactionSyncStatus::OPN => match response.ssl_transaction_type {
                SyncTransactionType::AuthOnly => HyperswitchAttemptStatus::Authorized,
                SyncTransactionType::Sale => HyperswitchAttemptStatus::Pending,
                SyncTransactionType::Return => HyperswitchAttemptStatus::Pending,
            },
            TransactionSyncStatus::PEN | TransactionSyncStatus::REV => {
                HyperswitchAttemptStatus::Pending
            }
            TransactionSyncStatus::PST
            | TransactionSyncStatus::FPR
            | TransactionSyncStatus::PRE => {
                if response.ssl_transaction_type == SyncTransactionType::AuthOnly
                    && response.ssl_trans_status == TransactionSyncStatus::PRE
                {
                    HyperswitchAttemptStatus::AuthenticationFailed
                } else {
                    HyperswitchAttemptStatus::Failure
                }
            }
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: DomainResponseId::ConnectorTransactionId(response.ssl_txn_id.clone()),
            redirection_data: None,
            connector_metadata: Some(serde_json::json!(response)),
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            mandate_reference: None,
            status_code: value.http_code,
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status: final_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}
