//! Citigate transformers.
//!
//! Citigate exposes a single-endpoint, verb-in-body JSON API. Every operation is a
//! `POST` to `/orion/interface/json.ashx`; the operation is selected by the
//! `TransTypeID` body field. Authentication (`MerchantName` / `MerchantPassword`) is
//! also carried in the body, so there are no auth headers.
//!
//! Scope of this module: Card / Authorize (Purchase, `TransTypeID = 0`), one-time,
//! non-3DS.

use common_enums::{AttemptStatus, CardNetwork};
use common_utils::{pii::Email, types::StringMinorUnit};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    utils::{get_card_issuer, CardIssuer},
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Deserializer, Serialize};

use crate::connectors::citigate::{CitigateAmountConvertor, CitigateRouterData};
use crate::types::ResponseRouterData;

/// `PaymentTypeID` for a card payment. Card is the only payment type in scope.
const PAYMENT_TYPE_ID_CARD: &str = "1";
/// `TransTypeID` for a purchase (UCS `Authorize`).
const TRANS_TYPE_ID_PURCHASE: &str = "0";

/// `ResponseCode` returned for an approved transaction.
const RESPONSE_CODE_APPROVED: &str = "0";
/// `ResponseCode` returned when a cardholder redirect (3DS) is required.
const RESPONSE_CODE_REDIRECT_REQUIRED: &str = "600";
/// Undocumented `ResponseCode` observed on Status Check responses; discriminated on
/// the response `TransTypeID`.
const RESPONSE_CODE_UNDOCUMENTED: &str = "999";

/// Response `TransTypeID` values (Transaction Key 4).
const RESP_TRANS_TYPE_SALE: &str = "1";
const RESP_TRANS_TYPE_AUTHORISE: &str = "2";
const RESP_TRANS_TYPE_CAPTURE: &str = "3";
const RESP_TRANS_TYPE_PENDING: &str = "6";

/// `TransactionID` value Citigate returns when no transaction was created.
const NO_TRANSACTION_ID: &str = "0";

// =============================================================================
// AUTH
// =============================================================================

/// Citigate body-key credentials.
///
/// `api_key` carries `MerchantName` and `key1` carries `MerchantPassword`; both are
/// injected into every request body rather than into headers.
#[derive(Debug, Clone)]
pub struct CitigateAuthType {
    pub merchant_name: Secret<String>,
    pub merchant_password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for CitigateAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Citigate { api_key, key1, .. } => Ok(Self {
                merchant_name: api_key.to_owned(),
                merchant_password: key1.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// =============================================================================
// CARD BRAND
// =============================================================================

/// The `Brand` values Citigate accepts. Any other network is rejected before the
/// request is built rather than being sent and refused with `ResponseCode` 567.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum CitigateBrand {
    #[serde(rename = "VISA")]
    Visa,
    #[serde(rename = "MASTERCARD")]
    Mastercard,
    #[serde(rename = "AMEX")]
    Amex,
    #[serde(rename = "DINERS")]
    Diners,
    #[serde(rename = "MAESTRO")]
    Maestro,
}

fn unsupported_brand(detail: String) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: detail,
        connector: "citigate",
        context: IntegrationErrorContext::default(),
    })
}

/// Resolve the Citigate `Brand` from the supplied card network, falling back to BIN
/// detection when the network is not provided by the caller.
fn get_citigate_brand<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<CitigateBrand, error_stack::Report<IntegrationError>> {
    if let Some(network) = card.card_network.as_ref() {
        return match network {
            CardNetwork::Visa => Ok(CitigateBrand::Visa),
            CardNetwork::Mastercard => Ok(CitigateBrand::Mastercard),
            CardNetwork::AmericanExpress => Ok(CitigateBrand::Amex),
            CardNetwork::DinersClub => Ok(CitigateBrand::Diners),
            CardNetwork::Maestro => Ok(CitigateBrand::Maestro),
            other => Err(unsupported_brand(format!("Card network {other:?}"))),
        };
    }

    match get_card_issuer(card.card_number.peek())? {
        CardIssuer::Visa => Ok(CitigateBrand::Visa),
        CardIssuer::Master => Ok(CitigateBrand::Mastercard),
        CardIssuer::AmericanExpress => Ok(CitigateBrand::Amex),
        CardIssuer::DinersClub => Ok(CitigateBrand::Diners),
        CardIssuer::Maestro => Ok(CitigateBrand::Maestro),
        other => Err(unsupported_brand(format!("Card issuer {other:?}"))),
    }
}

