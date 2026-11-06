use std::collections::HashMap;

use common_enums::{AttemptStatus, AuthorizationStatus, CaptureMethod, Currency, FutureUsage};
use common_utils::{consts::NO_ERROR_CODE, types::StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, IncrementalAuthorization, PSync, RSync, Refund, RepeatPayment,
        SetupMandate, Void, VoidPC,
    },
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId as DomainResponseId, SetupMandateRequestData,
    },
    payment_address::PaymentAddress,
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret, WithoutType};
use serde::{de::Deserializer, Deserialize, Serialize};
use serde_with::skip_serializing_none;

use super::ElavonRouterData;
use crate::types::ResponseRouterData;
use domain_types::{
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    router_data::FlowStatus,
};

#[derive(Debug, Clone, Serialize)]
pub struct ElavonAuthType {
    pub(super) ssl_merchant_id: Secret<String>,
    pub(super) ssl_user_id: Secret<String>,
    pub(super) ssl_pin: Secret<String>,
    pub(super) ssl_vendor_id: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for ElavonAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Elavon {
                ssl_merchant_id,
                ssl_user_id,
                ssl_pin,
                ssl_vendor_id,
                ..
            } => Ok(Self {
                ssl_merchant_id: ssl_merchant_id.clone(),
                ssl_user_id: ssl_user_id.clone(),
                ssl_pin: ssl_pin.clone(),
                ssl_vendor_id: ssl_vendor_id.clone(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Expected ConnectorSpecificConfig::Elavon containing \
                         ssl_merchant_id, ssl_user_id, and ssl_pin"
                            .to_string(),
                    ),
                    ..Default::default()
                },
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
    CcDelete,
    CcVoid,
    CcGetToken,
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
            Self::CcDelete => "ccdelete",
            Self::CcVoid => "ccvoid",
            Self::CcGetToken => "ccgettoken",
            Self::TxnQuery => "txnquery",
        };
        serializer.serialize_str(value)
    }
}

/// `ssl_cit_mit_cof_indicator`. Converge requires it whenever `ssl_token` is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CofIndicator {
    /// Consumer initiated, credential on file (ad hoc)
    #[serde(rename = "C01")]
    ConsumerAdHoc,
    #[serde(rename = "M01")]
    MerchantUnscheduled,
    #[serde(rename = "M02")]
    MerchantStandingOrder,
    #[serde(rename = "M03")]
    MerchantSubscription,
    #[serde(rename = "M04")]
    MerchantInstallment,
    #[serde(rename = "M08")]
    MerchantResubmission,
}

impl From<Option<common_enums::MitCategory>> for CofIndicator {
    fn from(mit_category: Option<common_enums::MitCategory>) -> Self {
        match mit_category {
            None | Some(common_enums::MitCategory::Unscheduled) => Self::MerchantUnscheduled,
            Some(common_enums::MitCategory::Recurring) => Self::MerchantStandingOrder,
            Some(common_enums::MitCategory::Subscription) => Self::MerchantSubscription,
            Some(common_enums::MitCategory::Installment) => Self::MerchantInstallment,
            Some(common_enums::MitCategory::Resubmission) => Self::MerchantResubmission,
        }
    }
}

/// Converge rejects the whole request (`5005 Field Character Limit Exceeded`) when a field is
/// over its documented length. Keep the tail, which holds the unique part of our IDs.
fn fit_to_length(value: &str, max_chars: usize) -> String {
    let skip = value.chars().count().saturating_sub(max_chars);
    value.chars().skip(skip).collect()
}

const INVOICE_NUMBER_MAX_LENGTH: usize = 25;
const CUSTOMER_CODE_MAX_LENGTH: usize = 17;

/// Serializes a request as Converge's `<txn>` XML and wraps it in the `xmldata` form field.
fn to_xmldata<R: Serialize>(
    request: &R,
    flow: &str,
) -> Result<HashMap<String, Secret<String, WithoutType>>, error_stack::Report<IntegrationError>> {
    let xml = quick_xml::se::to_string_with_root("txn", request).map_err(|e| {
        report!(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "Failed to serialize Elavon {flow} request to XML: {e}"
                )),
                ..Default::default()
            },
        })
    })?;
    Ok(HashMap::from([(
        "xmldata".to_string(),
        Secret::<_, WithoutType>::new(xml),
    )]))
}

/// External 3DS results. Converge runs 3DS itself only through Elavon's browser SDK, so on
/// the XML API the authentication comes from a separate leg and is sent with the payment.
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct ElavonThreeDsFields {
    pub ssl_eci_ind: u8,
    pub ssl_3dsecure_value: Option<Secret<String>>,
    /// First digit of the 3DS message version.
    pub ssl_program_protocol: Option<u64>,
    pub ssl_dir_server_tran_id: Option<String>,
    pub ssl_3ds_server_trans_id: Option<String>,
    pub ssl_3ds_message_version: Option<String>,
    pub ssl_3ds_trans_status: Option<common_enums::TransactionStatus>,
}

