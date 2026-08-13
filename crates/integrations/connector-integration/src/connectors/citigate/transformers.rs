//! Citigate transformers.
//!
//! Citigate exposes a single-endpoint, verb-in-body JSON API. Every operation is a
//! `POST` to `/orion/interface/json.ashx`; the operation is selected by the
//! `TransTypeID` body field. Authentication (`MerchantName` / `MerchantPassword`) is
//! also carried in the body, so there are no auth headers.
//!
//! Scope of this module: Card / Authorize (Purchase, `TransTypeID = 0`), one-time,
//! non-3DS **and** the 3DS user-redirect path, the Transaction Status Check
//! (`TransTypeID = 8`) used for both PSync and RSync, and the post-authorization
//! operations Capture (`3`), Void / Cancel (`4`) and Refund (`5`).

use std::collections::HashMap;

use common_enums::{AttemptStatus, AuthenticationType, CardNetwork, RefundStatus};
use common_utils::{pii::Email, types::StringMinorUnit, Method};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
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
/// `TransTypeID` for settling an open authorisation (UCS `Capture`).
const TRANS_TYPE_ID_CAPTURE: &str = "3";
/// `TransTypeID` for cancelling an open authorisation (UCS `Void`).
const TRANS_TYPE_ID_CANCEL: &str = "4";
/// `TransTypeID` for refunding a sale or a captured authorisation (UCS `Refund`).
const TRANS_TYPE_ID_REFUND: &str = "5";
/// `TransTypeID` for a transaction status check (UCS `PSync` **and** `RSync`).
const TRANS_TYPE_ID_STATUS_CHECK: &str = "8";

/// `ResponseCode` returned for an approved transaction.
const RESPONSE_CODE_APPROVED: &str = "0";
/// `ResponseCode` returned when the cardholder failed 3D authentication at the ACS.
const RESPONSE_CODE_3D_AUTH_FAILURE: &str = "103";
/// `ResponseCode` returned when a cardholder redirect (3DS) is required.
const RESPONSE_CODE_REDIRECT_REQUIRED: &str = "600";
/// Undocumented `ResponseCode` observed on Status Check responses; discriminated on
/// the response `TransTypeID`.
const RESPONSE_CODE_UNDOCUMENTED: &str = "999";

/// Response `TransTypeID` values (Transaction Key 4).
const RESP_TRANS_TYPE_SALE: &str = "1";
const RESP_TRANS_TYPE_AUTHORISE: &str = "2";
const RESP_TRANS_TYPE_CAPTURE: &str = "3";
const RESP_TRANS_TYPE_CANCEL: &str = "4";
const RESP_TRANS_TYPE_REFUND: &str = "5";
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

fn not_supported(detail: String) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: detail,
        connector: "citigate",
        context: IntegrationErrorContext::default(),
    })
}