// =============================================================================
// REQUEST
// =============================================================================

/// Purchase request (`TransTypeID = 0`).
///
/// Citigate's field names are neither `camelCase` nor plain `PascalCase`
/// (`CardNo`, `CVV`, `UserIP`, `StreetLine1`, ...), so every field carries an
/// explicit `#[serde(rename = ...)]`.
#[derive(Debug, Serialize)]
pub struct CitigatePaymentsRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "PaymentTypeID")]
    pub payment_type_id: String,
    #[serde(rename = "TransTypeID")]
    pub trans_type_id: String,
    #[serde(rename = "MerchantName")]
    pub merchant_name: Secret<String>,
    #[serde(rename = "MerchantPassword")]
    pub merchant_password: Secret<String>,
    #[serde(rename = "MerchantRef")]
    pub merchant_ref: String,
    #[serde(rename = "Currency")]
    pub currency: common_enums::Currency,
    #[serde(rename = "Amount")]
    pub amount: StringMinorUnit,
    #[serde(rename = "Brand")]
    pub brand: CitigateBrand,
    #[serde(rename = "CardholderName")]
    pub cardholder_name: Secret<String>,
    #[serde(rename = "CardNo")]
    pub card_no: RawCardNumber<T>,
    #[serde(rename = "ExpiryYear")]
    pub expiry_year: Secret<String>,
    #[serde(rename = "ExpiryMonth")]
    pub expiry_month: Secret<String>,
    #[serde(rename = "CVV")]
    pub cvv: Secret<String>,
    #[serde(rename = "Firstname")]
    pub firstname: Secret<String>,
    #[serde(rename = "Surname")]
    pub surname: Secret<String>,
    #[serde(rename = "StreetLine1")]
    pub street_line1: Secret<String>,
    #[serde(rename = "StreetLine2", skip_serializing_if = "Option::is_none")]
    pub street_line2: Option<Secret<String>>,
    #[serde(rename = "City")]
    pub city: Secret<String>,
    #[serde(rename = "PostalCode")]
    pub postal_code: Secret<String>,
    #[serde(rename = "StateProvince", skip_serializing_if = "Option::is_none")]
    pub state_province: Option<Secret<String>>,
    #[serde(rename = "Country")]
    pub country: common_enums::CountryAlpha2,
    #[serde(rename = "Email")]
    pub email: Email,
    #[serde(rename = "Telephone", skip_serializing_if = "Option::is_none")]
    pub telephone: Option<Secret<String>>,
    #[serde(rename = "UserIP")]
    pub user_ip: Secret<String>,
}