/// Converge accepts only 5 (authenticated), 6 (attempted) and 7 (not authenticated). Its
/// API reference maps 02/05 to 5 and 01/06 to 6, matching the schemes: Mastercard 02 and
/// Visa 05 are authenticated, Mastercard 01 and Visa 06 are attempted.
fn get_converge_eci(eci: Option<&str>) -> Result<u8, error_stack::Report<IntegrationError>> {
    match eci.map(|eci| eci.trim().trim_start_matches('0')) {
        Some("2" | "5") => Ok(5),
        Some("1" | "6") => Ok(6),
        None | Some("" | "3" | "7") => Ok(7),
        Some(other) => Err(report!(IntegrationError::InvalidDataFormat {
            field_name: "authentication_data.eci",
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "Elavon accepts ECI 02/05, 01/06 or 00/03/07 (sent as 5, 6 or 7); got {other}"
                )),
                ..Default::default()
            },
        })),
    }
}

fn get_three_ds_fields(
    authentication_data: Option<&AuthenticationData>,
    is_three_ds: bool,
) -> Result<Option<ElavonThreeDsFields>, error_stack::Report<IntegrationError>> {
    let Some(authentication_data) = authentication_data else {
        // A three_ds payment sent without results would go through unauthenticated.
        return if is_three_ds {
            Err(report!(IntegrationError::MissingRequiredField {
                field_name: "authentication_data",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "auth_type is three_ds but the request carries no 3DS authentication \
                         results. On the XML API, Elavon Converge takes the results of an \
                         authentication run elsewhere (cavv, eci, ds_trans_id, \
                         threeds_server_transaction_id, message_version, trans_status)."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }))
        } else {
            Ok(None)
        };
    };
    let message_version = authentication_data.message_version.as_ref();
    Ok(Some(ElavonThreeDsFields {
        ssl_eci_ind: get_converge_eci(authentication_data.eci.as_deref())?,
        ssl_3dsecure_value: authentication_data.cavv.clone(),
        ssl_program_protocol: message_version.map(|version| version.get_major()),
        ssl_dir_server_tran_id: authentication_data.ds_trans_id.clone(),
        ssl_3ds_server_trans_id: authentication_data.threeds_server_transaction_id.clone(),
        ssl_3ds_message_version: message_version.map(|version| version.to_string()),
        ssl_3ds_trans_status: authentication_data.trans_status.clone(),
    }))
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
    pub ssl_vendor_id: Option<Secret<String>>,
    pub ssl_amount: StringMajorUnit,
    pub ssl_card_number: RawCardNumber<T>,
    pub ssl_exp_date: Secret<String>,
    pub ssl_cvv2cvc2: Option<Secret<String>>,
    pub ssl_cvv2cvc2_indicator: Option<i32>,
    pub ssl_first_name: Option<Secret<String>>,
    pub ssl_last_name: Option<Secret<String>>,
    pub ssl_email: Option<common_utils::pii::Email>,
    /// `Y` stores the card in Converge's Card Manager. Any other value is ignored
    /// silently: the payment approves and no token comes back.
    pub ssl_add_token: Option<&'static str>,
    pub ssl_get_token: Option<&'static str>,
    pub ssl_cit_mit_cof_indicator: Option<CofIndicator>,
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
    #[serde(flatten)]
    pub three_ds: Option<ElavonThreeDsFields>,
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

/// `ssl_exp_date` in Converge's MMYY format.
fn get_card_exp_date<T: PaymentMethodDataTypes>(card: &Card<T>) -> Secret<String> {
    let exp_month = card.card_exp_month.peek().to_string();
    let formatted_exp_month = format!("{exp_month:0>2}");

    let exp_year = card.card_exp_year.peek().to_string();
    let formatted_exp_year = if exp_year.len() == 4 {
        &exp_year[2..]
    } else {
        &exp_year
    };
    Secret::new(format!("{formatted_exp_month}{formatted_exp_year}"))
}

/// Billing first/last name, falling back to a split of the card holder name.
fn get_first_and_last_name<T: PaymentMethodDataTypes>(
    common: &PaymentFlowData,
    card: &Card<T>,
) -> (Option<Secret<String>>, Option<Secret<String>>) {
    let holder_name = card
        .card_holder_name
        .as_ref()
        .map(|name| name.peek().trim());
    let first_name = common.get_optional_billing_first_name().or_else(|| {
        holder_name
            .and_then(|name| name.split_whitespace().next())
            .map(|first| Secret::new(first.to_string()))
    });
    let last_name = common.get_optional_billing_last_name().or_else(|| {
        holder_name
            .and_then(|name| name.split_once(char::is_whitespace))
            .map(|(_, last)| Secret::new(last.trim().to_string()))
    });
    (first_name, last_name)
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
                    Some(CaptureMethod::Manual) => TransactionType::CcAuthOnly,
                    Some(CaptureMethod::Automatic) => TransactionType::CcSale,
                    None => TransactionType::CcSale,
                    Some(other_capture_method) => {
                        Err(report!(IntegrationError::FlowNotSupported {
                            flow: format!("Capture method: {other_capture_method:?}"),
                            connector: "Elavon".to_string(),
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Elavon Authorize supports Automatic (ccsale) and \
                                     Manual (ccauthonly) capture methods only"
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        }))?
                    }
                };

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
                    .map(|c| fit_to_length(c.get_string_repr(), CUSTOMER_CODE_MAX_LENGTH));

                let save_card = matches!(
                    request_data.setup_future_usage,
                    Some(FutureUsage::OnSession | FutureUsage::OffSession)
                );
                let save_flag = save_card.then_some("Y");
                // Saving the card, or paying with one saved earlier, is a consumer-initiated
                // credential-on-file payment.
                let cof_indicator = (save_card || request_data.is_stored_credential == Some(true))
                    .then_some(CofIndicator::ConsumerAdHoc);

                let common = &router_data.resource_common_data;
                let (first_name, last_name) = get_first_and_last_name(common, card);

                let amount = item
                    .connector
                    .amount_converter
                    .convert(&request_data.money)
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: IntegrationErrorContext {
                            additional_context: Some(format!(
                                "Failed to convert minor amount {} {} to major unit \
                                 for Elavon Authorize request",
                                request_data.money.amount(),
                                request_data.currency
                            )),
                            ..Default::default()
                        },
                    })?;
                let card_req = CardPaymentRequest {
                    ssl_transaction_type: transaction_type,
                    ssl_account_id: auth_type.ssl_merchant_id.clone(),
                    ssl_user_id: auth_type.ssl_user_id.clone(),
                    ssl_pin: auth_type.ssl_pin.clone(),
                    ssl_vendor_id: auth_type.ssl_vendor_id.clone(),
                    ssl_amount: amount,
                    ssl_card_number: card.card_number.clone(),
                    ssl_exp_date: get_card_exp_date(card),
                    ssl_cvv2cvc2: Some(card.card_cvc.clone()),
                    ssl_cvv2cvc2_indicator: Some(1),
                    ssl_first_name: first_name,
                    ssl_last_name: last_name,
                    ssl_email: request_data.email.clone(),
                    ssl_add_token: save_flag,
                    ssl_get_token: save_flag,
                    ssl_cit_mit_cof_indicator: cof_indicator,
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
                    ssl_invoice_number: Some(fit_to_length(
                        &common.connector_request_reference_id,
                        INVOICE_NUMBER_MAX_LENGTH,
                    )),
                    three_ds: get_three_ds_fields(
                        request_data.authentication_data.as_ref(),
                        common.is_three_ds(),
                    )?,
                };
                tracing::debug!(?card_req, "Elavon Card Payment Request");
                Ok(Self::Card(card_req))
            }
            _ => Err(report!(IntegrationError::NotImplemented(
                "Only card payments are supported for Elavon".to_string(),
                Default::default()
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
pub struct XMLVoidRequest(pub HashMap<String, Secret<String, WithoutType>>);

#[derive(Debug, Serialize)]
pub struct XMLVoidPCRequest(pub HashMap<String, Secret<String, WithoutType>>);

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
        to_xmldata(&request, "Authorize (ccsale/ccauthonly)").map(Self)
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
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to build SyncRequest (txnquery) for Elavon PSync flow".to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("Failed to create SyncRequest from RouterData")?;
        to_xmldata(&request, "PSync (txnquery)").map(Self)
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

// ---------------------------------------------------------------------------
// Shared XML response shape — used by every payment/capture/refund deserialization.
// All fields are optional: `#[serde(default)]` at struct level fills any missing
// field with `None` via the derived `Default`.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename = "txn", default)]
pub struct ElavonXmlResponse {
    #[serde(rename = "errorCode")]
    pub error_code: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "errorName")]
    pub error_name: Option<String>,
    pub ssl_result: Option<String>,
    pub ssl_txn_id: Option<String>,
    pub ssl_result_message: Option<String>,
    pub ssl_token: Option<Secret<String>>,
    pub ssl_token_response: Option<Secret<String>>,
    pub ssl_approval_code: Option<String>,
    pub ssl_transaction_type: Option<String>,
    pub ssl_cvv2_response: Option<Secret<String>>,
    pub ssl_avs_response: Option<String>,
}

/// Convert a flat XML response into a typed `ElavonResult`.
///
/// Three recognised shapes:
/// - `ssl_result == "0"` → success; `ssl_txn_id` and `ssl_result_message` are required.
/// - `error_message` present (any `ssl_result`) → gateway-level error.
/// - `ssl_result` present but not `"0"`, no `error_message` → processor decline.
pub fn xml_response_into_elavon_result<E: serde::de::Error>(
    flat: ElavonXmlResponse,
) -> Result<ElavonResult, E> {
    let ElavonXmlResponse {
        error_code,
        error_message,
        error_name,
        ssl_result,
        ssl_txn_id,
        ssl_result_message,
        ssl_token,
        ssl_token_response,
        ssl_approval_code,
        ssl_transaction_type,
        ssl_cvv2_response,
        ssl_avs_response,
    } = flat;

    match (ssl_result.as_deref(), error_message) {
        (Some("0"), _) => Ok(ElavonResult::Success(PaymentResponse {
            ssl_result: SslResult::Approved,
            ssl_txn_id: ssl_txn_id.ok_or_else(|| E::missing_field("ssl_txn_id"))?,
            ssl_result_message: ssl_result_message
                .ok_or_else(|| E::missing_field("ssl_result_message"))?,
            ssl_token,
            ssl_approval_code,
            ssl_transaction_type,
            ssl_cvv2_response,
            ssl_avs_response,
            ssl_token_response: ssl_token_response.map(|s| s.expose()),
        })),
        (_, Some(error_message)) => Ok(ElavonResult::Error(ElavonErrorResponse {
            error_code: error_code.or(ssl_result),
            error_message,
            error_name,
            ssl_txn_id,
        })),
        (Some(_), None) => Ok(ElavonResult::Error(ElavonErrorResponse {
            error_code: ssl_result,
            error_message: ssl_result_message
                .unwrap_or_else(|| "Transaction resulted in an error".to_string()),
            error_name: None,
            ssl_txn_id,
        })),
        (None, None) => Err(E::custom(
            "Invalid response from Elavon: cannot determine success or error state, \
             missing both ssl_result and error_message",
        )),
    }
}

// Create distinct response types for Capture and Refund to avoid templating conflicts
#[derive(Debug, Clone, Serialize)]
pub struct ElavonCaptureResponse {
    pub result: ElavonResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElavonVoidResponse {
    pub result: ElavonResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElavonRefundResponse {
    pub result: ElavonResult,
}

// Implement deserialization for all response types via the shared helper.
impl<'de> Deserialize<'de> for ElavonPaymentsResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        xml_response_into_elavon_result(ElavonXmlResponse::deserialize(deserializer)?)
            .map(|result| Self { result })
    }
}

impl<'de> Deserialize<'de> for ElavonCaptureResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        xml_response_into_elavon_result(ElavonXmlResponse::deserialize(deserializer)?)
            .map(|result| Self { result })
    }
}

