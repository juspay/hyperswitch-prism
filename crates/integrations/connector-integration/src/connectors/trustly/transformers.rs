use crate::{connectors::trustly::TrustlyRouterData, types::ResponseRouterData, utils};
use base64::{engine::general_purpose, Engine};
use common_enums::{self, AttemptStatus, CountryAlpha2, Currency};
use common_utils::{
    pii::{self, IpAddress},
    request::Method,
    StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, RefundFlowData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{
        BankRedirectData, DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::base64_decode,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use openssl::{
    hash::MessageDigest,
    pkey::PKey,
    rsa::Rsa,
    sign::{Signer, Verifier},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const TRUSTLY_VERSION: &str = "1.1";
const BANK_LAST_DIGITS_LEN: usize = 4;

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyAuthType {
    pub(super) username: Secret<String>,
    pub(super) password: Secret<String>,
    pub(super) private_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TrustlyAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Trustly {
                username,
                password,
                private_key,
                ..
            } => Ok(Self {
                username: username.clone(),
                password: password.clone(),
                private_key: private_key.clone(),
            }),
            _ => Err(errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

//Error response structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyErrorResponse {
    pub version: String,
    pub error: TrustlyErrorResponseError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyErrorResponseError {
    pub name: String,
    pub code: i64,
    pub message: String,
    pub error: TrustlyErrorResponseErrorDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyErrorResponseErrorDetails {
    pub uuid: String,
}

// Authorize
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyPaymentRequest {
    pub method: TrustlyMethod,
    pub version: String,
    pub params: TrustlyPaymentRequestParams,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustlyPaymentRequestParams {
    data: TrustlyPaymentRequestData,
    signature: Secret<String>,
    u_u_i_d: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustlyPaymentRequestData {
    attributes: TrustlyPaymentRequestAttributes,
    end_user_i_d: String,
    message_i_d: String,
    notification_u_r_l: String,
    password: Secret<String>,
    username: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustlyPaymentRequestAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    account_i_d: Option<Secret<String>>,
    amount: StringMajorUnit,
    country: CountryAlpha2,
    currency: Currency,
    email: pii::Email,
    fail_u_r_l: String,
    firstname: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    i_p: Option<Secret<String, IpAddress>>,
    lastname: Secret<String>,
    locale: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mobile: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address_country: Option<CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address_postal_code: Option<Secret<String>>,
    shopper_statement: String,
    success_u_r_l: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum TrustlyMethod {
    Deposit,
    Refund,
}

impl TrustlyMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Deposit => "Deposit",
            Self::Refund => "Refund",
        }
    }
}

fn trustly_serialize<T: Serialize>(data: &T) -> String {
    let value = serde_json::to_value(data).unwrap_or_default();
    serialize_value(&value)
}

enum Algorithm {
    SHA256,
    SHA384,
    SHA512,
    SHA1,
}

impl Algorithm {
    fn message_digest(&self) -> MessageDigest {
        match self {
            Self::SHA256 => MessageDigest::sha256(),
            Self::SHA384 => MessageDigest::sha384(),
            Self::SHA512 => MessageDigest::sha512(),
            Self::SHA1 => MessageDigest::sha1(),
        }
    }

    fn prefix(&self) -> &'static str {
        "alg=RS256;"
    }
}

fn serialize_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: BTreeMap<_, _> = map.iter().collect();
            sorted
                .iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| format!("{}{}", k, serialize_value(v)))
                .collect()
        }
        serde_json::Value::Array(arr) => arr.iter().map(serialize_value).collect(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
    }
}

pub fn generate_trustly_signature<T: Serialize>(
    method: &str,
    uuid: &str,
    data: &T,
    private_key: &str,
) -> Result<String, errors::IntegrationError> {
    let algorithm = Algorithm::SHA256;
    let pem = base64_decode(private_key.to_string()).map_err(|_| {
        errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    })?;
    let rsa = Rsa::private_key_from_pem(&pem).map_err(|_| {
        errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    })?;
    let private_key =
        PKey::from_rsa(rsa).map_err(|_| errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;

    let plaintext = format!("{}{}{}", method, uuid, trustly_serialize(data));

    let mut signer = Signer::new(algorithm.message_digest(), &private_key).map_err(|_| {
        errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    })?;
    signer.update(plaintext.as_bytes()).map_err(|_| {
        errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    })?;
    let signature =
        signer
            .sign_to_vec()
            .map_err(|_| errors::IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

    Ok(format!(
        "{}{}",
        algorithm.prefix(),
        general_purpose::STANDARD.encode(&signature)
    ))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustlyRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TrustlyPaymentRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: TrustlyRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(BankRedirectData::Trustly { additional_details, .. }) => {
                let auth_details = TrustlyAuthType::try_from(&item.router_data.connector_config)?;

                let return_url = item
                    .router_data
                    .resource_common_data
                    .return_url
                    .clone()
                    .ok_or(errors::IntegrationError::MissingRequiredField {
                        field_name: "return_url",
                        context: Default::default(),
                    })?;
                let uuid = common_utils::fp_utils::generate_uuid_v4();
                let account_id = additional_details
                    .as_ref()
                    .and_then(|details| details.peek().get("account_id"))
                    .and_then(|aid| aid.as_str())
                    .map(|s| s.to_string());
                let attributes = TrustlyPaymentRequestAttributes {
                    account_i_d: account_id.map(Secret::new),
                    amount: item
                        .connector
                        .amount_converter
                        .convert(
                            item.router_data.request.minor_amount,
                            item.router_data.request.currency,
                        )
                        .change_context(errors::IntegrationError::AmountConversionFailed {
                            context: Default::default(),
                        })?,
                    country: match item.router_data.resource_common_data.get_billing_country() {
                        Ok(country) => country,
                        Err(_) => item
                            .router_data
                            .resource_common_data
                            .get_optional_shipping_country()
                            .ok_or(errors::IntegrationError::MissingRequiredField {
                                field_name: "country",
                                context: Default::default(),
                            })?,
                    },
                    currency: item.router_data.request.currency,
                    email: item
                        .router_data
                        .request
                        .email
                        .clone()
                        .or(item
                            .router_data
                            .resource_common_data
                            .get_optional_billing_email())
                        .ok_or(errors::IntegrationError::MissingRequiredField {
                            field_name: "email",
                            context: Default::default(),
                        })?,
                    fail_u_r_l: return_url.clone(),
                    firstname: item
                        .router_data
                        .resource_common_data
                        .get_billing_first_name()?,
                    i_p: item.router_data.request.get_ip_address_as_optional(),
                    lastname: item
                        .router_data
                        .resource_common_data
                        .get_billing_last_name()?,
                    locale: item
                        .router_data
                        .request
                        .get_optional_language_from_browser_info()
                        .ok_or(errors::IntegrationError::MissingRequiredField {
                            field_name: "locale",
                            context: Default::default(),
                        })?
                        .replace('-', "_"),
                    mobile: item
                        .router_data
                        .resource_common_data
                        .address
                        .get_payment_billing()
                        .and_then(|billing| billing.get_phone_with_country_code().ok()),
                    shipping_address_city: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_city(),
                    shipping_address_country: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_country(),
                    shipping_address_line1: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_line1(),
                    shipping_address_line2: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_line2(),
                    shipping_address_postal_code: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_zip(),
                    shopper_statement: item.router_data.resource_common_data.get_description()?,
                    success_u_r_l: return_url,
                };

                let data = TrustlyPaymentRequestData {
                    attributes,
                    end_user_i_d: item
                        .router_data
                        .resource_common_data
                        .get_connector_customer_id()?,
                    message_i_d: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id,
                    notification_u_r_l: "https://3245-223-185-33-154.ngrok-free.app/webhooks/merchant_1790505731/trustly".to_string(),
                    // item.router_data.request.webhook_url.clone().ok_or(
                    //     errors::IntegrationError::MissingRequiredField {
                    //         field_name: "webhook_url",
                    //         context: Default::default(),
                    //     },
                    // )?,
                    password: auth_details.password.clone(),
                    username: auth_details.username.clone(),
                };

                let signature = generate_trustly_signature(
                    TrustlyMethod::Deposit.as_str(),
                    uuid.as_str(),
                    &data,
                    &auth_details.private_key.expose(),
                )?;

                Ok(Self {
                    method: TrustlyMethod::Deposit,
                    version: TRUSTLY_VERSION.to_string(),
                    params: TrustlyPaymentRequestParams {
                        data,
                        signature: Secret::new(signature),
                        u_u_i_d: uuid,
                    },
                })
            }
            PaymentMethodData::Card(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_) => Err(error_stack::report!(
                errors::IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Trustly"),
                    connector: "Trustly",
                    context: Default::default(),
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TrustlyPaymentsResponse {
    Success(TrustlyPaymentsResponseSuccess),
    Failure(TrustlyErrorResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyPaymentsResponseSuccess {
    pub version: String,
    pub result: TrustlyPaymentsResponseResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyPaymentsResponseResult {
    pub signature: Secret<String>,
    pub uuid: String,
    pub method: String,
    pub data: TrustlyPaymentsResponseData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyPaymentsResponseData {
    pub orderid: String,
    pub url: String,
}

impl<F, T> TryFrom<ResponseRouterData<TrustlyPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<TrustlyPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            TrustlyPaymentsResponse::Success(response) => {
                let redirection_url = response.result.data.url;
                let redirection_data = Some(RedirectForm::Form {
                    endpoint: redirection_url,
                    method: Method::Get,
                    form_fields: Default::default(),
                });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::AuthenticationPending,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            response.result.data.orderid,
                        ),
                        redirection_data: redirection_data.map(Box::new),
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: Some(response.result.uuid),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
            TrustlyPaymentsResponse::Failure(error_response) => {
                let error_response = ErrorResponse {
                    code: error_response.error.code.to_string(),
                    message: error_response.error.message.clone(),
                    reason: Some(error_response.error.message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(error_response.error.error.uuid),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(error_response),
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyRefundRequest {
    pub method: TrustlyMethod,
    pub params: TrustlyRefundRequestParams,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustlyRefundRequestParams {
    data: TrustlyRefundRequestData,
    signature: Secret<String>,
    u_u_i_d: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustlyRefundRequestData {
    username: Secret<String>,
    password: Secret<String>,
    order_i_d: String,
    amount: StringMajorUnit,
    currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<TrustlyRefundAttributes>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustlyRefundAttributes {
    external_reference: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustlyRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TrustlyRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TrustlyRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth_details = TrustlyAuthType::try_from(&item.router_data.connector_config)?;
        let uuid = common_utils::fp_utils::generate_uuid_v4();
        let attributes = Some(TrustlyRefundAttributes {
            external_reference: item
                .router_data
                .resource_common_data
                .refund_id
                .clone()
                .ok_or(errors::IntegrationError::MissingRequiredField {
                    field_name: "refund_id",
                    context: Default::default(),
                })?,
        });
        let data = TrustlyRefundRequestData {
            amount: item
                .connector
                .amount_converter
                .convert(
                    item.router_data.request.minor_refund_amount,
                    item.router_data.request.currency,
                )
                .change_context(errors::IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
            attributes,
            currency: item.router_data.request.currency,
            order_i_d: item.router_data.request.connector_transaction_id.clone(),
            password: auth_details.password,
            username: auth_details.username,
        };

        let signature = generate_trustly_signature(
            TrustlyMethod::Refund.as_str(),
            uuid.as_str(),
            &data,
            &auth_details.private_key.expose(),
        )?;

        Ok(Self {
            method: TrustlyMethod::Refund,
            version: TRUSTLY_VERSION.to_string(),
            params: TrustlyRefundRequestParams {
                data,
                signature: Secret::new(signature),
                u_u_i_d: uuid,
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TrustlyRefundResponse {
    Success(TrustlyRefundResponseSuccess),
    Failure(TrustlyErrorResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyRefundResponseSuccess {
    pub version: String,
    pub result: TrustlyRefundResponseResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyRefundResponseResult {
    pub signature: Secret<String>,
    pub method: String,
    pub data: TrustlyRefundResponseData,
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrustlyRefundResult {
    #[serde(rename = "1")]
    Pending,
    #[serde(rename = "0")]
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyRefundResponseData {
    pub result: TrustlyRefundResult,
    pub orderid: String,
}

impl From<TrustlyRefundResult> for common_enums::RefundStatus {
    fn from(item: TrustlyRefundResult) -> Self {
        match item {
            TrustlyRefundResult::Pending => Self::Pending,
            TrustlyRefundResult::Failed => Self::Failure,
        }
    }
}

impl TryFrom<ResponseRouterData<TrustlyRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustlyRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            TrustlyRefundResponse::Success(response) => Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: response.result.data.orderid,
                    refund_status: common_enums::RefundStatus::from(response.result.data.result),
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
                ..item.router_data
            }),
            TrustlyRefundResponse::Failure(error_response) => {
                let error_response = ErrorResponse {
                    code: error_response.error.code.to_string(),
                    message: error_response.error.message.clone(),
                    reason: Some(error_response.error.message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(error_response.error.error.uuid),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                };

                Ok(Self {
                    response: Err(error_response),
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookBody {
    pub method: TrustlyWebhookMethod,
    pub params: TrustlyWebhookParams,
    pub version: String,
}

impl TrustlyWebhookMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Credit => "credit",
            Self::Debit => "debit",
            Self::Cancel => "cancel",
            Self::Account => "account",
            Self::Pending => "pending",
            Self::PayoutConfirmation => "payoutconfirmation",
            Self::PayoutFailed => "payoutfailed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TrustlyWebhookMethod {
    Credit,
    Debit,
    Cancel,
    Account,
    Pending,
    PayoutConfirmation,
    PayoutFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookParams {
    pub signature: String,
    pub uuid: String,
    pub data: TrustlyWebhookData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookData {
    pub amount: Option<StringMajorUnit>,
    pub currency: Option<Currency>,
    pub messageid: Secret<String>,
    pub orderid: String,
    pub enduserid: Option<String>,
    pub accountid: Option<String>,
    pub verified: Option<String>,
    pub notificationid: String,
    pub timestamp: Option<String>,
    pub attributes: Option<TrustlyWebhookAttributes>,
    pub errorcode: Option<String>,
    pub errormessage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookAttributes {
    pub bank: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub name: Option<Secret<String>>,
    pub address: Option<Secret<String>>,
    pub zipcode: Option<Secret<String>>,
    pub personid: Option<Secret<String>>,
    pub descriptor: Option<String>,
    pub lastdigits: Option<String>,
    pub clearinghouse: Option<String>,
}

fn map_trustly_bank_to_bank_name(bank: &str) -> Result<common_enums::BankNames, String> {
    match bank.trim().to_lowercase().as_str() {
        "abanca" => Ok(common_enums::BankNames::Abanca),
        "abn amro" => Ok(common_enums::BankNames::AbnAmro),
        "aib" => Ok(common_enums::BankNames::Aib),
        "aktia" => Ok(common_enums::BankNames::Aktia),
        "ålandsbanken" => Ok(common_enums::BankNames::Alandsbanken),
        "alior bank" => Ok(common_enums::BankNames::AliorBank),
        "alm. brand" => Ok(common_enums::BankNames::AlmBrand),
        "alpha fx" => Ok(common_enums::BankNames::AlphaFx),
        "arbejdernes landsbank" => Ok(common_enums::BankNames::ArbejdernesLandsbank),
        "arbuthnot latham" => Ok(common_enums::BankNames::ArbuthnotLatham),
        "asn bank" => Ok(common_enums::BankNames::AsnBank),
        "banco sabadell" => Ok(common_enums::BankNames::BancoDeSabadell),
        "banco popular" => Ok(common_enums::BankNames::BancoPopular),
        "banco santander" => Ok(common_enums::BankNames::BancoSantander),
        "bank99 (ex-ing)" => Ok(common_enums::BankNames::Bank99Ag),
        "bank austria" => Ok(common_enums::BankNames::BankAustria),
        "millennium bank" => Ok(common_enums::BankNames::BankMillennium),
        "bank of ireland uk" => Ok(common_enums::BankNames::BankOfIrelandUk),
        "bank of scotland" => Ok(common_enums::BankNames::BankOfScotland),
        "bank pekao" => Ok(common_enums::BankNames::BankPekaoSa),
        "bank pocztowy" => Ok(common_enums::BankNames::BankPocztowy),
        "bankia" => Ok(common_enums::BankNames::Bankia),
        "bankinter" => Ok(common_enums::BankNames::Bankinter),
        "barclays" => Ok(common_enums::BankNames::Barclays),
        "bawag p.s.k." => Ok(common_enums::BankNames::BawagPsk),
        "bbva" => Ok(common_enums::BankNames::Bbva),
        "bn bank asa" => Ok(common_enums::BankNames::BnBank),
        "bnp paribas" => Ok(common_enums::BankNames::BnpParibas),
        "caixabank" => Ok(common_enums::BankNames::Caixa),
        "cater allen" => Ok(common_enums::BankNames::CaterAllen),
        "česká spořitelna" => Ok(common_enums::BankNames::CeskaSporitelna),
        "chase uk" => Ok(common_enums::BankNames::Chase),
        "chelsea building society" => Ok(common_enums::BankNames::ChelseaBuildingSociety),
        "citadele" => Ok(common_enums::BankNames::Citadele),
        "citi handlowy" | "citibank" => Ok(common_enums::BankNames::Citi),
        "clydesdale bank" => Ok(common_enums::BankNames::ClydesdaleBank),
        "comdirect" => Ok(common_enums::BankNames::Comdirect),
        "commerzbank" => Ok(common_enums::BankNames::Commerzbank),
        "coop pank" => Ok(common_enums::BankNames::CoopPank),
        "the co-operative bank" => Ok(common_enums::BankNames::CooperativeBank),
        "coutts" => Ok(common_enums::BankNames::Coutts),
        "credit agricole" => Ok(common_enums::BankNames::CreditAgricole),
        "the cumberland" => Ok(common_enums::BankNames::Cumberland),
        "dab bank" => Ok(common_enums::BankNames::DabBank),
        "danske bank" => Ok(common_enums::BankNames::DanskeBank),
        "deutsche bank" | "deutsche bank polska" => Ok(common_enums::BankNames::DeutscheBank),
        "djurslands bank" => Ok(common_enums::BankNames::DjurslandsBank),
        "dkb - deutsche kreditbank" => Ok(common_enums::BankNames::Dkb),
        "dnb" => Ok(common_enums::BankNames::Dnb),
        "easybank" => Ok(common_enums::BankNames::EasyBank),
        "erste sparkasse george" => Ok(common_enums::BankNames::ErsteBankUndSparkassen),
        "etne sparebank" => Ok(common_enums::BankNames::EtneSparebank),
        "evo banco" => Ok(common_enums::BankNames::EvoBanco),
        "fana sparebank" => Ok(common_enums::BankNames::FanaSparebank),
        "fidor bank" => Ok(common_enums::BankNames::FidorBank),
        "first direct" => Ok(common_enums::BankNames::FirstDirect),
        "flekkefjord sparebank" => Ok(common_enums::BankNames::FlekkefjordSparebank),
        "forex" => Ok(common_enums::BankNames::ForexBank),
        "getin bank" => Ok(common_enums::BankNames::GetinBank),
        "halifax" => Ok(common_enums::BankNames::Halifax),
        "handelsbanken" => Ok(common_enums::BankNames::Handelsbanken),
        "haugesund sparebank" => Ok(common_enums::BankNames::HaugesundSparebank),
        "c. hoare & co." => Ok(common_enums::BankNames::HoareAndCo),
        "hsbc uk" => Ok(common_enums::BankNames::Hsbc),
        "hypovereinsbank" => Ok(common_enums::BankNames::HypoVereinsbank),
        "ibercaja" => Ok(common_enums::BankNames::Ibercaja),
        "ica banken" => Ok(common_enums::BankNames::IcaBanken),
        "icici bank uk" => Ok(common_enums::BankNames::IciciBank),
        "ing" | "ing bank śląski" | "ing-diba" => Ok(common_enums::BankNames::Ing),
        "inteligo" => Ok(common_enums::BankNames::Inteligo),
        "investec" => Ok(common_enums::BankNames::Investec),
        "jyske bank" => Ok(common_enums::BankNames::JyskeBank),
        "kleinwort hambros" => Ok(common_enums::BankNames::KleinwortHambros),
        "klp banken" => Ok(common_enums::BankNames::KlpBanken),
        "knab" => Ok(common_enums::BankNames::Knab),
        "kreditbanken" => Ok(common_enums::BankNames::Kreditbanken),
        "kutxabank" => Ok(common_enums::BankNames::Kutxabank),
        "landkreditt bank as" => Ok(common_enums::BankNames::LandkredittBank),
        "länsförsäkringar" => Ok(common_enums::BankNames::Lansforsakringar),
        "lhv pank" => Ok(common_enums::BankNames::LhvPank),
        "lillesands sparebank" => Ok(common_enums::BankNames::LillesandsSparebank),
        "lloyds bank" => Ok(common_enums::BankNames::Lloyds),
        "luminor" => Ok(common_enums::BankNames::Luminor),
        "luster sparebank" => Ok(common_enums::BankNames::LusterSparebank),
        "mbna" => Ok(common_enums::BankNames::Mbna),
        "metro bank" => Ok(common_enums::BankNames::MetroBank),
        "monzo" => Ok(common_enums::BankNames::Monzo),
        "n26" => Ok(common_enums::BankNames::N26),
        "natwest" => Ok(common_enums::BankNames::NatWest),
        "nationwide" => Ok(common_enums::BankNames::Nationwide),
        "nordea" | "nordea direct" => Ok(common_enums::BankNames::Nordea),
        "nordfyns bank" => Ok(common_enums::BankNames::NordfynsBank),
        "nordjyske bank" => Ok(common_enums::BankNames::NordjyskeBank),
        "norisbank" => Ok(common_enums::BankNames::Norisbank),
        "nykredit bank" => Ok(common_enums::BankNames::NykreditBank),
        "obos-banken as" => Ok(common_enums::BankNames::ObosBanken),
        "omasp" => Ok(common_enums::BankNames::OmaSp),
        "op" => Ok(common_enums::BankNames::Op),
        "orange finanse" => Ok(common_enums::BankNames::OrangeFinanse),
        "pareto bank asa" => Ok(common_enums::BankNames::ParetoBank),
        "pko bank polski" => Ok(common_enums::BankNames::PkoBankPolski),
        "pop pankki" => Ok(common_enums::BankNames::PopPankki),
        "postbank" => Ok(common_enums::BankNames::PostBank),
        "rabobank" => Ok(common_enums::BankNames::Rabobank),
        "raiffeisen" => Ok(common_enums::BankNames::RaiffeisenBankengruppeOsterreich),
        "regio bank" => Ok(common_enums::BankNames::Regiobank),
        "revolut" => Ok(common_enums::BankNames::Revolut),
        "ringkjøbing landbobank" => Ok(common_enums::BankNames::RingkjobingLandbobank),
        "royal bank of scotland" => Ok(common_enums::BankNames::RoyalBankOfScotland),
        "s-pankki" => Ok(common_enums::BankNames::SPankki),
        "säästöpankki" => Ok(common_enums::BankNames::Saastopankki),
        "santander" | "santander uk" => Ok(common_enums::BankNames::Santander),
        "sbanken" => Ok(common_enums::BankNames::Sbanken),
        "seb" => Ok(common_enums::BankNames::Seb),
        "šiaulių bankas" => Ok(common_enums::BankNames::SiauliuBankas),
        "silicon valley bank uk" => Ok(common_enums::BankNames::SiliconValleyBank),
        "skandiabanken" => Ok(common_enums::BankNames::Skandiabanken),
        "skjern bank" => Ok(common_enums::BankNames::SkjernBank),
        "skudenes & aakra sparebank" => Ok(common_enums::BankNames::SkudenesOgAakraSparebank),
        "sns bank" => Ok(common_enums::BankNames::SnsBank),
        "søgne og greipstad sparebank" => Ok(common_enums::BankNames::SogneOgGreipstadSparebank),
        "spar nord bank" => Ok(common_enums::BankNames::SparNordBank),
        "sparbanken syd" => Ok(common_enums::BankNames::SparbankenSyd),
        "sparda-bank" => Ok(common_enums::BankNames::SpardaBank),
        "sparebank 1"
        | "sparebank 1 gudbrandsdal"
        | "sparebank 1 hallingdal valdres"
        | "sparebank 1 lom og skjåk"
        | "sparebank 1 modum"
        | "sparebank 1 nordmøre"
        | "sparebank 1 ringerike hadeland"
        | "sparebank 1 smn"
        | "sparebank 1 sr-bank"
        | "sparebank 1 søre sunnmøre"
        | "sparebank 1 sørøst-norge (bv)"
        | "sparebank 1 sørøst-norge (telemark)"
        | "sparebank 1 østfold akershus"
        | "sparebank 1 østlandet" => Ok(common_enums::BankNames::SpareBank1),
        "sparebanken møre" => Ok(common_enums::BankNames::SparebankenMore),
        "sparebanken øst" => Ok(common_enums::BankNames::SparebankenOst),
        "sparebanken sogn og fjordane" => Ok(common_enums::BankNames::SparebankenSognOgFjordane),
        "sparebanken sør" => Ok(common_enums::BankNames::SparebankenSor),
        "sparebanken vest" => Ok(common_enums::BankNames::SparebankenVest),
        "sparekassen danmark" => Ok(common_enums::BankNames::SparekassenDanmark),
        "sparekassen sjælland-fyn" => Ok(common_enums::BankNames::SparekassenSjaellandFyn),
        "spareskillingsbanken" => Ok(common_enums::BankNames::Spareskillingsbanken),
        "sparkasse" => Ok(common_enums::BankNames::Sparkasse),
        "starling bank" => Ok(common_enums::BankNames::Starling),
        "swedbank" | "swedbank (& sparbankerna)" => Ok(common_enums::BankNames::Swedbank),
        "sydbank" => Ok(common_enums::BankNames::Sydbank),
        "targobank" => Ok(common_enums::BankNames::TargoBank),
        "tesco bank" => Ok(common_enums::BankNames::TescoBank),
        "tide" => Ok(common_enums::BankNames::Tide),
        "triodos" => Ok(common_enums::BankNames::Triodos),
        "tsb bank" => Ok(common_enums::BankNames::TsbBank),
        "ulster bank" => Ok(common_enums::BankNames::UlsterBank),
        "vanquis bank" => Ok(common_enums::BankNames::VanquisBank),
        "vestjysk bank" => Ok(common_enums::BankNames::VestjyskBank),
        "virgin money uk" => Ok(common_enums::BankNames::VirginMoney),
        "volksbank" => Ok(common_enums::BankNames::Volksbank),
        "volksbank-raiffeisenbank" => Ok(common_enums::BankNames::VolksbankenRaiffeisenbanken),
        "voss sparebank" => Ok(common_enums::BankNames::VossSparebank),
        "wise" => Ok(common_enums::BankNames::Wise),
        "yorkshire bank" => Ok(common_enums::BankNames::YorkshireBank),
        "yorkshire building society" => Ok(common_enums::BankNames::YorkshireBuildingSociety),
        "cash plus" => Ok(common_enums::BankNames::Zempler),
        other => Err(format!("Unknown Trustly bank name: {other}")),
    }
}

// Trustly can send more than the trailing four characters in `lastdigits`, so keep only the
// last four digits before propagating them as the bank account's last digits.
fn extract_bank_last_digits(lastdigits: &str) -> Option<Secret<String>> {
    let digits = lastdigits
        .chars()
        .rev()
        .take(BANK_LAST_DIGITS_LEN)
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    Some(Secret::new(digits))
}

pub fn extract_returned_bank_details(
    data: &TrustlyWebhookData,
) -> Option<PaymentMethodData<DefaultPCIHolder>> {
    let attributes = data.attributes.as_ref()?;

    let account_holder_name = attributes.name.clone();

    let bank_name = attributes.bank.as_ref().and_then(|bank| {
        map_trustly_bank_to_bank_name(bank.peek())
            .map_err(|error| {
                tracing::warn!(%error, "Failed to map Trustly bank to BankNames");
            })
            .ok()
    });

    let bank_last_digits = attributes
        .lastdigits
        .as_deref()
        .and_then(extract_bank_last_digits);

    let additional_details = data
        .accountid
        .clone()
        .map(|accountid| Secret::new(serde_json::json!({ "account_id": accountid })));

    if account_holder_name.is_none()
        && bank_last_digits.is_none()
        && additional_details.is_none()
    {
        return None;
    }

    Some(PaymentMethodData::<DefaultPCIHolder>::BankRedirect(
        BankRedirectData::Trustly {
            country: None,
            account_holder_name,
            bank_name,
            bank_last_digits,
            additional_details,
            connector_instrument_id: data.accountid.clone().map(Secret::new),
        },
    ))
}

pub fn verify_webhook_signature(
    webhook_body: TrustlyWebhookBody,
    public_key: Vec<u8>,
) -> error_stack::Result<bool, errors::WebhookError> {
    let method = webhook_body.method;
    let uuid = webhook_body.params.uuid;
    let data = &webhook_body.params.data;
    let signature = &webhook_body.params.signature;

    let pem_bytes = general_purpose::STANDARD
        .decode(&public_key)
        .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;

    let rsa = Rsa::public_key_from_pem(&pem_bytes)
        .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;
    let public_key = PKey::from_rsa(rsa)
        .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;

    let (algorithm, signature_b64) = if signature.len() >= 10
        && signature.starts_with("alg=RS")
        && matches!(signature.as_bytes().get(9), Some(b';'))
    {
        let prefix = &signature[..10];

        let algorithm = match prefix {
            "alg=RS256;" => Algorithm::SHA256,
            "alg=RS384;" => Algorithm::SHA384,
            "alg=RS512;" => Algorithm::SHA512,
            _ => Algorithm::SHA1,
        };

        (algorithm, &signature[10..])
    } else {
        (Algorithm::SHA1, signature.as_str())
    };

    let plaintext = format!("{}{}{}", method.as_str(), uuid, trustly_serialize(data));

    let signature_bytes = general_purpose::STANDARD
        .decode(signature_b64)
        .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

    let mut verifier = Verifier::new(algorithm.message_digest(), &public_key)
        .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;
    verifier
        .update(plaintext.as_bytes())
        .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;
    verifier
        .verify(&signature_bytes)
        .change_context(errors::WebhookError::WebhookSourceVerificationFailed)
}

pub const TRUSTLY_PAYOUT_MESSAGE_ID_PREFIX: &str = "payout_";

fn is_payout_message_id(message_id: &str) -> bool {
    message_id.starts_with(TRUSTLY_PAYOUT_MESSAGE_ID_PREFIX)
}

fn unexpected_webhook_event(
    event: &TrustlyWebhookMethod,
    flow: &'static str,
) -> error_stack::Report<errors::WebhookError> {
    error_stack::report!(errors::WebhookError::WebhookEventTypeNotFound).attach_printable(format!(
        "trustly: `{}` notification is not a {flow} event",
        event.as_str()
    ))
}

pub fn is_payment_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::Credit
            | TrustlyWebhookMethod::Debit
            | TrustlyWebhookMethod::Cancel
            | TrustlyWebhookMethod::Account
            | TrustlyWebhookMethod::Pending
    ) && !is_payout_message_id(message_id)
}

pub fn is_refund_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::PayoutConfirmation | TrustlyWebhookMethod::PayoutFailed
    ) && !is_payout_message_id(message_id)
}

pub fn is_payout_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::PayoutConfirmation
            | TrustlyWebhookMethod::PayoutFailed
            | TrustlyWebhookMethod::Credit
            | TrustlyWebhookMethod::Cancel
    ) && is_payout_message_id(message_id)
}

pub fn get_webhook_event(
    event: TrustlyWebhookMethod,
    message_id: String,
) -> domain_types::connector_types::EventType {
    match (event, !message_id.as_str().starts_with("payout_")) {
        (TrustlyWebhookMethod::Credit, true) => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        (TrustlyWebhookMethod::Credit, false) => {
            domain_types::connector_types::EventType::PayoutReversed
        }
        (TrustlyWebhookMethod::Debit, _) => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        (TrustlyWebhookMethod::Cancel, true) => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        (TrustlyWebhookMethod::Cancel, false) => {
            domain_types::connector_types::EventType::PayoutCancelled
        }
        (TrustlyWebhookMethod::Account, _) => {
            domain_types::connector_types::EventType::PaymentAssociatedDataUpdate
        }
        (TrustlyWebhookMethod::Pending, _) => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        (TrustlyWebhookMethod::PayoutConfirmation, true) => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        (TrustlyWebhookMethod::PayoutFailed, true) => {
            domain_types::connector_types::EventType::RefundFailure
        }
        (TrustlyWebhookMethod::PayoutConfirmation, false) => {
            domain_types::connector_types::EventType::PayoutSuccess
        }
        (TrustlyWebhookMethod::PayoutFailed, false) => {
            domain_types::connector_types::EventType::PayoutFailure
        }
    }
}

pub fn get_trustly_payment_webhook_status(
    event: &TrustlyWebhookMethod,
    message_id: &str,
) -> error_stack::Result<AttemptStatus, errors::WebhookError> {
    if !is_payment_webhook_event(event, message_id) {
        return Err(unexpected_webhook_event(event, "payment"));
    }

    match event {
        TrustlyWebhookMethod::Credit => Ok(AttemptStatus::Charged),
        TrustlyWebhookMethod::Debit => Ok(AttemptStatus::Failure),
        TrustlyWebhookMethod::Cancel => Ok(AttemptStatus::Voided),
        TrustlyWebhookMethod::Account | TrustlyWebhookMethod::Pending => Ok(AttemptStatus::Pending),
        TrustlyWebhookMethod::PayoutConfirmation | TrustlyWebhookMethod::PayoutFailed => {
            Err(unexpected_webhook_event(event, "payment"))
        }
    }
}

pub fn get_trustly_refund_webhook_status(
    event: &TrustlyWebhookMethod,
    message_id: &str,
) -> error_stack::Result<common_enums::RefundStatus, errors::WebhookError> {
    if !is_refund_webhook_event(event, message_id) {
        return Err(unexpected_webhook_event(event, "refund"));
    }

    match event {
        TrustlyWebhookMethod::PayoutConfirmation => Ok(common_enums::RefundStatus::Success),
        TrustlyWebhookMethod::PayoutFailed => Ok(common_enums::RefundStatus::Failure),
        TrustlyWebhookMethod::Credit
        | TrustlyWebhookMethod::Debit
        | TrustlyWebhookMethod::Cancel
        | TrustlyWebhookMethod::Account
        | TrustlyWebhookMethod::Pending => Err(unexpected_webhook_event(event, "refund")),
    }
}

pub fn get_trustly_payout_webhook_status(
    event: &TrustlyWebhookMethod,
    message_id: &str,
) -> error_stack::Result<common_enums::PayoutStatus, errors::WebhookError> {
    if !is_payout_webhook_event(event, message_id) {
        return Err(unexpected_webhook_event(event, "payout"));
    }

    match event {
        TrustlyWebhookMethod::PayoutConfirmation => Ok(common_enums::PayoutStatus::Success),
        TrustlyWebhookMethod::PayoutFailed => Ok(common_enums::PayoutStatus::Failure),
        TrustlyWebhookMethod::Credit => Ok(common_enums::PayoutStatus::Reversed),
        TrustlyWebhookMethod::Cancel => Ok(common_enums::PayoutStatus::Cancelled),
        TrustlyWebhookMethod::Debit
        | TrustlyWebhookMethod::Account
        | TrustlyWebhookMethod::Pending => Err(unexpected_webhook_event(event, "payout")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookResponse {
    pub result: TrustlyWebhookResponseResult,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookResponseResult {
    pub signature: Secret<String>,
    pub uuid: String,
    pub method: String,
    pub data: TrustlyWebhookResponseResultData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookResponseResultData {
    pub status: String,
}
