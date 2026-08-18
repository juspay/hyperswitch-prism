use super::TrustlyPayoutsRouterData;
use crate::types::ResponseRouterData;
use base64::{engine::general_purpose, Engine};
use common_utils::types::{StringMajorUnit, StringMajorUnitForConnector};
use domain_types::{
    connector_flow::{PayoutCreateRecipient, PayoutGet, PayoutTransfer},
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    payment_method_data::PaymentMethodDataTypes,
    payouts::{
        payout_method_data::{Bank, PayoutMethodData},
        payouts_types::{
            PayoutCreateRecipientRequest, PayoutCreateRecipientResponse, PayoutFlowData,
            PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest, PayoutTransferResponse,
        },
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils as domain_utils,
};
use error_stack::Report;
use hyperswitch_masking::{ExposeInterface, Secret};
use openssl::{hash::MessageDigest, pkey::PKey, rsa::Rsa, sign::Signer};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Debug};

// The Trustly error-response and webhook payload types are shared with the payment
// connector implementation.
use crate::connectors::trustly::transformers::{
    TrustlyErrorResponse, TrustlyWebhookBody, TrustlyWebhookMethod,
};

const TRUSTLY_VERSION: &str = "1.1";

// ===== AUTH TYPE =====

#[derive(Debug)]
pub struct TrustlyAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub private_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TrustlyAuthType {
    type Error = Report<IntegrationError>;
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
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// ===== SHARED TYPES =====

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum TrustlyMethod {
    RegisterAccount,
    AccountPayout,
    GetWithdrawals,
}

impl TrustlyMethod {
    fn as_str(&self) -> &'static str {
        match self {
            Self::RegisterAccount => "RegisterAccount",
            Self::AccountPayout => "AccountPayout",
            Self::GetWithdrawals => "GetWithdrawals",
        }
    }
}

// ===== REQUEST SIGNING =====
//
// Trustly signs every JSON-RPC method with RSA-SHA256 over `method + uuid` followed
// by a canonical serialization of the payload. The payment connector implements the
// same scheme for its own methods; it is duplicated here so the payout connector does
// not depend on the payment connector's internals.

/// Signature prefix Trustly expects for RSA-SHA256. The payout methods are always
/// signed with SHA256, so it is fixed rather than negotiated.
const TRUSTLY_SIGNATURE_PREFIX: &str = "alg=RS256;";

/// Canonical serialization used as the signature plaintext: object keys sorted,
/// null values dropped, and every scalar concatenated without separators.
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

fn trustly_serialize<T: Serialize>(data: &T) -> String {
    let value = serde_json::to_value(data).unwrap_or_default();
    serialize_value(&value)
}

fn generate_trustly_signature<T: Serialize>(
    method: &str,
    uuid: &str,
    data: &T,
    private_key: &str,
) -> Result<String, IntegrationError> {
    let encoding_failed = || IntegrationError::RequestEncodingFailed {
        context: Default::default(),
    };

    let pem =
        domain_utils::base64_decode(private_key.to_string()).map_err(|_| encoding_failed())?;
    let rsa = Rsa::private_key_from_pem(&pem).map_err(|_| encoding_failed())?;
    let private_key = PKey::from_rsa(rsa).map_err(|_| encoding_failed())?;

    let plaintext = format!("{}{}{}", method, uuid, trustly_serialize(data));

    let mut signer =
        Signer::new(MessageDigest::sha256(), &private_key).map_err(|_| encoding_failed())?;
    signer
        .update(plaintext.as_bytes())
        .map_err(|_| encoding_failed())?;
    let signature = signer.sign_to_vec().map_err(|_| encoding_failed())?;

    Ok(format!(
        "{TRUSTLY_SIGNATURE_PREFIX}{}",
        general_purpose::STANDARD.encode(&signature)
    ))
}

fn unsupported_payout_method_error(flow: &str) -> Report<IntegrationError> {
    IntegrationError::NotSupported {
        message: "Payout method is not supported".to_string(),
        connector: "Trustly",
        context: IntegrationErrorContext {
            additional_context: Some(format!(
                "Trustly {flow} - only the Trustly bank transfer payout method is supported"
            )),
            suggested_action: Some("Use a Trustly bank transfer payout method".to_string()),
            doc_url: None,
        },
    }
    .into()
}