impl<'de> Deserialize<'de> for ElavonVoidResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        xml_response_into_elavon_result(ElavonXmlResponse::deserialize(deserializer)?)
            .map(|result| Self { result })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ElavonVoidPCResponse {
    pub result: ElavonResult,
}

impl<'de> Deserialize<'de> for ElavonVoidPCResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        xml_response_into_elavon_result(ElavonXmlResponse::deserialize(deserializer)?)
            .map(|result| Self { result })
    }
}

impl<'de> Deserialize<'de> for ElavonRefundResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        xml_response_into_elavon_result(ElavonXmlResponse::deserialize(deserializer)?)
            .map(|result| Self { result })
    }
}

pub fn get_elavon_attempt_status(
    elavon_result: &ElavonResult,
    http_code: u16,
) -> (AttemptStatus, Option<ErrorResponse>) {
    match elavon_result {
        ElavonResult::Success(payment_response) => {
            let status = match payment_response.ssl_transaction_type.as_deref() {
                Some("ccauthonly") | Some("AUTHONLY") => AttemptStatus::Authorized,
                Some("ccsale") | Some("cccomplete") | Some("SALE") | Some("COMPLETE") => {
                    AttemptStatus::Charged
                }
                _ => match payment_response.ssl_result {
                    SslResult::Approved => AttemptStatus::Charged,
                    _ => AttemptStatus::Failure,
                },
            };
            (status, None)
        }
        ElavonResult::Error(error_resp) => (
            AttemptStatus::Failure,
            Some(ErrorResponse {
                status_code: http_code,
                code: error_resp
                    .error_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: error_resp.error_message.clone(),
                reason: error_resp.error_name.clone(),
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                connector_transaction_id: error_resp.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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
                        connector_mandate_id: Some(token),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    })
                });
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: DomainResponseId::ConnectorTransactionId(
                        payment_resp_struct.ssl_txn_id.clone(),
                    ),
                    redirection_data: None,
                    connector_metadata: None,
                    // `ssl_approval_code` is the issuer's approval code, not a network
                    // transaction ID; reporting it as one lets it be replayed as an NTID.
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: payment_resp_struct.ssl_approval_code.clone(),
                    // A ccauthonly can be extended by a later ccauthonly on its ssl_txn_id.
                    incremental_authorization_allowed: (attempt_status
                        == AttemptStatus::Authorized)
                        .then_some(true),
                    mandate_reference,
                    status_code: http_code,
                    splits: None,
                    payment_account_reference: None,
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
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                connector_transaction_id: error_payload.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_vendor_id: Option<Secret<String>>,
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
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon PSync (txnquery) requires a ConnectorTransactionId; \
                             NoConnectorTransactionId and ForeignTransactionId are not supported"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }))
                .attach_printable("Missing connector_transaction_id for Elavon PSync")
            }
        };

        Ok(Self {
            ssl_transaction_type: TransactionType::TxnQuery,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_vendor_id: auth_type.ssl_vendor_id,
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
    pub ssl_vendor_id: Option<Secret<String>>,
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
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon Capture (cccomplete) requires the ssl_txn_id from the \
                             preceding Authorize response stored as connector_transaction_id"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }))
            }
        };

        let amount = item
            .connector
            .amount_converter
            .convert(&router_data.request.capture_money)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "Failed to convert capture amount {} {} to major unit for \
                         Elavon Capture (cccomplete) request",
                        router_data.request.capture_money.amount(),
                        router_data.request.currency
                    )),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            ssl_transaction_type: TransactionType::CcComplete,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_vendor_id: auth_type.ssl_vendor_id,
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
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to build ElavonCaptureRequest (cccomplete) for Elavon Capture flow"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("Failed to create ElavonCaptureRequest")?;
        to_xmldata(&request, "Capture (cccomplete)").map(Self)
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
                        SslResult::Approved => AttemptStatus::Charged,
                        _ => AttemptStatus::Failure,
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
                    network_txn_link_id: None,
                    connector_response_reference_id: payment_resp_struct.ssl_approval_code.clone(),
                    incremental_authorization_allowed: None,
                    mandate_reference: None,
                    status_code: http_code,
                    splits: None,
                    payment_account_reference: None,
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
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                connector_transaction_id: error_payload.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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