/// `MerchantRef` is mandatory on every Citigate request and is the *only* key a
/// Transaction Status Check can be resolved by, so an empty reference is rejected
/// up front rather than sent and refused by the gateway.
fn required_merchant_ref(reference: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    if reference.is_empty() {
        return Err(error_stack::report!(
            IntegrationError::MissingRequiredField {
                field_name: "merchant_transaction_id",
                context: IntegrationErrorContext::default(),
            }
        ));
    }
    Ok(reference.to_string())
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
            other => Err(not_supported(format!("Card network {other:?}"))),
        };
    }

    match get_card_issuer(card.card_number.peek())? {
        CardIssuer::Visa => Ok(CitigateBrand::Visa),
        CardIssuer::Master => Ok(CitigateBrand::Mastercard),
        CardIssuer::AmericanExpress => Ok(CitigateBrand::Amex),
        CardIssuer::DinersClub => Ok(CitigateBrand::Diners),
        CardIssuer::Maestro => Ok(CitigateBrand::Maestro),
        other => Err(not_supported(format!("Card issuer {other:?}"))),
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
    /// `Y**` — mandatory on 3D-Secure-enabled MIDs, ignored elsewhere. Citigate
    /// POSTs the cardholder back here once the redirect transaction is approved.
    #[serde(rename = "SuccessURL", skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    /// `Y**` — the declined counterpart of `SuccessURL`. UCS has a single return
    /// URL and re-derives the outcome with PSync rather than from the landing URL,
    /// so both carry the same value.
    #[serde(rename = "FailURL", skip_serializing_if = "Option::is_none")]
    pub fail_url: Option<String>,
    /// `Y**` — receives the server-side POST that carries the final result.
    #[serde(rename = "CallbackURL", skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
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

        // The gateway predates 3DS2: there is no field anywhere in the interface to
        // carry an externally obtained CAVV / ECI / dsTransId, so merchant-provided
        // authentication data cannot be honoured.
        if router_data.request.authentication_data.is_some() {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "External/merchant-provided 3DS authentication data".to_string(),
                connector: "citigate",
                context: IntegrationErrorContext::default(),
            }));
        }

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

        // 3DS is an attribute of the MID, not of the transaction: there is no
        // `3ds`/`no_3ds` request flag anywhere in the interface, and the gateway may
        // ask for a redirect "irrespective of MID type". The three `Y**` URLs are
        // therefore forwarded whenever the caller supplied them — a non-3D MID
        // simply ignores them — but are required up front when the caller did ask
        // for 3DS, so a missing URL surfaces here instead of as ResponseCode
        // 584 / 585 / 586.
        let return_url = router_data
            .request
            .router_return_url
            .clone()
            .or_else(|| router_data.request.complete_authorize_url.clone())
            .or_else(|| common.return_url.clone());
        let callback_url = router_data.request.webhook_url.clone();

        if common.auth_type == AuthenticationType::ThreeDs {
            if return_url.is_none() {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "router_return_url",
                        context: IntegrationErrorContext::default(),
                    }
                ));
            }
            if callback_url.is_none() {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "webhook_url",
                        context: IntegrationErrorContext::default(),
                    }
                ));
            }
        }

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
            success_url: return_url.clone(),
            fail_url: return_url,
            callback_url,
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

    /// `true` when the gateway approved the transaction, asked for a cardholder
    /// redirect (`600`), or reported it as still pending (`999` + `TransTypeID 6`).
    fn is_success(&self) -> bool {
        match self.response_code() {
            RESPONSE_CODE_APPROVED => true,
            // `600` is the normal first answer on a 3D-enabled MID and must travel
            // through the success arm — but only if the gateway actually told us
            // where to send the cardholder.
            RESPONSE_CODE_REDIRECT_REQUIRED => self.redirect_form().is_some(),
            RESPONSE_CODE_UNDOCUMENTED => self.trans_type_id() == RESP_TRANS_TYPE_PENDING,
            _ => false,
        }
    }

    /// Status for an approved / redirecting / pending Authorize response.
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
            // Cardholder must be sent to the ACS page; the response `TransTypeID`
            // is not yet meaningful and only settles once the redirect completes.
            RESPONSE_CODE_REDIRECT_REQUIRED => AttemptStatus::AuthenticationPending,
            _ => AttemptStatus::Pending,
        }
    }

    /// Status attached to a failed Authorize response.
    fn failure_status(&self) -> AttemptStatus {
        match self.response_code() {
            // Cardholder failed / abandoned authentication at the ACS, or the
            // gateway asked for a redirect without telling us where to.
            RESPONSE_CODE_3D_AUTH_FAILURE | RESPONSE_CODE_REDIRECT_REQUIRED => {
                AttemptStatus::AuthenticationFailed
            }
            _ => AttemptStatus::AuthorizationFailed,
        }
    }

    /// Where to send the cardholder when the gateway demanded a redirect.
    ///
    /// `RedirectURL` is an absolute URL whose query string is an opaque token, so
    /// it is handed over verbatim as a `GET` with no form fields — there is no
    /// `PaReq`/`creq` to post. It is only populated on `ResponseCode 600`, and the
    /// PDF's own sample carries a stray leading space, hence the `trim`.
    fn redirect_form(&self) -> Option<RedirectForm> {
        if self.response_code() != RESPONSE_CODE_REDIRECT_REQUIRED {
            return None;
        }
        self.redirect_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(|url| RedirectForm::Form {
                endpoint: url.to_string(),
                method: Method::Get,
                form_fields: HashMap::new(),
            })
    }

    /// `true` when a Status Check answered with a usable state. `600` cannot be
    /// acted upon in a pull-based sync (there is no browser to redirect), so it is
    /// reported as "redirect not consumed yet", i.e. still pending.
    fn sync_is_success(&self) -> bool {
        match self.response_code() {
            RESPONSE_CODE_APPROVED | RESPONSE_CODE_REDIRECT_REQUIRED => true,
            RESPONSE_CODE_UNDOCUMENTED => self.trans_type_id() == RESP_TRANS_TYPE_PENDING,
            _ => false,
        }
    }

    /// Status Check status. The response `TransTypeID` describes the *original*
    /// transaction, so it is what decides the terminal state after a 3DS redirect.
    fn sync_attempt_status(&self) -> AttemptStatus {
        match self.response_code() {
            RESPONSE_CODE_APPROVED => match self.trans_type_id() {
                // A 3D MID runs a sale, so a completed 3DS payment is charged.
                RESP_TRANS_TYPE_SALE | RESP_TRANS_TYPE_CAPTURE | RESP_TRANS_TYPE_REFUND => {
                    AttemptStatus::Charged
                }
                RESP_TRANS_TYPE_AUTHORISE => AttemptStatus::Authorized,
                RESP_TRANS_TYPE_CANCEL => AttemptStatus::Voided,
                _ => AttemptStatus::Pending,
            },
            // `6` (still processing) and an unconsumed `600` both mean "keep polling".
            _ => AttemptStatus::Pending,
        }
    }

    /// Status attached to a failed Status Check. `TransTypeID 99` means the
    /// `MerchantName` + `MerchantPassword` + `MerchantRef` triple matched nothing.
    fn sync_failure_status(&self) -> AttemptStatus {
        match self.response_code() {
            RESPONSE_CODE_3D_AUTH_FAILURE => AttemptStatus::AuthenticationFailed,
            _ => AttemptStatus::Failure,
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

    /// Metadata for Capture and Void, whose response `TransactionID` identifies a
    /// **new** leg (auth `310` -> capture `312`) rather than the payment. UCS's
    /// `resource_id` must keep pointing at the original authorisation, so the leg id
    /// is retained here instead.
    fn leg_connector_metadata(&self) -> Option<serde_json::Value> {
        let leg_transaction_id = self.connector_transaction_id();
        if leg_transaction_id.is_none()
            && self.business_case.is_none()
            && self.descriptor.is_none()
            && self.bank.is_none()
        {
            return None;
        }
        Some(serde_json::json!({
            "business_case": self.business_case,
            "descriptor": self.descriptor,
            "bank": self.bank,
            "leg_transaction_id": leg_transaction_id,
        }))
    }

    /// `true` when the gateway approved a post-authorization operation
    /// (Capture / Void / Refund). These flows have neither a redirect (`600`) nor a
    /// pending state, so `ResponseCode` alone decides the outcome.
    fn is_approved(&self) -> bool {
        self.response_code() == RESPONSE_CODE_APPROVED
    }

    /// Capture status. On approval the response echoes `TransTypeID 3` and carries a
    /// **new** capture-leg `TransactionID`.
    fn capture_attempt_status(&self) -> AttemptStatus {
        if self.is_approved() {
            AttemptStatus::Charged
        } else {
            // Includes `561` — the auth was already captured, quite possibly by
            // Citigate's own 48-96h auto-capture service, which cannot be disabled.
            AttemptStatus::CaptureFailed
        }
    }

    /// Void / Cancel status. On approval the response echoes `TransTypeID 4`.
    fn void_attempt_status(&self) -> AttemptStatus {
        if self.is_approved() {
            AttemptStatus::Voided
        } else {
            // `561` here means the auth is no longer open and a Refund is the
            // correct operation instead.
            AttemptStatus::VoidFailed
        }
    }

    /// Refund status. `605` ("Bank does not support API refunds") is a failure of
    /// the API call even though Citigate logs a manual refund out of band; nothing
    /// in this flow can resolve that, so it is reported as a failure verbatim.
    fn refund_status(&self) -> RefundStatus {
        if self.is_approved() {
            RefundStatus::Success
        } else {
            RefundStatus::Failure
        }
    }

    /// RSync status. The Status Check reports the `TransTypeID` of the transaction
    /// the `MerchantRef` resolved to, so only a refund leg (`5`) may be reported as
    /// a settled refund.
    fn refund_sync_status(&self) -> RefundStatus {
        match (self.response_code(), self.trans_type_id()) {
            (RESPONSE_CODE_APPROVED, RESP_TRANS_TYPE_REFUND) => RefundStatus::Success,
            // The refund leg exists but has not reached a terminal state yet.
            (RESPONSE_CODE_APPROVED, _) | (RESPONSE_CODE_UNDOCUMENTED, RESP_TRANS_TYPE_PENDING) => {
                RefundStatus::Pending
            }
            // `999` + `TransTypeID 99` is "MerchantRef not found".
            _ => RefundStatus::Failure,
        }
    }

    pub fn to_error_response(&self, http_code: u16) -> ErrorResponse {
        self.to_error_response_with_status(http_code, self.failure_status())
    }

    fn to_error_response_with_status(
        &self,
        http_code: u16,
        attempt_status: AttemptStatus,
    ) -> ErrorResponse {
        self.to_flow_error_response(http_code, FlowStatus::Payment(attempt_status))
    }

    fn to_refund_error_response(
        &self,
        http_code: u16,
        refund_status: RefundStatus,
    ) -> ErrorResponse {
        self.to_flow_error_response(http_code, FlowStatus::Refund(refund_status))
    }

    fn to_flow_error_response(&self, http_code: u16, flow_status: FlowStatus) -> ErrorResponse {
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
            attempt_status: Some(flow_status),
            connector_transaction_id: self.connector_transaction_id(),
            network_decline_code: is_bank_decline.then(|| self.bank_code.clone()).flatten(),
            network_advice_code: None,
            network_error_message: is_bank_decline
                .then(|| self.bank_description.clone())
                .flatten(),
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
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
                redirection_data: response.redirect_form().map(Box::new),
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

// =============================================================================
// PSYNC — TRANSACTION STATUS CHECK (`TransTypeID = 8`)
// =============================================================================

/// Transaction Status Check request.
///
/// The lookup key is the `MerchantName` + `MerchantPassword` + `MerchantRef`
/// triple — **not** the `TransactionID`. Querying with a different MID than the
/// one that created the payment therefore reports a perfectly good transaction as
/// `TransTypeID 99` / "MerchantRef not found".
#[derive(Debug, Serialize)]
pub struct CitigateSyncRequest {
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
}

type SyncRouterData = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<CitigateRouterData<SyncRouterData, T>> for CitigateSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: CitigateRouterData<SyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = CitigateAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            payment_type_id: PAYMENT_TYPE_ID_CARD.to_string(),
            trans_type_id: TRANS_TYPE_ID_STATUS_CHECK.to_string(),
            merchant_name: auth.merchant_name,
            merchant_password: auth.merchant_password,
            merchant_ref: required_merchant_ref(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )?,
        })
    }
}