type AuthorizeRouterData<T> =
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<CitigateRouterData<AuthorizeRouterData<T>, T>> for CitigatePaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: CitigateRouterData<AuthorizeRouterData<T>, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by citigate".to_string(),
                    IntegrationErrorContext::default(),
                )))
            }
        };

        let auth = CitigateAuthType::try_from(&router_data.connector_config)?;
        let common = &router_data.resource_common_data;

        let amount = CitigateAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        let firstname = common.get_billing_first_name()?;
        let surname = common.get_billing_last_name()?;
        let cardholder_name = match card.get_optional_cardholder_name() {
            Some(name) => name,
            None => Secret::new(format!("{} {}", firstname.peek(), surname.peek())),
        };

        let email = match router_data.request.email.clone() {
            Some(email) => email,
            None => common.get_billing_email()?,
        };

        // `UserIP` is documented as mandatory (`Y`). Fail loudly rather than let the
        // gateway reject the transaction with ResponseCode 535.
        let user_ip = router_data.request.get_ip_address()?;

        Ok(Self {
            payment_type_id: PAYMENT_TYPE_ID_CARD.to_string(),
            trans_type_id: TRANS_TYPE_ID_PURCHASE.to_string(),
            merchant_name: auth.merchant_name,
            merchant_password: auth.merchant_password,
            merchant_ref: common.connector_request_reference_id.clone(),
            currency: router_data.request.currency,
            amount,
            brand: get_citigate_brand(card)?,
            cardholder_name,
            card_no: card.card_number.clone(),
            expiry_year: card.get_expiry_year_4_digit(),
            expiry_month: card.get_card_expiry_month_2_digit()?,
            cvv: card.card_cvc.clone(),
            firstname,
            surname,
            street_line1: common.get_billing_line1()?,
            street_line2: common.get_optional_billing_line2(),
            city: common.get_billing_city()?,
            postal_code: common.get_billing_zip()?,
            state_province: common.get_optional_billing_state(),
            country: common.get_billing_country()?,
            email,
            telephone: common.get_optional_billing_phone_number(),
            user_ip: Secret::new(user_ip.peek().to_string()),
        })
    }
}

// =============================================================================
// RESPONSE
// =============================================================================

/// Citigate's .NET JSON serializer emits an **empty array** (`[]`) for empty string
/// fields, and quotes numeric fields inconsistently. This collapses string, number
/// and `[]`/`null` forms into `Option<String>`; a plain `Option<String>` would fail
/// to deserialize `[]`.
fn deserialize_tolerant_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(serde_json::Value::String(string)) if !string.is_empty() => Some(string),
        Some(serde_json::Value::Number(number)) => Some(number.to_string()),
        Some(serde_json::Value::Bool(boolean)) => Some(boolean.to_string()),
        // `[]`, `{}`, `""`, `null` and absent all mean "no value".
        _ => None,
    })
}

/// The single response envelope Citigate returns for every flow and every outcome
/// (approval, bank decline and gateway rejection all arrive as HTTP 200).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitigatePaymentsResponse {
    #[serde(
        rename = "TransactionID",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub transaction_id: Option<String>,
    #[serde(
        rename = "MerchantRef",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub merchant_ref: Option<String>,
    #[serde(
        rename = "TransTypeID",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub trans_type_id: Option<String>,
    #[serde(
        rename = "Currency",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub currency: Option<String>,
    #[serde(
        rename = "Amount",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub amount: Option<String>,
    #[serde(
        rename = "BusinessCase",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub business_case: Option<String>,
    #[serde(
        rename = "Descriptor",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub descriptor: Option<String>,
    #[serde(
        rename = "Bank",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub bank: Option<String>,
    #[serde(
        rename = "ResponseCode",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub response_code: Option<String>,
    #[serde(
        rename = "ResponseDescription",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub response_description: Option<String>,
    #[serde(
        rename = "BankCode",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub bank_code: Option<String>,
    #[serde(
        rename = "BankDescription",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub bank_description: Option<String>,
    #[serde(
        rename = "RedirectURL",
        default,
        deserialize_with = "deserialize_tolerant_string"
    )]
    pub redirect_url: Option<String>,
}

impl CitigatePaymentsResponse {
    /// `ResponseCode` is the primary status driver; the response `TransTypeID`
    /// refines an approval into authorized vs. charged.
    fn response_code(&self) -> &str {
        self.response_code.as_deref().unwrap_or_default()
    }

    fn trans_type_id(&self) -> &str {
        self.trans_type_id.as_deref().unwrap_or_default()
    }