// Void uses ccdelete: ccvoid rejects an auth-only transaction (error 5040)
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct ElavonVoidRequest {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_vendor_id: Option<Secret<String>>,
    pub ssl_txn_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for ElavonVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            ssl_transaction_type: TransactionType::CcDelete,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_vendor_id: auth_type.ssl_vendor_id,
            ssl_txn_id: router_data.request.connector_transaction_id.clone(),
        })
    }
}

// Implementation for XMLVoidRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for XMLVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = ElavonVoidRequest::try_from(data)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to build ElavonVoidRequest (ccdelete) for Elavon Void flow"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("Failed to create ElavonVoidRequest")?;
        to_xmldata(&request, "Void (ccdelete)").map(Self)
    }
}

// Response handling for Void flow. get_elavon_attempt_status is not used here:
// it maps an approved response of unknown type to Charged.
impl<F> TryFrom<ResponseRouterData<ElavonVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(value: ResponseRouterData<ElavonVoidResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let (status, response_data) = match response.result {
            ElavonResult::Success(payment_response) => (
                AttemptStatus::Voided,
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: DomainResponseId::ConnectorTransactionId(
                        payment_response.ssl_txn_id,
                    ),
                    redirection_data: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: payment_response.ssl_approval_code,
                    incremental_authorization_allowed: None,
                    mandate_reference: None,
                    status_code: http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
            ),
            ElavonResult::Error(error_payload) => (
                AttemptStatus::VoidFailed,
                Err(ErrorResponse {
                    status_code: http_code,
                    code: error_payload
                        .error_code
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: error_payload.error_message,
                    reason: error_payload.error_name,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                    connector_transaction_id: error_payload.ssl_txn_id,
                    ..Default::default()
                }),
            ),
        };

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// VoidPC uses ccvoid: removes a captured sale from the open batch before settlement
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct ElavonVoidPCRequest {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_vendor_id: Option<Secret<String>>,
    pub ssl_txn_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ElavonVoidPCRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            ssl_transaction_type: TransactionType::CcVoid,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_vendor_id: auth_type.ssl_vendor_id,
            ssl_txn_id: router_data.request.connector_transaction_id.clone(),
        })
    }
}

