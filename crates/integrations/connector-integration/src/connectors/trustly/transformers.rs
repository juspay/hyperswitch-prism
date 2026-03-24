use crate::{connectors::trustly::TrustlyRouterData, types::ResponseRouterData, utils};
use base64::{engine::general_purpose, Engine};
use common_enums::{self, AttemptStatus, CountryAlpha2, Currency};
use common_utils::{
    pii::{self, IpAddress},
    request::Method,
    CustomerId, StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, RefundFlowData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{BankRedirectData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::{base64_decode, convert_amount},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use openssl::{
    hash::MessageDigest,
    pkey::PKey,
    rsa::Rsa,
    sign::{Signer, Verifier},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const TRUSTLY_VERSION: &str = "1.1";
const TRUSTLY_PUBLIC_KEY_TEST: &str = "LS0tLS1CRUdJTiBQVUJMSUMgS0VZLS0tLS0KTUlJQklqQU5CZ2txaGtpRzl3MEJBUUVGQUFPQ0FROEFNSUlCQ2dLQ0FRRUF5N2gveVg4REVBMm01ODhTcld5ZQpBQzhyVE1iRXJ3SHQyaG9UaVA5ZnRlL2lPbzBGWElaU21Oc051NDIyTCtpSnl2WlF1MTllYmVMN1hnQjBVWHF0CnpBNkt0WEJNWElLd3VNQ1poYmRlUjhzYjdPS2JYMm5sV00rZTJIbXJyOUNUZmtaa0ZCZVNDK2lOOWZBVTZQb1IKWDBpNVBXbTB1Wm5hb1dYY1puazVDeFFDZ25mWWdzeDd4c2Q4QXUrbXJxRThTSGVUOHppL0ludzBYcDZiYTI1RwpZc1poSGZJUEQycmNaUU9wV2JtSFJTNEprNGFHelNPQkhiQVpoS2xQOTdQeG9WZlVjUEkzaUNBMSszak1zMWwyClBZc0hVYlA2ME5NVndrR1BqRk9UdjRtMWExd0tzdWUwbWhzcERkdnN3WlVlS0UrUE9HT3Vld3FUUUorZ0loWHcKbVFJREFRQUIKLS0tLS1FTkQgUFVCTElDIEtFWS0tLS0t";
const TRUSTLY_PUBLIC_KEY_LIVE: &str = "LS0tLS1CRUdJTiBQVUJMSUMgS0VZLS0tLS0KTUlJQklqQU5CZ2txaGtpRzl3MEJBUUVGQUFPQ0FROEFNSUlCQ2dLQ0FRRUFvWmhucWlFTGVvWDNRTlNnN2pwVQprYkxWNEJVMzJMb1NNdUFCQWFQZHhocFphY2NGWXVkMno0UVVsTXEvajQ2dmRWRHBhQ0ZhQ1orcU5UNSt0SGJRCkJGZ2NyeDgydTdyK2FNSHZLeTRGRWN6VDVhZXYwTnhSbFFLSG1OUXlndnAzaE5rcWVPdzRuSnkzUG9ENGNnQ3AKU2xMVGlQT0J5MlpzV1VIUXBTVkpkRFVpTHdBUWZOVjkwak1xYTN6cTFuVGZtVEJtZDZOUjFYQWpnNWVTNlNXcgp0bzFuVlMxYjdYS0d2N0NjMWt0MFJWZDU0dFdxb0NNREh3RWlVMHN0NjZCQ0tkWWszcjV3b0RaeEdaVVVqVmRtCmc5TzJ4cHFSUkRjZEpHbThISU9WSEdTTlQ5UjdMTXVjSC9QR3dyZnBkV21CRGp5MEJrdURsc3N1QmdoNzMxbDIKY3dJREFRQUIKLS0tLS1FTkQgUFVCTElDIEtFWS0tLS0t";

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyAuthType {
    pub(super) username: Secret<String>,
    pub(super) password: Secret<String>,
    pub(super) private_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TrustlyAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;
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
            _ => Err(errors::ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

//Metadata
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyMetadata {
    private_key: Secret<String>,
}

impl TryFrom<&Option<pii::SecretSerdeValue>> for TrustlyMetadata {
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(meta_data: &Option<pii::SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata: Self = utils::to_connector_meta_from_secret::<Self>(meta_data.clone())
            .change_context(errors::ConnectorError::InvalidConnectorConfig {
                config: "metadata",
            })?;
        Ok(metadata)
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
    end_user_i_d: CustomerId,
    message_i_d: String,
    notification_u_r_l: String,
    password: Secret<String>,
    username: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustlyPaymentRequestAttributes {
    amount: StringMajorUnit,
    country: CountryAlpha2,
    currency: Currency,
    email: pii::Email,
    fail_u_r_l: String,
    firstname: Secret<String>,
    i_p: Option<Secret<String, IpAddress>>,
    lastname: Secret<String>,
    locale: String,
    mobile: Option<Secret<String>>,
    shipping_address_city: Option<Secret<String>>,
    shipping_address_country: Option<CountryAlpha2>,
    shipping_address_line1: Option<Secret<String>>,
    shipping_address_line2: Option<Secret<String>>,
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

fn generate_trustly_signature<T: Serialize>(
    method: &TrustlyMethod,
    uuid: &str,
    data: &T,
    private_key: &str,
) -> Result<String, errors::ConnectorError> {
    let algorithm = Algorithm::SHA256;
    let pem = base64_decode(private_key.to_string())
        .map_err(|_| errors::ConnectorError::RequestEncodingFailed)?;
    let rsa = Rsa::private_key_from_pem(&pem)
        .map_err(|_| errors::ConnectorError::RequestEncodingFailed)?;
    let private_key =
        PKey::from_rsa(rsa).map_err(|_| errors::ConnectorError::RequestEncodingFailed)?;

    let plaintext = format!("{}{}{}", method.as_str(), uuid, trustly_serialize(data));

    let mut signer = Signer::new(algorithm.message_digest(), &private_key)
        .map_err(|_| errors::ConnectorError::RequestEncodingFailed)?;
    signer
        .update(plaintext.as_bytes())
        .map_err(|_| errors::ConnectorError::RequestEncodingFailed)?;
    let signature = signer
        .sign_to_vec()
        .map_err(|_| errors::ConnectorError::RequestEncodingFailed)?;

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
    type Error = error_stack::Report<errors::ConnectorError>;
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
            PaymentMethodData::BankRedirect(BankRedirectData::Trustly { .. }) => {
                let auth_details = TrustlyAuthType::try_from(&item.router_data.connector_config)?;

                let return_url = item
                    .router_data
                    .resource_common_data
                    .return_url
                    .clone()
                    .ok_or(errors::ConnectorError::MissingRequiredField {
                        field_name: "return_url",
                    })?;
                let uuid = uuid::Uuid::new_v4().to_string();
                let attributes = TrustlyPaymentRequestAttributes {
                    amount: convert_amount(
                        item.connector.amount_converter,
                        item.router_data.request.minor_amount,
                        item.router_data.request.currency,
                    )?,
                    country: item
                        .router_data
                        .resource_common_data
                        .get_billing_country()
                        .unwrap_or(
                            item.router_data
                                .resource_common_data
                                .get_optional_shipping_country()
                                .ok_or(errors::ConnectorError::MissingRequiredField {
                                    field_name: "country",
                                })?,
                        ),
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
                        .ok_or(errors::ConnectorError::MissingRequiredField {
                            field_name: "email",
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
                        .ok_or(errors::ConnectorError::MissingRequiredField {
                            field_name: "locale",
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
                    end_user_i_d: item.router_data.request.get_customer_id()?,
                    message_i_d: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id,
                    notification_u_r_l: item.router_data.request.webhook_url.clone().ok_or(
                        errors::ConnectorError::MissingRequiredField {
                            field_name: "webhook_url",
                        },
                    )?,
                    password: auth_details.password.clone(),
                    username: auth_details.username.clone(),
                };

                let signature = generate_trustly_signature(
                    &TrustlyMethod::Deposit,
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
            | PaymentMethodData::CardToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::MobilePayment(_) => Err(errors::ConnectorError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Truelayer"),
            )
            .into()),
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
                        connector_response_reference_id: Some(response.result.uuid),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
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
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: TrustlyRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth_details = TrustlyAuthType::try_from(&item.router_data.connector_config)?;
        let uuid = uuid::Uuid::new_v4().to_string();
        let attributes = Some(TrustlyRefundAttributes {
            external_reference: item
                .router_data
                .resource_common_data
                .refund_id
                .clone()
                .ok_or(errors::ConnectorError::MissingRequiredField {
                    field_name: "refund_id",
                })?,
        });
        let data = TrustlyRefundRequestData {
            amount: convert_amount(
                item.connector.amount_converter,
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )?,
            attributes,
            currency: item.router_data.request.currency,
            order_i_d: item.router_data.request.connector_transaction_id.clone(),
            password: auth_details.password,
            username: auth_details.username,
        };

        let signature = generate_trustly_signature(
            &TrustlyMethod::Refund,
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
                    connector_refund_id: response.result.uuid,
                    refund_status: common_enums::RefundStatus::from(response.result.data.result),
                    status_code: item.http_code,
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
    pub accountid: Option<Secret<String>>,
    pub verified: Option<String>,
    pub notificationid: String,
    pub timestamp: Option<String>,
    pub attributes: Option<TrustlyWebhookAttributes>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookAttributes {
    pub bank: Secret<String>,
    pub city: Secret<String>,
    pub name: Secret<String>,
    pub address: Secret<String>,
    pub zipcode: Secret<String>,
    pub personid: Secret<String>,
    pub descriptor: String,
    pub lastdigits: String,
    pub clearinghouse: String,
}

pub fn verify_webhook_signature(
    webhook_body: TrustlyWebhookBody,
) -> error_stack::Result<bool, errors::ConnectorError> {
    let public_key = match std::env::var("ROUTER_ENV") {
        Ok(val) if val.eq_ignore_ascii_case("production") => TRUSTLY_PUBLIC_KEY_LIVE,
        _ => TRUSTLY_PUBLIC_KEY_TEST,
    };

    let method = webhook_body.method;
    let uuid = webhook_body.params.uuid;
    let data = &webhook_body.params.data;
    let signature = &webhook_body.params.signature;

    let pem_bytes = general_purpose::STANDARD
        .decode(public_key.as_bytes())
        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

    let rsa = Rsa::public_key_from_pem(&pem_bytes)
        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;
    let public_key = PKey::from_rsa(rsa)
        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

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
        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

    let mut verifier = Verifier::new(algorithm.message_digest(), &public_key)
        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;
    verifier
        .update(plaintext.as_bytes())
        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;
    verifier
        .verify(&signature_bytes)
        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)
}

pub fn get_webhook_event(event: TrustlyWebhookMethod) -> domain_types::connector_types::EventType {
    match event {
        TrustlyWebhookMethod::Credit => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        TrustlyWebhookMethod::Debit | TrustlyWebhookMethod::Cancel => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        TrustlyWebhookMethod::Account | TrustlyWebhookMethod::Pending => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
    }
}

pub fn get_trustly_payment_webhook_status(event: &TrustlyWebhookMethod) -> AttemptStatus {
    match event {
        TrustlyWebhookMethod::Credit => AttemptStatus::Charged,
        TrustlyWebhookMethod::Debit | TrustlyWebhookMethod::Cancel => AttemptStatus::Failure,
        TrustlyWebhookMethod::Account | TrustlyWebhookMethod::Pending => AttemptStatus::Pending,
    }
}