    /// `true` when the gateway approved the transaction, or reported it as still
    /// pending (`999` + `TransTypeID 6`).
    fn is_success(&self) -> bool {
        match self.response_code() {
            RESPONSE_CODE_APPROVED => true,
            RESPONSE_CODE_UNDOCUMENTED => self.trans_type_id() == RESP_TRANS_TYPE_PENDING,
            _ => false,
        }
    }

    /// Status for an approved / pending Authorize response.
    fn attempt_status(&self) -> AttemptStatus {
        match self.response_code() {
            RESPONSE_CODE_APPROVED => match self.trans_type_id() {
                // Bank supports sales only: authorized *and* captured in one step.
                RESP_TRANS_TYPE_SALE | RESP_TRANS_TYPE_CAPTURE => AttemptStatus::Charged,
                // Pre-auth performed; capture outstanding (Citigate auto-captures
                // open auths after 48-96h).
                RESP_TRANS_TYPE_AUTHORISE => AttemptStatus::Authorized,
                _ => AttemptStatus::Pending,
            },
            _ => AttemptStatus::Pending,
        }
    }

    /// Status attached to a failed Authorize response.
    fn failure_status(&self) -> AttemptStatus {
        match self.response_code() {
            // Redirect (3DS) is out of scope for this integration: surface it as an
            // authentication failure rather than silently reporting a plain failure.
            RESPONSE_CODE_REDIRECT_REQUIRED => AttemptStatus::AuthenticationFailed,
            _ => AttemptStatus::AuthorizationFailed,
        }
    }

    /// Bank declines (`ResponseCode < 500`) carry bank-specific detail in
    /// `BankCode` / `BankDescription`; gateway rejections (`> 500`) do not.
    fn is_bank_decline(&self) -> bool {
        self.response_code()
            .parse::<i64>()
            .map(|code| code != 0 && code < 500)
            .unwrap_or(false)
    }

    /// `TransactionID` is `"0"` when no transaction was created (gateway rejection).
    fn connector_transaction_id(&self) -> Option<String> {
        self.transaction_id
            .as_ref()
            .filter(|id| id.as_str() != NO_TRANSACTION_ID)
            .cloned()
    }

    /// The MID and descriptor actually used, which vary on load-balanced master MIDs.
    fn connector_metadata(&self) -> Option<serde_json::Value> {
        if self.business_case.is_none() && self.descriptor.is_none() && self.bank.is_none() {
            return None;
        }
        Some(serde_json::json!({
            "business_case": self.business_case,
            "descriptor": self.descriptor,
            "bank": self.bank,
        }))
    }

    pub fn to_error_response(&self, http_code: u16) -> ErrorResponse {
        let message = self
            .response_description
            .clone()
            .unwrap_or_else(|| "Citigate transaction failed".to_string());
        let reason = self
            .bank_description
            .clone()
            .or_else(|| self.response_description.clone());
        let is_bank_decline = self.is_bank_decline();

        ErrorResponse {
            status_code: http_code,
            code: self
                .response_code
                .clone()
                .unwrap_or_else(|| "NO_RESPONSE_CODE".to_string()),
            message,
            reason,
            attempt_status: Some(FlowStatus::Payment(self.failure_status())),
            connector_transaction_id: self.connector_transaction_id(),
            network_decline_code: is_bank_decline.then(|| self.bank_code.clone()).flatten(),
            network_advice_code: None,
            network_error_message: is_bank_decline
                .then(|| self.bank_description.clone())
                .flatten(),
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<CitigatePaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CitigatePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        if !response.is_success() {
            return Ok(Self {
                response: Err(response.to_error_response(item.http_code)),
                resource_common_data: PaymentFlowData {
                    status: response.failure_status(),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let resource_id = match response.connector_transaction_id() {
            Some(transaction_id) => ResponseId::ConnectorTransactionId(transaction_id),
            None => ResponseId::NoResponseId,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.connector_metadata(),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.merchant_ref.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: response.attempt_status(),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