// Implementation for XMLVoidPCRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XMLVoidPCRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = ElavonVoidPCRequest::try_from(data)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to build ElavonVoidPCRequest (ccvoid) for Elavon VoidPC flow"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("Failed to create ElavonVoidPCRequest")?;
        to_xmldata(&request, "VoidPC (ccvoid)").map(Self)
    }
}

// Response handling for VoidPC flow. On approval ssl_txn_id is a new id for the
// void record. A declined ccvoid (e.g. 5040 after settlement) leaves the payment as it was.
impl<F> TryFrom<ResponseRouterData<ElavonVoidPCResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<ElavonVoidPCResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let (status, response_data) = match response.result {
            ElavonResult::Success(payment_response) => (
                AttemptStatus::VoidedPostCapture,
                Ok(PaymentsResponseData::PostCaptureVoidResponse {
                    post_capture_void_status: common_enums::PostCaptureVoidStatus::Succeeded,
                    connector_reference_id: Some(payment_response.ssl_txn_id),
                    status_code: http_code,
                }),
            ),
            ElavonResult::Error(error_payload) => (
                router_data.resource_common_data.status,
                Err(ErrorResponse {
                    status_code: http_code,
                    code: error_payload
                        .error_code
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: error_payload.error_message,
                    reason: error_payload.error_name,
                    attempt_status: None,
                    connector_transaction_id: error_payload.ssl_txn_id,
                    ..Default::default()
                }),
            ),
        };

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status,
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
    pub ssl_vendor_id: Option<Secret<String>>,
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

        let amount = item
            .connector
            .amount_converter
            .convert(&request_data.refund_money)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "Failed to convert refund amount {} {} to major unit for \
                         Elavon Refund (ccreturn) request",
                        request_data.refund_money.amount(),
                        request_data.currency
                    )),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            ssl_transaction_type: TransactionType::CcReturn,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_vendor_id: auth_type.ssl_vendor_id,
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
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to build ElavonRefundRequest (ccreturn) for Elavon Refund flow"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("Failed to create ElavonRefundRequest")?;
        to_xmldata(&request, "Refund (ccreturn)").map(Self)
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
                acquirer_reference_number: None,
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
                attempt_status: Some(FlowStatus::Payment(attempt_status)),
                connector_transaction_id: error_payload.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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
            ssl_vendor_id: auth_type.ssl_vendor_id,
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
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to build SyncRequest (txnquery) for Elavon RSync flow".to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("Failed to create SyncRequest for RSync")?;
        to_xmldata(&request, "RSync (txnquery)").map(Self)
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
            acquirer_reference_number: None,
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
                SyncTransactionType::Sale => AttemptStatus::Charged,
                SyncTransactionType::AuthOnly => AttemptStatus::Charged,
                SyncTransactionType::Return => AttemptStatus::Pending,
            },
            TransactionSyncStatus::OPN => match response.ssl_transaction_type {
                SyncTransactionType::AuthOnly => AttemptStatus::Authorized,
                SyncTransactionType::Sale => AttemptStatus::Pending,
                SyncTransactionType::Return => AttemptStatus::Pending,
            },
            TransactionSyncStatus::PEN | TransactionSyncStatus::REV => AttemptStatus::Pending,
            TransactionSyncStatus::PST
            | TransactionSyncStatus::FPR
            | TransactionSyncStatus::PRE => {
                if response.ssl_transaction_type == SyncTransactionType::AuthOnly
                    && response.ssl_trans_status == TransactionSyncStatus::PRE
                {
                    AttemptStatus::AuthenticationFailed
                } else {
                    AttemptStatus::Failure
                }
            }
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: DomainResponseId::ConnectorTransactionId(response.ssl_txn_id.clone()),
            redirection_data: None,
            connector_metadata: Some(serde_json::json!(response)),
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            mandate_reference: None,
            status_code: value.http_code,
            splits: None,
            payment_account_reference: None,
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