/// Status Check answers with the very same envelope as a purchase (minus
/// `RedirectURL`); the newtype exists only to give the macro framework a distinct
/// response type per flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CitigateSyncResponse(pub CitigatePaymentsResponse);

impl TryFrom<ResponseRouterData<CitigateSyncResponse, Self>> for SyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<CitigateSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response.0;

        if !response.sync_is_success() {
            return Ok(Self {
                response: Err(response
                    .to_error_response_with_status(item.http_code, response.sync_failure_status())),
                resource_common_data: PaymentFlowData {
                    status: response.sync_failure_status(),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let resource_id = match response.connector_transaction_id() {
            Some(transaction_id) => ResponseId::ConnectorTransactionId(transaction_id),
            None => item.router_data.request.connector_transaction_id.clone(),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                // A sync has no browser to redirect: the redirect instruction, if
                // any, was already handed out by Authorize.
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
                status: response.sync_attempt_status(),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// CAPTURE (`TransTypeID = 3`)
// =============================================================================

/// Capture request — settles an open authorisation.
///
/// The documented field list has exactly six entries and **no `Amount`**: Citigate
/// cannot perform a partial, multiple or incremental capture.
#[derive(Debug, Serialize)]
pub struct CitigateCaptureRequest {
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
    /// The transaction reference from the original authorisation.
    #[serde(rename = "TransactionID")]
    pub transaction_id: String,
}

type CaptureRouterData =
    RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<CitigateRouterData<CaptureRouterData, T>> for CitigateCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: CitigateRouterData<CaptureRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // Fail fast rather than silently capturing an amount the caller did not ask
        // for: the wire format simply has nowhere to put one.
        if request.is_multiple_capture() {
            return Err(not_supported("Multiple partial captures".to_string()));
        }
        if let Some(authorized) = router_data.resource_common_data.minor_amount_authorized {
            if authorized != request.minor_amount_to_capture {
                return Err(not_supported("Partial capture".to_string()));
            }
        }

        let auth = CitigateAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            payment_type_id: PAYMENT_TYPE_ID_CARD.to_string(),
            trans_type_id: TRANS_TYPE_ID_CAPTURE.to_string(),
            merchant_name: auth.merchant_name,
            merchant_password: auth.merchant_password,
            merchant_ref: required_merchant_ref(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )?,
            transaction_id: request.get_connector_transaction_id()?,
        })
    }
}

/// Capture answers with the shared response envelope; the newtype exists only to
/// give the macro framework a distinct response type per flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CitigateCaptureResponse(pub CitigatePaymentsResponse);

impl TryFrom<ResponseRouterData<CitigateCaptureResponse, Self>> for CaptureRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CitigateCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;
        let status = response.capture_attempt_status();

        if !response.is_approved() {
            return Ok(Self {
                response: Err(response.to_error_response_with_status(item.http_code, status)),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // The response `TransactionID` is the new capture leg, not the
                // payment — keep the id every later operation and PSync key off.
                resource_id: item.router_data.request.connector_transaction_id.clone(),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.leg_connector_metadata(),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.merchant_ref.clone(),
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

// =============================================================================
// VOID / CANCEL (`TransTypeID = 4`)
// =============================================================================

/// Cancel request — voids an open authorisation.
///
/// Same six fields as Capture. There is no `Amount` (a void always cancels the
/// whole authorisation) and no field able to carry a cancellation reason.
#[derive(Debug, Serialize)]
pub struct CitigateVoidRequest {
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
    /// The transaction reference from the original authorisation.
    #[serde(rename = "TransactionID")]
    pub transaction_id: String,
}

type VoidRouterData = RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<CitigateRouterData<VoidRouterData, T>> for CitigateVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: CitigateRouterData<VoidRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        if let (Some(requested), Some(authorized)) = (
            request.amount,
            router_data.resource_common_data.minor_amount_authorized,
        ) {
            if requested != authorized {
                return Err(not_supported("Partial void".to_string()));
            }
        }

        let auth = CitigateAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            payment_type_id: PAYMENT_TYPE_ID_CARD.to_string(),
            trans_type_id: TRANS_TYPE_ID_CANCEL.to_string(),
            merchant_name: auth.merchant_name,
            merchant_password: auth.merchant_password,
            merchant_ref: required_merchant_ref(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )?,
            transaction_id: request.connector_transaction_id.clone(),
        })
    }
}