fn missing_field(field_name: &'static str, flow: &str) -> Report<IntegrationError> {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            additional_context: Some(format!("Trustly {flow} - missing required field")),
            suggested_action: None,
            doc_url: None,
        },
    }
    .into()
}

#[derive(Debug, Deserialize)]
pub struct TrustlyAccountId {
    account_id: Secret<String>,
}

fn to_payout_connector_meta<T: serde::de::DeserializeOwned>(
    connector_meta: Option<serde_json::Value>,
) -> Result<T, Report<IntegrationError>> {
    let json = connector_meta
        .ok_or_else(|| missing_field("payout_connector_metadata", "Payout Transfer"))?;
    serde_json::from_value(json).map_err(|_| {
        IntegrationError::InvalidDataFormat {
            field_name: "payout_connector_metadata",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Trustly Payout Transfer - failed to parse payout_connector_metadata"
                        .to_string(),
                ),
                suggested_action: None,
                doc_url: None,
            },
        }
        .into()
    })
}

// ===== PAYOUT CREATE RECIPIENT (RegisterAccount) =====

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterAccountRequest {
    method: TrustlyMethod,
    params: RegisterAccountParams,
    version: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct RegisterAccountParams {
    data: RegisterAccountData,
    signature: Secret<String>,
    #[serde(rename = "UUID")]
    uuid: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct RegisterAccountData {
    account_number: Secret<String>,
    bank_number: Secret<String>,
    clearing_house: String,
    end_user_i_d: String,
    firstname: Secret<String>,
    lastname: Secret<String>,
    username: Secret<String>,
    password: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<RegisterAccountAttributes>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct RegisterAccountAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    address_country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mobile_phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<common_utils::pii::Email>,
}

/// Derive the payee's first and last name from the customer name or, failing that,
/// from the billing address.
fn get_recipient_names(
    req: &PayoutCreateRecipientRequest,
) -> Result<(Secret<String>, Secret<String>), Report<IntegrationError>> {
    let customer_name = req
        .customer
        .as_ref()
        .and_then(|c| c.name.clone())
        .map(Secret::new);

    if let (Some(first), Some(last)) = domain_utils::split_full_name(customer_name) {
        return Ok((first, last));
    }

    let billing_address = req.get_optional_billing_address();
    match (
        billing_address.and_then(|addr| addr.get_optional_first_name()),
        billing_address.and_then(|addr| addr.get_optional_last_name()),
    ) {
        (Some(first), Some(last)) => Ok((first, last)),
        _ => Err(missing_field(
            "customer.name / billing first_name and last_name",
            "Payout Create Recipient",
        )),
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustlyPayoutsRouterData<
            RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
            T,
        >,
    > for RegisterAccountRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TrustlyPayoutsRouterData<
            RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item.router_data;
        let trustly_data = match item.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Bank(Bank::Trustly(data))) => data,
            _ => return Err(unsupported_payout_method_error("Payout Create Recipient")),
        };

        let (account_number, bank_number) = if let Some(iban) = trustly_data.iban.clone() {
            (iban, Secret::new(String::new()))
        } else {
            (
                trustly_data.bank_account_number.clone().ok_or_else(|| {
                    missing_field("bank_account_number", "Payout Create Recipient")
                })?,
                trustly_data
                    .bank_number
                    .clone()
                    .ok_or_else(|| missing_field("bank_number", "Payout Create Recipient"))?,
            )
        };

        let (first_name, last_name) = get_recipient_names(&item.request)?;

        let end_user_id = item
            .request
            .customer
            .as_ref()
            .and_then(|c| c.merchant_customer_id.clone())
            .ok_or_else(|| {
                missing_field("customer.merchant_customer_id", "Payout Create Recipient")
            })?;

        let billing = item
            .request
            .address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref());
        let customer_email = item.request.customer.as_ref().and_then(|c| c.email.clone());

        let attributes = if billing.is_some() || customer_email.is_some() {
            let billing_address = item.request.get_optional_billing_address();
            Some(RegisterAccountAttributes {
                address_city: billing_address.and_then(|addr| addr.get_optional_city()),
                address_country: billing_address.and_then(|addr| addr.get_optional_country()),
                address_line1: billing_address.and_then(|addr| addr.get_optional_line1()),
                address_line2: billing_address.and_then(|addr| addr.get_optional_line2()),
                address_postal_code: billing_address.and_then(|addr| addr.get_optional_zip()),
                email: billing.and_then(|b| b.get_email().ok()),
                mobile_phone: billing.and_then(|b| b.get_phone_with_country_code().ok()),
            })
        } else {
            None
        };

        let auth = TrustlyAuthType::try_from(&item.connector_config)?;

        let register_account_data = RegisterAccountData {
            account_number,
            bank_number,
            clearing_house: common_enums::Country::from_alpha2(trustly_data.bank_country_code)
                .to_string()
                .to_uppercase(),
            end_user_i_d: end_user_id,
            firstname: first_name,
            lastname: last_name,
            username: auth.username.clone(),
            password: auth.password.clone(),
            attributes,
        };

        let uuid = uuid::Uuid::new_v4().to_string();
        let signature = generate_trustly_signature(
            TrustlyMethod::RegisterAccount.as_str(),
            uuid.as_str(),
            &register_account_data,
            &auth.private_key.clone().expose(),
        )
        .map_err(Report::new)?;

        Ok(Self {
            method: TrustlyMethod::RegisterAccount,
            params: RegisterAccountParams {
                data: register_account_data,
                signature: Secret::new(signature),
                uuid,
            },
            version: TRUSTLY_VERSION.to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum RegisterAccountResponse {
    Success(RegisterAccountResponseSuccess),
    Error(Box<TrustlyErrorResponse>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterAccountResponseSuccess {
    pub result: RegisterAccountResponseResult,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterAccountResponseResult {
    data: RegisterAccountResponseResultData,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterAccountResponseResultData {
    accountid: Secret<String>,
    clearinghouse: String,
    bank: String,
}

impl TryFrom<ResponseRouterData<RegisterAccountResponse, Self>>
    for RouterDataV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RegisterAccountResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            RegisterAccountResponse::Success(response) => {
                let account_id = response.result.data.accountid;
                let payout_connector_metadata = Some(Secret::new(serde_json::json!({
                    "account_id": account_id,
                })));
                Ok(Self {
                    response: Ok(PayoutCreateRecipientResponse {
                        merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                        payout_status: common_enums::PayoutStatus::RequiresCreation,
                        connector_payout_id: None,
                        status_code: item.http_code,
                        payout_connector_metadata,
                    }),
                    ..item.router_data
                })
            }
            RegisterAccountResponse::Error(error_response) => Ok(Self {
                response: Err(build_error_from_response(&error_response, item.http_code)),
                ..item.router_data
            }),
        }
    }
}

// ===== PAYOUT TRANSFER (AccountPayout) =====

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AccountPayoutRequest {
    method: TrustlyMethod,
    params: AccountPayoutParams,
    version: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct AccountPayoutParams {
    signature: Secret<String>,
    #[serde(rename = "UUID")]
    uuid: String,
    data: AccountPayoutData,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct AccountPayoutData {
    account_i_d: Secret<String>,
    amount: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<AccountPayoutAttributes>,
    currency: common_enums::Currency,
    end_user_i_d: String,
    message_i_d: String,
    notification_u_r_l: String,
    password: Secret<String>,
    username: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct AccountPayoutAttributes {
    shopper_statement: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustlyPayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            T,
        >,
    > for AccountPayoutRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TrustlyPayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item.router_data;
        // The Trustly payout only makes sense for the Trustly bank transfer method.
        match item.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Bank(Bank::Trustly(_))) => {}
            _ => return Err(unsupported_payout_method_error("Payout Transfer")),
        }

        // The account id created by the RegisterAccount (recipient) step is
        // carried over in payout_connector_metadata.
        let metadata = item
            .request
            .payout_connector_metadata
            .clone()
            .map(|secret| secret.expose());
        let account_id: TrustlyAccountId = to_payout_connector_meta(metadata)?;

        let amount = domain_utils::convert_amount(
            &StringMajorUnitForConnector,
            item.request.amount,
            item.request.source_currency,
        )?;

        let notification_url = item
            .request
            .webhook_url
            .clone()
            .ok_or_else(|| missing_field("webhook_url", "Payout Transfer"))?;

        let shopper_statement = item
            .resource_common_data
            .description
            .clone()
            .ok_or_else(|| missing_field("description", "Payout Transfer"))?;

        let end_user_id = item
            .request
            .get_customer_id()?
            .get_string_repr()
            .to_string();

        let auth = TrustlyAuthType::try_from(&item.connector_config)?;

        let account_payout_data = AccountPayoutData {
            account_i_d: account_id.account_id,
            amount,
            attributes: Some(AccountPayoutAttributes { shopper_statement }),
            currency: item.request.destination_currency,
            end_user_i_d: end_user_id,
            message_i_d: format!("payout_{}", item.resource_common_data.payout_id),
            notification_u_r_l: notification_url,
            password: auth.password.clone(),
            username: auth.username.clone(),
        };

        let uuid = uuid::Uuid::new_v4().to_string();
        let signature = generate_trustly_signature(
            TrustlyMethod::AccountPayout.as_str(),
            uuid.as_str(),
            &account_payout_data,
            &auth.private_key.clone().expose(),
        )
        .map_err(Report::new)?;

        Ok(Self {
            method: TrustlyMethod::AccountPayout,
            params: AccountPayoutParams {
                data: account_payout_data,
                signature: Secret::new(signature),
                uuid,
            },
            version: TRUSTLY_VERSION.to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum PayoutResult {
    #[serde(rename = "0")]
    Failed,
    #[serde(rename = "1")]
    Pending,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AccountPayoutResponse {
    Success(AccountPayoutResponseSuccess),
    Error(Box<TrustlyErrorResponse>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AccountPayoutResponseSuccess {
    version: String,
    result: AccountPayoutResponseResult,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AccountPayoutResponseResult {
    data: AccountPayoutResponseData,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AccountPayoutResponseData {
    orderid: String,
    result: PayoutResult,
}

impl TryFrom<ResponseRouterData<AccountPayoutResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AccountPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            AccountPayoutResponse::Success(success_response) => {
                let data = success_response.result.data;
                let payout_status = match data.result {
                    PayoutResult::Failed => common_enums::PayoutStatus::Failure,
                    PayoutResult::Pending => common_enums::PayoutStatus::Initiated,
                };
                Ok(Self {
                    response: Ok(PayoutTransferResponse {
                        merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                        payout_status,
                        connector_payout_id: Some(data.orderid),
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            AccountPayoutResponse::Error(error_response) => Ok(Self {
                response: Err(build_error_from_response(&error_response, item.http_code)),
                ..item.router_data
            }),
        }
    }
}

// ===== PAYOUT GET (GetWithdrawals) =====

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyPayoutSyncRequest {
    method: TrustlyMethod,
    params: PayoutSyncRequestParams,
    version: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct PayoutSyncRequestParams {
    #[serde(rename = "UUID")]
    uuid: String,
    data: PayoutSyncRequestData,
    signature: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct PayoutSyncRequestData {
    order_id: Secret<String>,
    password: Secret<String>,
    username: Secret<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TrustlyPayoutsRouterData<
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
            T,
        >,
    > for TrustlyPayoutSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TrustlyPayoutsRouterData<
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item.router_data;
        let auth = TrustlyAuthType::try_from(&item.connector_config)?;

        let order_id = item
            .request
            .connector_payout_id
            .clone()
            .ok_or_else(|| missing_field("connector_payout_id", "Payout Get"))?;

        let data = PayoutSyncRequestData {
            order_id: Secret::new(order_id),
            password: auth.password.clone(),
            username: auth.username.clone(),
        };

        let uuid = uuid::Uuid::new_v4().to_string();
        let signature = generate_trustly_signature(
            TrustlyMethod::GetWithdrawals.as_str(),
            uuid.as_str(),
            &data,
            &auth.private_key.clone().expose(),
        )
        .map_err(Report::new)?;

        Ok(Self {
            method: TrustlyMethod::GetWithdrawals,
            params: PayoutSyncRequestParams {
                uuid,
                data,
                signature: Secret::new(signature),
            },
            version: TRUSTLY_VERSION.to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TrustlyPayoutSyncResponse {
    Success(TrustlyPayoutSyncResponseSuccess),
    Error(Box<TrustlyErrorResponse>),
    Webhook(Box<TrustlyWebhookBody>),
}

fn get_payout_status_from_webhook(
    method: TrustlyWebhookMethod,
) -> Result<common_enums::PayoutStatus, Report<ConnectorError>> {
    match method {
        TrustlyWebhookMethod::Credit => Ok(common_enums::PayoutStatus::Reversed),
        TrustlyWebhookMethod::Cancel => Ok(common_enums::PayoutStatus::Cancelled),
        TrustlyWebhookMethod::PayoutFailed => Ok(common_enums::PayoutStatus::Failure),
        TrustlyWebhookMethod::PayoutConfirmation => Ok(common_enums::PayoutStatus::Success),
        _ => Err(ConnectorError::UnexpectedResponseError {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(
                    "Trustly GetWithdrawals - unexpected webhook method in sync response"
                        .to_string(),
                ),
            },
        }
        .into()),
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyPayoutSyncResponseSuccess {
    result: TrustlyPayoutSyncResponseResult,
    version: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyPayoutSyncResponseResult {
    uuid: String,
    method: String,
    data: Vec<TrustlyPayoutSyncResponseData>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TrustlyPayoutSyncResponseData {
    reference: String,
    orderid: String,
    transferstate: TrustlyPayoutStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum TrustlyPayoutStatus {
    Confirmed,
    Executing,
    Executed,
    Pending,
    Queued,
    Preparing,
    Prepared,
    Bounced,
    Error,
    Failed,
    Returned,
}

impl From<TrustlyPayoutStatus> for common_enums::PayoutStatus {
    fn from(item: TrustlyPayoutStatus) -> Self {
        match item {
            TrustlyPayoutStatus::Confirmed => Self::Success,
            TrustlyPayoutStatus::Failed
            | TrustlyPayoutStatus::Error
            | TrustlyPayoutStatus::Bounced
            | TrustlyPayoutStatus::Returned => Self::Failure,
            TrustlyPayoutStatus::Executing | TrustlyPayoutStatus::Executed => Self::Pending,
            TrustlyPayoutStatus::Pending
            | TrustlyPayoutStatus::Queued
            | TrustlyPayoutStatus::Preparing
            | TrustlyPayoutStatus::Prepared => Self::Initiated,
        }
    }
}

impl TryFrom<ResponseRouterData<TrustlyPayoutSyncResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustlyPayoutSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            TrustlyPayoutSyncResponse::Success(response) => {
                let (payout_status, connector_payout_id) = match response.result.data.first() {
                    Some(first) => (
                        common_enums::PayoutStatus::from(first.transferstate.clone()),
                        Some(first.orderid.clone()),
                    ),
                    None => (
                        common_enums::PayoutStatus::Pending,
                        item.router_data.request.connector_payout_id.clone(),
                    ),
                };
                Ok(Self {
                    response: Ok(PayoutGetResponse {
                        merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                        payout_status,
                        connector_payout_id,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            TrustlyPayoutSyncResponse::Error(error_response) => Ok(Self {
                response: Err(build_error_from_response(&error_response, item.http_code)),
                ..item.router_data
            }),
            TrustlyPayoutSyncResponse::Webhook(webhook_body) => {
                let payout_status = get_payout_status_from_webhook(webhook_body.method.clone())?;
                Ok(Self {
                    response: Ok(PayoutGetResponse {
                        merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                        payout_status,
                        connector_payout_id: Some(webhook_body.params.data.orderid.clone()),
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

// ===== ERROR HANDLING =====

/// Build a domain `ErrorResponse` from a Trustly JSON-RPC error body embedded in
/// an otherwise `2xx`/untagged response.
fn build_error_from_response(
    error_response: &TrustlyErrorResponse,
    status_code: u16,
) -> domain_types::router_data::ErrorResponse {
    let typed = crate::connectors::macros::serialize_typed_connector_payload(
        error_response,
        "typed_connector_response",
    );
    domain_types::router_data::ErrorResponse {
        code: error_response.error.code.to_string(),
        message: error_response.error.message.clone(),
        reason: Some(error_response.error.message.clone()),
        status_code,
        attempt_status: None,
        connector_transaction_id: Some(error_response.error.error.uuid.clone()),
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: typed,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}