// ============================================================================
// RepeatPayment Flow (Merchant-Initiated / Token-Based)
// ============================================================================
//
// Elavon uses `ssl_token` (returned during the initial card payment when
// `ssl_add_token=Y` and `ssl_get_token=Y` are sent) as the mandate reference.
// Subsequent MIT payments skip the card fields entirely and pass `ssl_token`
// instead. The `connector_mandate_id` in the authorize response carries that
// token; the RepeatPayment request reads it back via `request.connector_mandate_id()`.

/// Wire request for a token-based (mandate) payment.
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct MandatePaymentRequest {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_vendor_id: Option<Secret<String>>,
    pub ssl_amount: StringMajorUnit,
    pub ssl_email: Option<common_utils::pii::Email>,
    pub ssl_token: Secret<String>,
    pub ssl_cit_mit_cof_indicator: CofIndicator,
    /// `1` marks a recurring payment. The installment value `2` also needs
    /// `ssl_payment_number` and `ssl_payment_count`, which no MIT carries, so it is never sent.
    pub ssl_recurring_flag: Option<u8>,
    pub ssl_invoice_number: Option<String>,
}

/// Form-encoded XML wrapper for the repeat payment request.
#[derive(Debug, Serialize)]
pub struct XMLRepeatPaymentRequest(pub HashMap<String, Secret<String, WithoutType>>);

/// Response type for RepeatPayment — same XML shape as other payment flows.
#[derive(Debug, Clone, Serialize)]
pub struct ElavonRepeatPaymentResponse {
    pub result: ElavonResult,
}