/// Cancel answers with the shared response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CitigateVoidResponse(pub CitigatePaymentsResponse);

impl TryFrom<ResponseRouterData<CitigateVoidResponse, Self>> for VoidRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<CitigateVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response.0;
        let status = response.void_attempt_status();

        if !response.is_approved() {
            return Ok(Self {
                response: Err(response.to_error_response_with_status(item.http_code, status)),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // As with Capture, the response `TransactionID` is the new cancel
                // leg and must not replace the payment's id.
                resource_id: ResponseId::ConnectorTransactionId(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.leg_connector_metadata(),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.merchant_ref.clone(),
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

// =============================================================================
// REFUND (`TransTypeID = 5`)
// =============================================================================

/// Refund request — refunds a sale, or an authorisation that has been captured
/// (including one auto-captured by Citigate, in which case the *authorisation's*
/// `TransactionID` is still the correct value to send).
#[derive(Debug, Serialize)]
pub struct CitigateRefundRequest {
    #[serde(rename = "PaymentTypeID")]
    pub payment_type_id: String,
    #[serde(rename = "TransTypeID")]
    pub trans_type_id: String,
    #[serde(rename = "MerchantName")]
    pub merchant_name: Secret<String>,
    #[serde(rename = "MerchantPassword")]
    pub merchant_password: Secret<String>,
    /// Also the RSync lookup key — the Status Check cannot be keyed on anything else.
    #[serde(rename = "MerchantRef")]
    pub merchant_ref: String,
    #[serde(rename = "TransactionID")]
    pub transaction_id: String,
    /// Omitted for a full refund, which is both the documented default and the only
    /// form that works on an account where partial refunds have not been enabled
    /// (the default; sending an `Amount` there is rejected with `629`).
    #[serde(rename = "Amount", skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMinorUnit>,
}

type RefundRouterData = RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<CitigateRouterData<RefundRouterData, T>> for CitigateRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: CitigateRouterData<RefundRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let auth = CitigateAuthType::try_from(&router_data.connector_config)?;

        let amount = if request.minor_refund_amount == request.minor_payment_amount {
            None
        } else {
            Some(CitigateAmountConvertor::convert(
                request.minor_refund_amount,
                request.currency,
            )?)
        };

        Ok(Self {
            payment_type_id: PAYMENT_TYPE_ID_CARD.to_string(),
            trans_type_id: TRANS_TYPE_ID_REFUND.to_string(),
            merchant_name: auth.merchant_name,
            merchant_password: auth.merchant_password,
            merchant_ref: required_merchant_ref(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )?,
            transaction_id: request.connector_transaction_id.clone(),
            amount,
        })
    }
}

/// Refund answers with the shared response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CitigateRefundResponse(pub CitigatePaymentsResponse);

impl TryFrom<ResponseRouterData<CitigateRefundResponse, Self>> for RefundRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CitigateRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;
        let refund_status = response.refund_status();

        if !response.is_approved() {
            return Ok(Self {
                response: Err(response.to_refund_error_response(item.http_code, refund_status)),
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Here the new leg id *is* the refund, so it becomes `connector_refund_id`.
        let connector_refund_id = response
            .connector_transaction_id()
            .unwrap_or_else(|| item.router_data.request.refund_id.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// RSYNC — TRANSACTION STATUS CHECK ON THE REFUND'S `MerchantRef`
// =============================================================================

/// Refund status check. Byte-identical on the wire to [`CitigateSyncRequest`] — the
/// only thing that makes a `TransTypeID = 8` call an RSync rather than a PSync is
/// that the `MerchantRef` is the **refund's** reference. Keying it on
/// `connector_refund_id` (a Citigate `TransactionID`, which the status check has no
/// field for) or on the Authorize `MerchantRef` would silently resolve the payment
/// leg instead.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct CitigateRefundSyncRequest(pub CitigateSyncRequest);

type RefundSyncRouterData =
    RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<CitigateRouterData<RefundSyncRouterData, T>> for CitigateRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: CitigateRouterData<RefundSyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = CitigateAuthType::try_from(&router_data.connector_config)?;

        Ok(Self(CitigateSyncRequest {
            payment_type_id: PAYMENT_TYPE_ID_CARD.to_string(),
            trans_type_id: TRANS_TYPE_ID_STATUS_CHECK.to_string(),
            merchant_name: auth.merchant_name,
            merchant_password: auth.merchant_password,
            // The RefundSync RouterData carries the same
            // `connector_request_reference_id` the Refund request used.
            merchant_ref: required_merchant_ref(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )?,
        }))
    }
}

/// The refund status check answers with the shared response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CitigateRefundSyncResponse(pub CitigatePaymentsResponse);

impl TryFrom<ResponseRouterData<CitigateRefundSyncResponse, Self>> for RefundSyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CitigateRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;
        let refund_status = response.refund_sync_status();

        if refund_status == RefundStatus::Failure {
            return Ok(Self {
                response: Err(response.to_refund_error_response(item.http_code, refund_status)),
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let connector_refund_id = response
            .connector_transaction_id()
            .unwrap_or_else(|| item.router_data.request.connector_refund_id.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