impl<'de> Deserialize<'de> for ElavonRepeatPaymentResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        xml_response_into_elavon_result(ElavonXmlResponse::deserialize(deserializer)?)
            .map(|result| Self { result })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MandatePaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let request = &router_data.request;
        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;

        let ssl_token = match &request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => {
                mandate_data.get_connector_mandate_id().ok_or_else(|| {
                    report!(IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Elavon RepeatPayment requires ssl_token from the initial \
                                 Authorize response stored as connector_mandate_id; \
                                 the field was present but empty"
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })
                })?
            }
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(report!(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon RepeatPayment only supports ConnectorMandateId (ssl_token); \
                             NetworkMandateId and NetworkTokenWithNTI are not accepted"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }))
                .attach_printable(
                    "Elavon RepeatPayment requires a ConnectorMandateId (ssl_token); \
                     NetworkMandateId and NetworkTokenWithNTI are not supported",
                )
            }
        };

        let transaction_type = match request.capture_method {
            Some(CaptureMethod::Manual) => TransactionType::CcAuthOnly,
            Some(CaptureMethod::Automatic) | None => TransactionType::CcSale,
            Some(other_capture_method) => {
                return Err(report!(IntegrationError::FlowNotSupported {
                    flow: format!("Capture method: {other_capture_method:?}"),
                    connector: "Elavon".to_string(),
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon RepeatPayment supports Automatic (ccsale) and \
                             Manual (ccauthonly) capture methods only"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }))
            }
        };

        let amount = item
            .connector
            .amount_converter
            .convert(&request.money)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "Failed to convert repeat payment amount {} {} to major unit for \
                         Elavon RepeatPayment request",
                        request.money.amount(),
                        request.currency
                    )),
                    ..Default::default()
                },
            })?;

        let cof_indicator = CofIndicator::from(request.mit_category.clone());
        Ok(Self {
            ssl_transaction_type: transaction_type,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_vendor_id: auth_type.ssl_vendor_id,
            ssl_amount: amount,
            ssl_email: request.email.clone(),
            ssl_token: Secret::new(ssl_token),
            ssl_cit_mit_cof_indicator: cof_indicator,
            ssl_recurring_flag: (cof_indicator == CofIndicator::MerchantStandingOrder).then_some(1),
            ssl_invoice_number: Some(fit_to_length(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
                INVOICE_NUMBER_MAX_LENGTH,
            )),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XMLRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request =
            MandatePaymentRequest::try_from(data)
                .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to build MandatePaymentRequest (ccsale/ccauthonly with ssl_token) \
                         for Elavon RepeatPayment flow"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        to_xmldata(&request, "RepeatPayment").map(Self)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ElavonRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<ElavonRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let (attempt_status, error_response) =
            get_elavon_attempt_status(&response.result, http_code);

        let payments_response_data = match (&response.result, error_response) {
            (ElavonResult::Success(payment_resp_struct), None) => {
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: DomainResponseId::ConnectorTransactionId(
                        payment_resp_struct.ssl_txn_id.clone(),
                    ),
                    redirection_data: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: payment_resp_struct.ssl_approval_code.clone(),
                    incremental_authorization_allowed: None,
                    // Token-based repeat payments do not issue a new ssl_token;
                    // the original mandate remains valid.
                    mandate_reference: None,
                    status_code: http_code,
                    splits: None,
                    payment_account_reference: None,
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
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                connector_transaction_id: error_payload.ssl_txn_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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

// SetupMandate uses ccgettoken with ssl_verify=Y: a $0 account verification that stores
// the card in Card Manager and returns its token. It never charges the card.
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct ElavonSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_vendor_id: Option<Secret<String>>,
    pub ssl_card_number: RawCardNumber<T>,
    pub ssl_exp_date: Secret<String>,
    pub ssl_cvv2cvc2: Secret<String>,
    pub ssl_cvv2cvc2_indicator: i32,
    pub ssl_avs_address: Option<Secret<String>>,
    pub ssl_avs_zip: Option<Secret<String>>,
    pub ssl_first_name: Option<Secret<String>>,
    pub ssl_last_name: Option<Secret<String>>,
    pub ssl_email: Option<common_utils::pii::Email>,
    pub ssl_verify: &'static str,
    pub ssl_add_token: &'static str,
    pub ssl_customer_code: Option<String>,
    pub ssl_invoice_number: String,
    #[serde(flatten)]
    pub three_ds: Option<ElavonThreeDsFields>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ElavonSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let request_data = &router_data.request;

        // ccgettoken takes no amount, so a non-zero setup would report a charge that never happened.
        if request_data
            .money
            .as_ref()
            .is_some_and(|money| money.amount() > common_utils::types::MinorUnit::zero())
        {
            return Err(report!(IntegrationError::FlowNotSupported {
                flow: "SetupMandate with a non-zero amount".to_string(),
                connector: "Elavon".to_string(),
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon SetupMandate (ccgettoken) is a $0 card verification and \
                         cannot charge an amount"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }));
        }

        let card = match &request_data.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(report!(IntegrationError::NotImplemented(
                    "Only card payments are supported for Elavon SetupMandate".to_string(),
                    Default::default()
                )))
            }
        };

        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;
        let common = &router_data.resource_common_data;
        let (avs_address, avs_zip) = get_avs_details_from_payment_address(Some(&common.address));
        let (first_name, last_name) = get_first_and_last_name(common, card);

        Ok(Self {
            ssl_transaction_type: TransactionType::CcGetToken,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_vendor_id: auth_type.ssl_vendor_id,
            ssl_card_number: card.card_number.clone(),
            ssl_exp_date: get_card_exp_date(card),
            ssl_cvv2cvc2: card.card_cvc.clone(),
            ssl_cvv2cvc2_indicator: 1,
            ssl_avs_address: avs_address,
            ssl_avs_zip: avs_zip,
            ssl_first_name: first_name,
            ssl_last_name: last_name,
            ssl_email: request_data.email.clone(),
            ssl_verify: "Y",
            ssl_add_token: "Y",
            ssl_customer_code: request_data
                .customer_id
                .as_ref()
                .map(|c| fit_to_length(c.get_string_repr(), CUSTOMER_CODE_MAX_LENGTH)),
            ssl_invoice_number: fit_to_length(
                &common.connector_request_reference_id,
                INVOICE_NUMBER_MAX_LENGTH,
            ),
            three_ds: get_three_ds_fields(
                request_data.authentication_data.as_ref(),
                common.is_three_ds(),
            )?,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct XMLSetupMandateRequest(pub HashMap<String, Secret<String, WithoutType>>);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XMLSetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = ElavonSetupMandateRequest::try_from(data)
            .attach_printable("Failed to create ElavonSetupMandateRequest")?;
        to_xmldata(&request, "SetupMandate (ccgettoken)").map(Self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ElavonSetupMandateResponse {
    pub result: ElavonResult,
}

impl<'de> Deserialize<'de> for ElavonSetupMandateResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        xml_response_into_elavon_result(ElavonXmlResponse::deserialize(deserializer)?)
            .map(|result| Self { result })
    }
}

// Response handling for SetupMandate flow. A declined verification arrives as ssl_result=1
// ("Acct Verification Failed"), which xml_response_into_elavon_result turns into Error.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ElavonSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<ElavonSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let (status, response_data) = match response.result {
            ElavonResult::Success(payment_response) => {
                let token = if payment_response.ssl_token_response.as_deref() == Some("SUCCESS") {
                    payment_response
                        .ssl_token
                        .as_ref()
                        .map(|t| t.peek().to_string())
                } else {
                    None
                };
                match token {
                    Some(token) => (
                        AttemptStatus::Charged,
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: DomainResponseId::ConnectorTransactionId(
                                payment_response.ssl_txn_id,
                            ),
                            redirection_data: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: payment_response.ssl_approval_code,
                            incremental_authorization_allowed: None,
                            mandate_reference: Some(Box::new(MandateReference {
                                connector_mandate_id: Some(token),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata: None,
                            })),
                            status_code: http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                    ),
                    // Never report a saved card without its token.
                    None => (
                        AttemptStatus::Failure,
                        Err(ErrorResponse {
                            status_code: http_code,
                            code: NO_ERROR_CODE.to_string(),
                            message: format!(
                                "Converge approved the card verification but returned no card \
                                 token (ssl_token_response: {})",
                                payment_response
                                    .ssl_token_response
                                    .as_deref()
                                    .unwrap_or("absent")
                            ),
                            reason: None,
                            attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                            connector_transaction_id: Some(payment_response.ssl_txn_id),
                            ..Default::default()
                        }),
                    ),
                }
            }
            ElavonResult::Error(error_payload) => (
                AttemptStatus::Failure,
                Err(ErrorResponse {
                    status_code: http_code,
                    code: error_payload
                        .error_code
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: error_payload.error_message,
                    reason: error_payload.error_name,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: error_payload.ssl_txn_id,
                    ..Default::default()
                }),
            ),
        };

        Ok(Self {
            response: response_data,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// IncrementalAuthorization reuses ccauthonly with the original auth's ssl_txn_id; ssl_amount
// is the increase (delta), not the new total. Capture and void keep using the original ID.
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct ElavonIncrementalAuthorizationRequest {
    pub ssl_transaction_type: TransactionType,
    pub ssl_account_id: Secret<String>,
    pub ssl_user_id: Secret<String>,
    pub ssl_pin: Secret<String>,
    pub ssl_vendor_id: Option<Secret<String>>,
    pub ssl_txn_id: String,
    pub ssl_amount: StringMajorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ElavonIncrementalAuthorizationRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth_type = ElavonAuthType::try_from(&router_data.connector_config)?;

        let original_connector_txn_id = match &router_data.request.connector_transaction_id {
            DomainResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(report!(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon IncrementalAuthorization (ccauthonly) requires the ssl_txn_id \
                             of the original auth-only transaction as connector_transaction_id"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }))
            }
        };

        // `amount` is the new total; Converge wants only the increase.
        let additional_amount = router_data
            .request
            .additional_money
            .clone()
            .ok_or_else(|| {
                report!(IntegrationError::MissingRequiredField {
                    field_name: "additional_amount",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon IncrementalAuthorization (ccauthonly) sends the increase as \
                         ssl_amount, not the new total"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })
            })?;

        let amount = item
            .connector
            .amount_converter
            .convert(&additional_amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "Failed to convert additional amount {} {} to major unit for \
                         Elavon IncrementalAuthorization (ccauthonly) request",
                        additional_amount.amount(),
                        additional_amount.currency()
                    )),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            ssl_transaction_type: TransactionType::CcAuthOnly,
            ssl_account_id: auth_type.ssl_merchant_id,
            ssl_user_id: auth_type.ssl_user_id,
            ssl_pin: auth_type.ssl_pin,
            ssl_vendor_id: auth_type.ssl_vendor_id,
            ssl_txn_id: original_connector_txn_id,
            ssl_amount: amount,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct XMLIncrementalAuthorizationRequest(pub HashMap<String, Secret<String, WithoutType>>);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XMLIncrementalAuthorizationRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        data: ElavonRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = ElavonIncrementalAuthorizationRequest::try_from(data)
            .attach_printable("Failed to create ElavonIncrementalAuthorizationRequest")?;
        to_xmldata(&request, "IncrementalAuthorization (ccauthonly)").map(Self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ElavonIncrementalAuthorizationResponse {
    pub result: ElavonResult,
}

impl<'de> Deserialize<'de> for ElavonIncrementalAuthorizationResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        xml_response_into_elavon_result(ElavonXmlResponse::deserialize(deserializer)?)
            .map(|result| Self { result })
    }
}

// Response handling for IncrementalAuthorization flow. A refused increment leaves the
// original authorization intact, so the error carries no attempt_status and the payment
// status is left untouched.
impl TryFrom<ResponseRouterData<ElavonIncrementalAuthorizationResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<ElavonIncrementalAuthorizationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        match response.result {
            // ssl_txn_id here is a new ID for the increment; txnquery cannot read it.
            ElavonResult::Success(payment_response) => Ok(Self {
                response: Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                    status: AuthorizationStatus::Success,
                    connector_authorization_id: Some(payment_response.ssl_txn_id),
                    status_code: http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Authorized,
                    ..router_data.resource_common_data
                },
                ..router_data
            }),
            ElavonResult::Error(error_payload) => Ok(Self {
                response: Err(ErrorResponse {
                    status_code: http_code,
                    code: error_payload
                        .error_code
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: error_payload.error_message,
                    reason: error_payload.error_name,
                    attempt_status: None,
                    connector_transaction_id: error_payload.ssl_txn_id,
                    ..Default::default()
                }),
                ..router_data
            }),
        }
    }
}
