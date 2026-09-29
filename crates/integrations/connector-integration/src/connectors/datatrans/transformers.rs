use std::collections::HashMap;

use crate::types::ResponseRouterData;
use base64::{engine::general_purpose::STANDARD, Engine};
use common_enums::{AttemptStatus, CountryAlpha2, Currency, PostCaptureVoidStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    pii::Email,
    request::Method,
    MinorUnit,
};
use domain_types::errors::{
    ConnectorError, IntegrationError, IntegrationErrorContext, ResponseTransformationErrorContext,
    WebhookError,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PaymentMethodToken, RSync, Refund,
        RepeatPayment, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        DatatransClientAuthenticationResponse as DatatransClientAuthenticationResponseDomain,
        EventType, MandateReference, MandateReferenceId, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, BrowserInformation},
    router_response_types::RedirectForm,
    types::{AdditionalCardInfo, AdditionalPaymentData, ConnectorParams},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

const UNSUPPORTED_PAYMENT_METHOD_ERROR: &str =
    "Only card, Google Pay and Apple Pay payments are supported for Datatrans";
/// NotSupported message for network-transaction-id / network-token payment method data:
/// the Datatrans public API carries no NTI or network-token field (spec: NTID NotSupported).
const UNSUPPORTED_NETWORK_TRANSACTION_ID_ERROR: &str =
    "network transaction id / network token payments";

/// Builds the browser redirect to the Datatrans hosted payment page
/// (`GET {pay-host}/v1/start/{transactionId}`) for a transaction created on the
/// redirect-capable init endpoint (`POST /v1/transactions`).
///
/// The payment-page host is environment-specific and differs from the API host, so it
/// is read from `connectors.datatrans.secondary_base_url` (sandbox
/// `https://pay.sandbox.datatrans.com`, production `https://pay.datatrans.com`).
/// A missing value fails closed rather than guessing a host.
/// spec: "Redirect to Payment Page (browser)" — `GET /v1/start/{transactionId}` on the pay host.
fn datatrans_start_redirect(
    params: &ConnectorParams,
    transaction_id: &str,
) -> Result<RedirectForm, error_stack::Report<ConnectorError>> {
    let host = params
        .secondary_base_url
        .as_deref()
        .filter(|host| !host.trim().is_empty())
        .ok_or_else(|| {
            error_stack::report!(ConnectorError::ResponseHandlingFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "Datatrans redirect requires the payment-page host in config key datatrans.secondary_base_url"
                            .to_string(),
                    ),
                },
            })
        })?;
    Ok(RedirectForm::Form {
        endpoint: format!("{}/v1/start/{}", host.trim_end_matches('/'), transaction_id),
        method: Method::Get,
        form_fields: HashMap::new(),
    })
}
/// Card `type` discriminator sent to Datatrans for raw PAN card data.
const CARD_TYPE_PLAIN: &str = "PLAIN";
/// Card `type` discriminator sent to Datatrans for a stored-alias charge
/// (MIT / RepeatPayment): the alias created by SetupMandate is reused in place of a PAN.
const CARD_TYPE_ALIAS: &str = "ALIAS";
/// Error surfaced when a Datatrans MIT/RepeatPayment carries a mandate reference type
/// this connector cannot charge via an alias (only connector-stored alias mandates work).
const UNSUPPORTED_MANDATE_REFERENCE_ERROR: &str =
    "Only connector-stored alias mandates are supported for Datatrans repeat payments";
/// Datatrans `authenticationResponse` value flagging a completed external 3DS
/// authentication (`Y` = authenticated) when forwarding passthrough cavv/eci/xid.
const THREE_DS_AUTHENTICATION_RESPONSE_Y: &str = "Y";

/// Builds an `IntegrationErrorContext` carrying Datatrans-specific remediation detail,
/// so error sites never fall back to a context-free `Default::default()`.
fn datatrans_context(additional_context: &str) -> IntegrationErrorContext {
    IntegrationErrorContext {
        additional_context: Some(additional_context.to_string()),
        ..Default::default()
    }
}

/// Maximum length of the Datatrans merchant reference (`refno`), per the API reference.
const DATATRANS_REFNO_MAX_LEN: usize = 40;

/// Validates a Datatrans merchant reference (`refno`): the API accepts a string of
/// 1..=40 characters. A value outside that range is refused before the call with a named
/// error instead of being truncated or substituted.
fn datatrans_refno(value: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    let length = value.chars().count();
    if (1..=DATATRANS_REFNO_MAX_LEN).contains(&length) {
        Ok(value.to_string())
    } else {
        Err(error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "refno",
            context: datatrans_context("Datatrans refno must be 1-40 characters"),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct DatatransAuthType {
    pub merchant_id: Secret<String>,
    pub password: Secret<String>,
}

impl DatatransAuthType {
    pub fn generate_basic_auth(&self) -> String {
        let credentials = format!("{}:{}", self.merchant_id.peek(), self.password.peek());
        let encoded = STANDARD.encode(credentials);
        format!("Basic {encoded}")
    }
}

impl TryFrom<&ConnectorSpecificConfig> for DatatransAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Datatrans {
                merchant_id,
                password,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                password: password.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// Error response structure - Datatrans API uses nested format
// Format: {"error": {"code": "...", "message": "..."}}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatatransErrorResponse {
    pub error: DatatransErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatatransErrorDetail {
    pub code: String,
    pub message: String,
}

impl DatatransErrorResponse {
    pub fn code(&self) -> String {
        self.error.code.clone()
    }

    pub fn message(&self) -> String {
        self.error.message.clone()
    }

    /// Builds an error from a non-JSON body (e.g. an HTML gateway error page) so the raw page
    /// text is surfaced instead of a deserialization failure. Mirrors HS Direct's HTML fallback
    /// (decoded lossily as UTF-8 rather than pulling in an ISO-8859-10 codec).
    pub fn from_non_json_body(body: &[u8]) -> Self {
        Self {
            error: DatatransErrorDetail {
                code: NO_ERROR_CODE.to_string(),
                message: String::from_utf8_lossy(body).trim().to_string(),
            },
        }
    }
}

impl Default for DatatransErrorResponse {
    fn default() -> Self {
        Self {
            error: DatatransErrorDetail {
                code: NO_ERROR_CODE.to_string(),
                message: NO_ERROR_MESSAGE.to_string(),
            },
        }
    }
}

/// Which flow an HTTP error response belongs to, so the error path can report a
/// flow-aware `attempt_status` (see [`datatrans_error_attempt_status`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatatransErrorFlow {
    /// Charging calls: Authorize, SetupMandate, RepeatPayment.
    Payment,
    Capture,
    Void,
    Refund,
    /// Status reads (PSync / RSync): a request-level error says nothing about the transaction.
    Sync,
    /// Token / session flows that own no payment attempt.
    Other,
}

/// Flow-aware `attempt_status` for a Datatrans HTTP error response.
///
/// Datatrans reports declines and transaction failures as 4xx ("4xx: Client error or
/// transaction failure"), so a 4xx on a flow that owns a terminal failure maps to that
/// failure. 409 means an idempotent retry of a request that is still being processed, and
/// 5xx is transient: neither says the transaction failed, so both stay `None`.
pub(crate) fn datatrans_error_attempt_status(
    flow: DatatransErrorFlow,
    status_code: u16,
) -> Option<FlowStatus> {
    if status_code == 409 || status_code >= 500 {
        return None;
    }
    match flow {
        DatatransErrorFlow::Payment => Some(FlowStatus::Payment(AttemptStatus::Failure)),
        DatatransErrorFlow::Capture => Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
        DatatransErrorFlow::Void => Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
        DatatransErrorFlow::Refund => Some(FlowStatus::Refund(RefundStatus::Failure)),
        DatatransErrorFlow::Sync | DatatransErrorFlow::Other => None,
    }
}

// Card details for Datatrans API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_month: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_year: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<RawCardNumber<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub card_type: Option<String>,
    /// The `3D` object driving card 3DS: either merchant-supplied external
    /// authentication artifacts (`Authentication`) or cardholder details that ask
    /// Datatrans to run native 3DS (`Cardholder`). Omitted for non-3DS card payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "3D")]
    pub three_ds: Option<ThreeDSecureData>,
}

/// The `3D` object attached to a Datatrans card in an Authorize request.
#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
pub enum ThreeDSecureData {
    /// Native 3DS: Datatrans drives the ACS challenge using these cardholder details.
    Cardholder(ThreedsInfo),
    /// Passthrough external 3DS: the merchant already authenticated and forwards
    /// the resulting cavv/eci/xid to Datatrans.
    Authentication(ThreeDSData),
}

#[derive(Debug, Serialize, Clone)]
pub struct ThreedsInfo {
    pub cardholder: CardHolder,
    /// EMVCo 3DS requestor preferences; sent on the Authorize native-3DS path only.
    #[serde(rename = "threeDSRequestor", skip_serializing_if = "Option::is_none")]
    pub three_ds_requestor: Option<ThreeDSRequestor>,
    /// Browser data for the 3DS risk assessment (one of browserIP/deviceID is mandatory
    /// for Datatrans 3DS); sent on the Authorize native-3DS path when `browser_info` is given.
    #[serde(rename = "browserInformation", skip_serializing_if = "Option::is_none")]
    pub browser_information: Option<DatatransBrowserInformation>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThreeDSRequestor {
    #[serde(rename = "threeDSRequestorChallengeInd")]
    pub three_ds_requestor_challenge_ind: ThreeDSRequestorChallengeIndicator,
}

/// EMVCo `threeDSRequestorChallengeInd`. Only "01" (no preference) is reachable: the UCS
/// Authorize request carries no force-challenge flag.
#[derive(Debug, Serialize, Clone)]
pub enum ThreeDSRequestorChallengeIndicator {
    #[serde(rename = "01")]
    NoPreference,
}

#[derive(Debug, Serialize, Clone)]
pub struct DatatransBrowserInformation {
    #[serde(rename = "browserIP", skip_serializing_if = "Option::is_none")]
    pub browser_ip: Option<Secret<String>>,
    #[serde(
        rename = "browserScreenHeight",
        skip_serializing_if = "Option::is_none"
    )]
    pub browser_screen_height: Option<u32>,
    #[serde(rename = "browserScreenWidth", skip_serializing_if = "Option::is_none")]
    pub browser_screen_width: Option<u32>,
    #[serde(rename = "browserLanguage", skip_serializing_if = "Option::is_none")]
    pub browser_language: Option<String>,
    #[serde(rename = "browserTZ", skip_serializing_if = "Option::is_none")]
    pub browser_tz: Option<i32>,
}

impl DatatransBrowserInformation {
    /// `None` when the request carries no browser data at all, so an empty
    /// `browserInformation` object is never sent.
    fn from_browser_info(browser_info: &BrowserInformation) -> Option<Self> {
        let info = Self {
            browser_ip: browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string())),
            browser_screen_height: browser_info.screen_height,
            browser_screen_width: browser_info.screen_width,
            browser_language: browser_info.language.clone(),
            browser_tz: browser_info.time_zone,
        };
        let has_any = info.browser_ip.is_some()
            || info.browser_screen_height.is_some()
            || info.browser_screen_width.is_some()
            || info.browser_language.is_some()
            || info.browser_tz.is_some();
        has_any.then_some(info)
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CardHolder {
    pub cardholder_name: Secret<String>,
    pub email: Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_post_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_state: Option<Secret<String>>,
    /// ISO 3166-1 numeric country code, 3 digits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_line1: Option<Secret<String>>,
}

/// External (passthrough) 3DS authentication data forwarded to Datatrans.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSData {
    #[serde(rename = "threeDSTransactionId")]
    pub three_ds_transaction_id: Option<Secret<String>>,
    pub cavv: Secret<String>,
    pub eci: Option<String>,
    pub xid: Option<Secret<String>>,
    #[serde(rename = "threeDSVersion")]
    pub three_ds_version: Option<String>,
    #[serde(rename = "authenticationResponse")]
    pub authentication_response: String,
}

/// Redirect return URLs supplied to Datatrans on the redirect-capable init endpoint
/// (`POST /v1/transactions`). Every init transaction redirects the customer to the hosted
/// page, so all three URLs are always present.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RedirectUrls {
    pub success_url: String,
    pub cancel_url: String,
    pub error_url: String,
}

/// Builds the `redirect` block for an init (`POST /v1/transactions`) request. The init
/// transaction always redirects the customer to the hosted payment page, so a missing
/// `router_return_url` is refused before any HTTP call instead of sending null URLs.
fn init_redirect_urls(
    router_return_url: Option<&String>,
    flow: &str,
) -> Result<RedirectUrls, error_stack::Report<IntegrationError>> {
    let url = router_return_url.cloned().ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "router_return_url",
            context: datatrans_context(&format!(
                "Datatrans {flow} on the redirect-capable /v1/transactions endpoint requires router_return_url for successUrl/cancelUrl/errorUrl"
            )),
        })
    })?;
    Ok(RedirectUrls {
        success_url: url.clone(),
        cancel_url: url.clone(),
        error_url: url,
    })
}

/// Whether an Authorize goes to the redirect-capable init endpoint (`POST /v1/transactions`)
/// instead of the direct server-to-server `POST /v1/transactions/authorize`.
///
/// Mirrors HS Direct: native 3DS on a raw card (Datatrans drives the challenge, no external
/// authentication data), a CIT alias registration on a raw card (`is_mandate_payment`), or
/// native 3DS on a Google Pay alias (`PaymentMethodToken`). Shared by the request builder,
/// `get_url` and the response mapper so all three agree on which endpoint answered.
pub(crate) fn authorize_uses_init_endpoint<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> bool {
    let native_three_ds = router_data.resource_common_data.is_three_ds()
        && router_data.request.authentication_data.is_none();
    match router_data.request.payment_method_data {
        PaymentMethodData::Card(_) => native_three_ds || router_data.request.is_mandate_payment(),
        PaymentMethodData::PaymentMethodToken(_) => native_three_ds,
        _ => false,
    }
}

// Authorize request structure based on tech spec
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub currency: Currency,
    pub refno: String,
    /// Charge amount in minor units. Omitted (`None`) for zero-auth SetupMandate/CIT
    /// alias creation, where no amount is captured; always present for Authorize.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<DatatransCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_settle: Option<bool>,
    /// Present only for native 3DS: the cardholder is redirected here after the
    /// ACS challenge. Omitted for passthrough external 3DS and no-3DS payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect: Option<RedirectUrls>,
    // Don't skip serializing - we want "option": null to appear in JSON
    pub option: Option<DatatransPaymentOptions>,
    #[serde(rename = "PAY", skip_serializing_if = "Option::is_none")]
    pub pay: Option<DatatransGooglePayRequest>,
    #[serde(rename = "APL", skip_serializing_if = "Option::is_none")]
    pub apl: Option<DatatransApplePayRequest>,
    /// Billing address (AVS input). Omitted when no billing field is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<DatatransAddress>,
    /// Billing contact. Omitted when no customer field is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<DatatransCustomer>,
    /// Shipping address. Omitted when no shipping field is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<DatatransAddress>,
}

/// `billing` and `shipping` objects on `/v1/transactions/authorize` and
/// `/v1/transactions` (init). spec: "### Billing" / "### Shipping" — every field is
/// optional on Datatrans.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip_code: Option<Secret<String>>,
    /// ISO 3166-1 alpha-2 (plan UD-11).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<CountryAlpha2>,
}

impl DatatransAddress {
    /// Builds the billing object from the payment's billing address; `None` when every
    /// field is absent so an empty `billing: {}` is never sent.
    fn from_billing(resource_common_data: &PaymentFlowData) -> Option<Self> {
        Self {
            street: resource_common_data.get_optional_billing_line1(),
            street2: resource_common_data.get_optional_billing_line2(),
            city: resource_common_data.get_optional_billing_city(),
            zip_code: resource_common_data.get_optional_billing_zip(),
            country: resource_common_data.get_optional_billing_country(),
        }
        .into_non_empty()
    }

    /// Builds the shipping object from the payment's shipping address; `None` when every
    /// field is absent so an empty `shipping: {}` is never sent.
    fn from_shipping(resource_common_data: &PaymentFlowData) -> Option<Self> {
        Self {
            street: resource_common_data.get_optional_shipping_line1(),
            street2: resource_common_data.get_optional_shipping_line2(),
            city: resource_common_data.get_optional_shipping_city(),
            zip_code: resource_common_data.get_optional_shipping_zip(),
            country: resource_common_data.get_optional_shipping_country(),
        }
        .into_non_empty()
    }

    fn into_non_empty(self) -> Option<Self> {
        let has_any_field = self.street.is_some()
            || self.street2.is_some()
            || self.city.is_some()
            || self.zip_code.is_some()
            || self.country.is_some();
        has_any_field.then_some(self)
    }
}

/// `customer` object on `/v1/transactions/authorize` and `/v1/transactions` (init).
/// `phone` is deliberately not sent (plan UD-12: format undocumented, optional).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCustomer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
}

impl DatatransCustomer {
    /// Builds the customer object from the billing contact, falling back to the
    /// request-level email; `None` when every field is absent.
    fn from_billing(
        resource_common_data: &PaymentFlowData,
        request_email: Option<&Email>,
    ) -> Option<Self> {
        let customer = Self {
            first_name: resource_common_data.get_optional_billing_first_name(),
            last_name: resource_common_data.get_optional_billing_last_name(),
            email: resource_common_data
                .get_optional_billing_email()
                .or_else(|| request_email.cloned()),
        };
        let has_any_field = customer.first_name.is_some()
            || customer.last_name.is_some()
            || customer.email.is_some();
        has_any_field.then_some(customer)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransGooglePayRequest {
    signature: Secret<String>,
    protocol_version: Secret<String>,
    signed_message: Secret<String>,
    intermediate_signing_key: DatatransGooglePayIntermediateSigningKey,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransGooglePayIntermediateSigningKey {
    signed_key: Secret<String>,
    signatures: Vec<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransApplePayRequest {
    data: Secret<String>,
    header: DatatransApplePayHeader,
    signature: Secret<String>,
    version: Secret<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransApplePayHeader {
    public_key_hash: Secret<String>,
    ephemeral_public_key: Secret<String>,
    transaction_id: Secret<String>,
}

/// SetupMandate (zero-auth mandate registration) request. The body depends on the endpoint
/// chosen by [`setup_mandate_uses_validate`]:
/// - `Validate` — `POST /v1/transactions/validate`, the zero-amount card check that returns
///   `card.alias` synchronously for a raw card without 3DS.
/// - `Init` — `POST /v1/transactions`, the redirect-capable init (native 3DS on a raw card,
///   or a Google Pay alias registration) completed by the customer on the hosted page.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum DatatransSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Init(Box<DatatransPaymentsRequest<T>>),
    Validate(DatatransValidateRequest<T>),
}

/// Zero-amount card check (`POST /v1/transactions/validate`).
/// spec: "Validate an Alias" — https://docs.datatrans.ch/docs/payment-process-validate-an-alias
/// No `amount`, `autoSettle`, `option` or `redirect`: the check charges nothing and
/// completes server-to-server.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransValidateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub currency: Currency,
    pub refno: String,
    pub card: DatatransValidateCard<T>,
}

/// Raw card on the validate request. Unlike [`DatatransCard`] it carries no `type`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransValidateCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub number: RawCardNumber<T>,
    pub expiry_month: Secret<String>,
    /// 2-digit expiry year.
    pub expiry_year: Secret<String>,
    pub cvv: Secret<String>,
}

/// Whether a SetupMandate goes to the zero-amount card check (`POST /v1/transactions/validate`)
/// instead of the redirect-capable init (`POST /v1/transactions`): a raw card without 3DS.
/// Shared by the request builder, `get_url` and the response mapper so all three agree on
/// which endpoint answered.
pub(crate) fn setup_mandate_uses_validate<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> bool {
    matches!(
        router_data.request.payment_method_data,
        PaymentMethodData::Card(_)
    ) && !router_data.resource_common_data.is_three_ds()
}

/// SetupMandate response: the validate body (`{transactionId, acquirerAuthorizationCode,
/// card.alias}`) and the init body (`{transactionId[, 3D]}`) share the Authorize response
/// shape; the mapper keys on the endpoint, not on the body.
pub type DatatransSetupMandateResponse = DatatransPaymentsResponse;

/// MIT / RepeatPayment reuses the Authorize request and response shapes: the stored alias
/// is charged via the same `/v1/transactions/authorize` endpoint. These aliases give the
/// Bridge macro distinct per-flow templating type names without duplicating the structs.
pub type DatatransRepeatPaymentRequest<T> = DatatransPaymentsRequest<T>;
pub type DatatransRepeatPaymentResponse = DatatransPaymentsResponse;

// Payment options for Datatrans API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransPaymentOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_alias: Option<bool>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DatatransPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        // The init endpoint (native 3DS, raw-card CIT, or native 3DS on an alias) always
        // redirects to the hosted page and needs the return URLs; passthrough external 3DS
        // and no-3DS payments use the direct authorize endpoint and do not redirect.
        let uses_init_endpoint = authorize_uses_init_endpoint(router_data);
        // CIT ("purchase + save card"): a customer-initiated Authorize that also registers a
        // reusable Datatrans alias for later MIT/RepeatPayment. Mirrors the HS Direct Authorize
        // path, which sets `createAlias` and redirect URLs for `is_mandate_payment()`.
        let is_mandate_payment = router_data.request.is_mandate_payment();

        // Extract card data or token
        let (card, redirect, pay, apl) = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Direct card flow - use raw card details
                let card = DatatransCard {
                    alias: None,
                    number: Some(card_data.card_number.clone()),
                    expiry_month: Some(card_data.card_exp_month.clone()),
                    expiry_year: Some(card_data.get_card_expiry_year_2_digit()?),
                    cvv: Some(card_data.card_cvc.clone()),
                    card_type: Some(CARD_TYPE_PLAIN.to_string()),
                    three_ds: build_three_ds_data(
                        router_data.request.authentication_data.as_ref(),
                        &router_data.resource_common_data,
                        false,
                        Some(ThreeDSRequestorChallengeIndicator::NoPreference),
                        router_data.request.browser_info.as_ref(),
                    )?,
                };
                // Return URLs are required for a native-3DS challenge OR a CIT alias
                // registration (which Datatrans runs through the redirect-capable endpoint).
                let redirect = uses_init_endpoint
                    .then(|| {
                        init_redirect_urls(router_data.request.router_return_url.as_ref(), "Authorize")
                    })
                    .transpose()?;
                (Some(card), redirect, None, None)
            }
            // Google Pay alias charge: the PaymentMethodToken flow tokenized the Google
            // Pay payload via POST /v1/aliases/tokenize, and this token carries the
            // resulting alias. The alias is charged as an `ALIAS` card — the Datatrans
            // path that supports a native 3DS challenge for Google Pay.
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let token = token_data.token.clone();

                let card = DatatransCard {
                    alias: Some(token),
                    number: None,
                    expiry_month: None,
                    expiry_year: None,
                    cvv: None,
                    card_type: Some(CARD_TYPE_ALIAS.to_string()),
                    three_ds: build_three_ds_data(
                        router_data.request.authentication_data.as_ref(),
                        &router_data.resource_common_data,
                        false,
                        Some(ThreeDSRequestorChallengeIndicator::NoPreference),
                        router_data.request.browser_info.as_ref(),
                    )?,
                };
                // Native 3DS on the alias follows the same redirect contract as a
                // raw-card 3DS charge (the alias itself carries no expiry/CVV, so the
                // CIT-mandate `createAlias` branch is not applicable here).
                let redirect = uses_init_endpoint
                    .then(|| {
                        init_redirect_urls(router_data.request.router_return_url.as_ref(), "Authorize")
                    })
                    .transpose()?;
                (Some(card), redirect, None, None)
            }
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::GooglePay(google_pay_data) => {
                    let token = google_pay_data
                        .tokenization_data
                        .get_encrypted_google_pay_token()
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "google_pay.tokenization_data.token",
                            context: datatrans_context(
                                "Datatrans Google Pay Authorize requires the encrypted Google Pay tokenization_data.token",
                            ),
                        })?;
                    let pay = serde_json::from_str::<DatatransGooglePayRequest>(&token)
                        .change_context(IntegrationError::InvalidWalletToken {
                            wallet_name: "Google Pay".to_string(),
                            context: datatrans_context(
                                "Datatrans Google Pay Authorize requires tokenization_data.token to be a JSON string containing signature, protocolVersion, signedMessage, and intermediateSigningKey",
                            ),
                        })?;
                    (None, None, Some(pay), None)
                }
                WalletData::ApplePay(wallet_data) => {
                    let token = wallet_data.get_applepay_decoded_payment_data()?;
                    let apl = serde_json::from_str::<DatatransApplePayRequest>(&token.expose())
                        .change_context(IntegrationError::InvalidWalletToken {
                            wallet_name: "Apple Pay".to_string(),
                            context: datatrans_context(
                                "Datatrans Apple Pay Authorize requires tokenization_data.token to be a JSON string containing data, header, signature, and version",
                            ),
                        })?;
                    (None, None, None, Some(apl))
                }
                WalletData::AliPayQr(_)
                | WalletData::AliPayRedirect(_)
                | WalletData::AliPayHkRedirect(_)
                | WalletData::BluecodeRedirect {}
                | WalletData::AmazonPayRedirect(_)
                | WalletData::MomoRedirect(_)
                | WalletData::KakaoPayRedirect(_)
                | WalletData::GoPayRedirect(_)
                | WalletData::GcashRedirect(_)
                | WalletData::ApplePayRedirect(_)
                | WalletData::ApplePayThirdPartySdk(_)
                | WalletData::DanaRedirect {}
                | WalletData::GrabpayRedirect {}
                | WalletData::GooglePayRedirect(_)
                | WalletData::GooglePayThirdPartySdk(_)
                | WalletData::MbWayRedirect(_)
                | WalletData::MobilePayRedirect(_)
                | WalletData::PaypalRedirect(_)
                | WalletData::PaypalSdk(_)
                | WalletData::Paze(_)
                | WalletData::SamsungPay(_)
                | WalletData::TwintRedirect {}
                | WalletData::VippsRedirect {}
                | WalletData::TouchNGoRedirect(_)
                | WalletData::WeChatPayRedirect(_)
                | WalletData::WeChatPayQr(_)
                | WalletData::CashappQr(_)
                | WalletData::SwishQr(_)
                | WalletData::Mifinity(_)
                | WalletData::RevolutPay(_)
                | WalletData::MbWay(_)
                | WalletData::Satispay(_)
                | WalletData::Wero(_)
                | WalletData::LazyPayRedirect(_)
                | WalletData::PhonePeRedirect(_)
                | WalletData::BillDeskRedirect(_)
                | WalletData::CashfreeRedirect(_)
                | WalletData::PayURedirect(_)
                | WalletData::EaseBuzzRedirect(_)
                | WalletData::QwikcilverWalletDirect(_)
                | WalletData::Skrill(_)
                | WalletData::Neteller(_)
                | WalletData::PaymayaRedirect(_)
                | WalletData::PayhereRedirect {} => Err(IntegrationError::NotImplemented(
                    domain_types::utils::get_unimplemented_payment_method_error_message(
                        "Datatrans",
                    ),
                    datatrans_context("Datatrans Authorize supports Google Pay and Apple Pay only"),
                ))?,
            }
            // Network-transaction-id and network-token carriers: the Datatrans public API
            // has no NTI or network-token field in or out, so this is a capability the
            // connector lacks (NotSupported), not one not yet built.
            PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_) => Err(IntegrationError::NotSupported {
                message: UNSUPPORTED_NETWORK_TRANSACTION_ID_ERROR.to_string(),
                connector: "datatrans",
                context: datatrans_context(
                    "Datatrans Authorize has no network transaction id / network token input; use a raw card or a stored Datatrans alias",
                ),
            })?,
            PaymentMethodData::CardRedirect(_)
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
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_) => Err(IntegrationError::NotImplemented(
                UNSUPPORTED_PAYMENT_METHOD_ERROR.to_string(),
                datatrans_context(
                    "Datatrans Authorize supports raw card, Secure Fields token, Google Pay and Apple Pay payment methods only",
                ),
            ))?,
        };

        // auto_settle mirrors is_auto_capture(): Automatic/SequentialAutomatic/None -> true,
        // Manual/ManualMultiple/Scheduled -> false. (SequentialAutomatic previously fell through
        // to None and let the connector default it — now correctly maps to true.)
        let auto_settle = Some(router_data.request.is_auto_capture());

        // Billing address and contact (AVS input) ride the card-shaped requests only: the
        // Card and PaymentMethodToken arms, on both the direct authorize and the init
        // endpoint. Wallet payload (PAY / APL) charges do not carry them.
        let (billing, customer) = if card.is_some() {
            (
                DatatransAddress::from_billing(&router_data.resource_common_data),
                DatatransCustomer::from_billing(
                    &router_data.resource_common_data,
                    router_data.request.email.as_ref(),
                ),
            )
        } else {
            (None, None)
        };

        // Shipping address rides every Authorize request (direct and init); an address
        // with no field set is omitted rather than sent as `shipping: {}`.
        let shipping = DatatransAddress::from_shipping(&router_data.resource_common_data);

        Ok(Self {
            currency: router_data.request.currency,
            refno: datatrans_refno(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )?,
            amount: Some(router_data.request.minor_amount),
            card,
            auto_settle,
            redirect,
            // CIT mandate registration asks Datatrans to persist a reusable alias
            // (surfaced later via PSync as `connector_mandate_id`); non-mandate Authorize
            // sends `option: null`.
            option: should_create_alias(
                &router_data.request.payment_method_data,
                is_mandate_payment,
            )
            .then_some(DatatransPaymentOptions {
                create_alias: Some(true),
            }),
            pay,
            apl,
            billing,
            customer,
            shipping,
        })
    }
}

/// Decides whether a `DatatransPaymentsRequest` should ask Datatrans to persist a
/// reusable alias (`option.createAlias = true`). Reused by every flow that builds
/// this request type (Authorize / SetupMandate).
///
/// Required iff both:
/// - the merchant declared mandate intent (`customer_acceptance` +
///   `setup_future_usage = off_session`, i.e. `is_mandate_payment()`), and
/// - the charged instrument is not itself already a Datatrans alias, i.e. a raw
///   card (`PLAIN`) charge. Wallet payload (`PAY` / `APL`) charges never set
///   `createAlias` — unchanged from the pre-refactor `is_card()` gating.
///
/// Never required for an `ALIAS` card charge or registration (Secure Fields token /
/// Google Pay `/v1/aliases/tokenize` alias): the alias was already created up front,
/// so `createAlias` is omitted there. MIT charges (`RepeatPayment`) reuse an existing
/// alias and likewise never set it.
fn should_create_alias<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
    is_mandate_payment: bool,
) -> bool {
    is_mandate_payment && matches!(payment_method_data, PaymentMethodData::Card(_))
}

/// Builds the optional `3D` object for a card request (Authorize / SetupMandate).
/// - external/passthrough 3DS (merchant supplied `authentication_data`) -> `Authentication`
/// - Datatrans-native 3DS (`auth_type == ThreeDs` with no external data, or a flow that
///   always drives a native challenge such as SetupMandate zero-auth alias registration,
///   signaled via `native_challenge`) -> `Cardholder`
/// - no 3DS -> `None`
///
/// `requestor` and `browser_info` are forwarded on the native (`Cardholder`) path only;
/// the Authorize native-3DS path supplies them, SetupMandate passes `None` for both.
fn build_three_ds_data(
    authentication_data: Option<&AuthenticationData>,
    resource_common_data: &PaymentFlowData,
    native_challenge: bool,
    requestor: Option<ThreeDSRequestorChallengeIndicator>,
    browser_info: Option<&BrowserInformation>,
) -> Result<Option<ThreeDSecureData>, error_stack::Report<IntegrationError>> {
    if let Some(auth_data) = authentication_data {
        let cavv = auth_data.cavv.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.cavv",
                context: datatrans_context(
                    "Datatrans passthrough external 3DS requires the cavv authentication value"
                ),
            })
        })?;
        Ok(Some(ThreeDSecureData::Authentication(ThreeDSData {
            three_ds_transaction_id: auth_data
                .threeds_server_transaction_id
                .clone()
                .map(Secret::new),
            cavv,
            eci: auth_data.eci.clone(),
            xid: auth_data.ds_trans_id.clone().map(Secret::new),
            three_ds_version: auth_data.message_version.as_ref().map(|v| v.to_string()),
            authentication_response: THREE_DS_AUTHENTICATION_RESPONSE_Y.to_string(),
        })))
    } else if native_challenge || resource_common_data.is_three_ds() {
        Ok(Some(ThreeDSecureData::Cardholder(ThreedsInfo {
            cardholder: CardHolder {
                cardholder_name: resource_common_data.get_billing_full_name()?,
                email: resource_common_data.get_billing_email()?,
                // Address fields are sent when available and never required.
                bill_addr_line1: resource_common_data.get_optional_billing_line1(),
                bill_addr_post_code: resource_common_data.get_optional_billing_zip(),
                bill_addr_city: resource_common_data.get_optional_billing_city(),
                bill_addr_state: resource_common_data.get_optional_billing_state(),
                bill_addr_country: resource_common_data
                    .get_optional_billing_country()
                    .map(|country| format!("{:03}", CountryAlpha2::to_numeric(country))),
                ship_addr_line1: resource_common_data.get_optional_shipping_line1(),
            },
            three_ds_requestor: requestor.map(|three_ds_requestor_challenge_ind| {
                ThreeDSRequestor {
                    three_ds_requestor_challenge_ind,
                }
            }),
            browser_information: browser_info
                .and_then(DatatransBrowserInformation::from_browser_info),
        })))
    } else {
        Ok(None)
    }
}

// Response card structure from tech spec
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCardResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked: Option<String>,
    /// Stored card token created when `option.createAlias=true` (SetupMandate/CIT).
    /// Surfaced from PSync as the `connector_mandate_id` that MIT/RepeatPayment reuses.
    /// Masked in logs; the domain `connector_mandate_id` boundary requires the plain value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<Secret<String>>,
}

// Authorize response — Datatrans returns either a settled/authorized transaction or,
// for native 3DS, a 3DS-enrolled response carrying the redirect transactionId.
// Variant order matters for `#[serde(untagged)]`: `ThreeDSResponse` requires the `3D`
// object and is tried first, so a plain transaction response (no `3D`) falls through
// to `TransactionResponse`. Connector errors (non-2xx) are handled by `build_error_response`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DatatransPaymentsResponse {
    ThreeDSResponse(Datatrans3DSResponse),
    TransactionResponse(DatatransSuccessResponse),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransSuccessResponse {
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<DatatransCardResponse>,
}

/// Native 3DS enrollment response. The `3D` object's presence discriminates this
/// variant from a plain transaction response; the cardholder must be redirected to
/// the Datatrans challenge page identified by `transaction_id`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Datatrans3DSResponse {
    pub transaction_id: String,
    #[serde(rename = "3D")]
    pub three_ds: ThreeDSEnrolled,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSEnrolled {
    /// Whether the card is enrolled in 3DS; drives untagged variant discrimination
    /// (a plain transaction response has no `3D`/`enrolled` field).
    pub enrolled: bool,
}

/// Derives the attempt status for a Datatrans Authorize response.
/// - Native 3DS responses need a cardholder challenge -> `AuthenticationPending`.
/// - A `{transactionId}` body from the init endpoint (`POST /v1/transactions`,
///   `uses_init_endpoint`) only created the transaction; money moves when the customer
///   completes the hosted page -> `AuthenticationPending`.
/// - A completed direct authorize (`POST /v1/transactions/authorize`) is `Charged` when
///   auto-captured (`autoSettle=true`), else `Authorized`.
// spec: Status Mappings — https://api-reference.datatrans.ch/#tag/v1transactions/operation/authorize
fn get_authorize_status(
    response: &DatatransPaymentsResponse,
    is_auto_capture: bool,
    uses_init_endpoint: bool,
) -> AttemptStatus {
    match response {
        DatatransPaymentsResponse::ThreeDSResponse(_) => AttemptStatus::AuthenticationPending,
        DatatransPaymentsResponse::TransactionResponse(_) if uses_init_endpoint => {
            AttemptStatus::AuthenticationPending
        }
        DatatransPaymentsResponse::TransactionResponse(_) => {
            if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        }
    }
}

/// Resolves the connector `MandateReference` for an Authorize/SetupMandate response.
///
/// The reusable Datatrans alias (the `connector_mandate_id` that MIT/RepeatPayment
/// later charges) comes from whichever is available, in order:
/// - the response echo (`card.alias`) — returned once the alias exists (a completed
///   `createAlias` CIT, or an echo of the charged alias), or
/// - the request's `payment_method_token`: a tokenize-sourced alias
///   (`POST /v1/aliases/tokenize`, e.g. Google Pay) is created upstream of the
///   Authorize / SetupMandate call, so the sent token is itself the mandate
///   reference. This is the only source on a 3DS-enrolled response, which carries no
///   `card` object.
///
/// The token fallback applies only to mandate payments: surfacing a mandate
/// reference for a plain one-off payment would misreport a single-use charge as
/// reusable.
fn connector_mandate_reference<T: PaymentMethodDataTypes>(
    response_card: Option<&DatatransCardResponse>,
    payment_method_data: &PaymentMethodData<T>,
    is_mandate_payment: bool,
) -> Option<Box<MandateReference>> {
    response_card
        .and_then(|card| card.alias.as_ref())
        .map(|alias| alias.peek().clone())
        .or_else(|| match payment_method_data {
            PaymentMethodData::PaymentMethodToken(token_data) if is_mandate_payment => {
                Some(token_data.token.peek().clone())
            }
            _ => None,
        })
        .map(|alias| {
            Box::new(MandateReference {
                connector_mandate_id: Some(alias),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<DatatransPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = item.router_data.request.is_auto_capture();
        let uses_init_endpoint = authorize_uses_init_endpoint(&item.router_data);
        let status = get_authorize_status(&item.response, is_auto_capture, uses_init_endpoint);

        let payments_response_data = match &item.response {
            // Init endpoint (`POST /v1/transactions`): a `{transactionId}` body without a `3D`
            // object only created the transaction; the customer must still complete the
            // hosted payment page, so it is mapped exactly like the 3DS-enrolled response.
            DatatransPaymentsResponse::TransactionResponse(response) if uses_init_endpoint => {
                let mandate_reference = connector_mandate_reference(
                    None,
                    &item.router_data.request.payment_method_data,
                    item.router_data.request.is_mandate_payment(),
                );
                let redirection_data = datatrans_start_redirect(
                    &item.router_data.resource_common_data.connectors.datatrans,
                    &response.transaction_id,
                )?;
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: Some(Box::new(redirection_data)),
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }
            }
            DatatransPaymentsResponse::TransactionResponse(response) => {
                let mandate_reference = connector_mandate_reference(
                    response.card.as_ref(),
                    &item.router_data.request.payment_method_data,
                    item.router_data.request.is_mandate_payment(),
                );

                // Non-3DS / passthrough external 3DS: no redirect. For raw-card mandate
                // CITs (createAlias), the alias may only be surfaced later via PSync,
                // not on this response.
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: response.acquirer_authorization_code.clone(),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }
            }
            DatatransPaymentsResponse::ThreeDSResponse(response) => {
                // Native 3DS: redirect the cardholder to the Datatrans challenge page.
                // The enrolled response carries no `card` object, but a tokenize-sourced
                // alias mandate survives the challenge: the alias already exists (created
                // by `/v1/aliases/tokenize`), so surface the mandate reference now rather
                // than relying on PSync to echo `card.alias` (Datatrans only echoes it on
                // `card_check`/createAlias syncs, not on a settled `payment`).
                let mandate_reference = connector_mandate_reference(
                    None,
                    &item.router_data.request.payment_method_data,
                    item.router_data.request.is_mandate_payment(),
                );
                let redirection_data = datatrans_start_redirect(
                    &item.router_data.resource_common_data.connectors.datatrans,
                    &response.transaction_id,
                )?;
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: Some(Box::new(redirection_data)),
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }
            }
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== SETUP MANDATE (ZERO-AUTH CIT) FLOW =====

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DatatransSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let refno = datatrans_refno(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        // Raw card without 3DS: zero-amount card check, which returns `card.alias`
        // synchronously — no hosted page, no billing or return URL needed.
        if setup_mandate_uses_validate(router_data) {
            if let PaymentMethodData::Card(card_data) = &router_data.request.payment_method_data {
                return Ok(Self::Validate(DatatransValidateRequest {
                    currency: router_data.request.currency,
                    refno,
                    card: DatatransValidateCard {
                        number: card_data.card_number.clone(),
                        expiry_month: card_data.card_exp_month.clone(),
                        expiry_year: card_data.get_card_expiry_year_2_digit()?,
                        cvv: card_data.card_cvc.clone(),
                    },
                }));
            }
        }

        // Init path (native 3DS on a raw card, or a Google Pay alias registration): the
        // customer completes the registration on the hosted page, so Datatrans-native 3DS
        // runs (`native_challenge: true`) with the cardholder details.
        // Shared by the raw-card (`PLAIN`) and the Google Pay alias (`ALIAS`) registration,
        // since SetupMandate never carries passthrough external authentication artifacts.
        // The call sits inside the match arms so an unsupported payment method still
        // reports `NotImplemented` rather than a missing-billing-field error.
        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => DatatransCard {
                alias: None,
                number: Some(card_data.card_number.clone()),
                expiry_month: Some(card_data.card_exp_month.clone()),
                expiry_year: Some(card_data.get_card_expiry_year_2_digit()?),
                cvv: Some(card_data.card_cvc.clone()),
                card_type: Some(CARD_TYPE_PLAIN.to_string()),
                // zero auth always runs native 3DS, so Datatrans can drive the ACS challenge with the cardholder details
                three_ds: build_three_ds_data(
                    None,
                    &router_data.resource_common_data,
                    true,
                    None,
                    None,
                )?,
            },
            // Google Pay zero-auth registration: the PaymentMethodToken flow already
            // tokenized the Google Pay payload into a Datatrans alias
            // (`POST /v1/aliases/tokenize`), and this token carries it. The alias is
            // registered as an `ALIAS` card — the Datatrans path that supports a native
            // 3DS challenge for Google Pay — and is itself the reusable mandate
            // reference, so no `createAlias` is requested (see `should_create_alias`).
            PaymentMethodData::PaymentMethodToken(token_data) => DatatransCard {
                alias: Some(token_data.token.clone()),
                number: None,
                // A tokenize-sourced alias carries no expiry/CVV of its own.
                expiry_month: None,
                expiry_year: None,
                cvv: None,
                card_type: Some(CARD_TYPE_ALIAS.to_string()),
                three_ds: build_three_ds_data(
                    None,
                    &router_data.resource_common_data,
                    true,
                    None,
                    None,
                )?,
            },
            // Same classification as Authorize: no NTI / network-token input exists on the
            // Datatrans API, so these carriers are NotSupported rather than NotImplemented.
            PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_) => Err(IntegrationError::NotSupported {
                message: UNSUPPORTED_NETWORK_TRANSACTION_ID_ERROR.to_string(),
                connector: "datatrans",
                context: datatrans_context(
                    "Datatrans SetupMandate has no network transaction id / network token input; register a raw card or a Google Pay alias",
                ),
            })?,
            PaymentMethodData::CardRedirect(_)
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
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_) => {
                Err(IntegrationError::NotImplemented(
                    UNSUPPORTED_PAYMENT_METHOD_ERROR.to_string(),
                    datatrans_context(
                        "Datatrans SetupMandate (zero-auth alias registration) supports raw card and Google Pay (tokenized alias) payment methods only",
                    ),
                ))?
            }
        };

        Ok(Self::Init(Box::new(DatatransPaymentsRequest {
            currency: router_data.request.currency,
            refno,
            // Zero-auth: no amount is charged; the field is omitted from the request.
            amount: None,
            card: Some(card),
            // Zero-auth alias creation cannot be manually captured.
            auto_settle: Some(true),
            redirect: Some(init_redirect_urls(
                router_data.request.router_return_url.as_ref(),
                "SetupMandate",
            )?),
            // Ask Datatrans to persist a reusable alias for later MIT/RepeatPayment.
            // SetupMandate is by construction the zero-auth mandate-registration
            // CIT, so the mandate intent holds trivially true here; the Google Pay
            // alias registration still opts out (its alias already exists).
            option: should_create_alias(&router_data.request.payment_method_data, true).then_some(
                DatatransPaymentOptions {
                    create_alias: Some(true),
                },
            ),
            pay: None,
            apl: None,
            billing: None,
            customer: None,
            shipping: None,
        })))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<DatatransPaymentsResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // The mapping keys on the endpoint that answered, not on the body: a validate
        // `{transactionId, acquirerAuthorizationCode, card.alias}` is a completed zero-amount
        // check (`Charged`), while an init `{transactionId}` only created the transaction
        // the customer still has to complete on the hosted page (`AuthenticationPending`).
        let uses_validate = setup_mandate_uses_validate(&item.router_data);
        let status = get_authorize_status(&item.response, true, !uses_validate);

        let payments_response_data = match &item.response {
            DatatransPaymentsResponse::TransactionResponse(response) if uses_validate => {
                // The alias is the whole point of the zero-amount check: a success without
                // `card.alias` fails closed instead of reporting a mandate with no id.
                let alias = response
                    .card
                    .as_ref()
                    .and_then(|card| card.alias.as_ref())
                    .ok_or_else(|| {
                        error_stack::report!(ConnectorError::ResponseHandlingFailed {
                            context: ResponseTransformationErrorContext {
                                http_status_code: Some(item.http_code),
                                additional_context: Some(
                                    "Datatrans /v1/transactions/validate succeeded without card.alias; no mandate can be registered"
                                        .to_string(),
                                ),
                            },
                        })
                    })?;
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: Some(Box::new(MandateReference {
                        connector_mandate_id: Some(alias.peek().clone()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    })),
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: response.acquirer_authorization_code.clone(),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }
            }
            DatatransPaymentsResponse::TransactionResponse(response) => {
                // Init endpoint (`POST /v1/transactions`): a `{transactionId}` body without a
                // `3D` object only created the transaction; nothing is registered until the
                // customer finishes the hosted page, so it is mapped like the 3DS-enrolled
                // response (redirect, never `Charged`). A Google Pay registration's alias
                // already exists (minted by `/v1/aliases/tokenize`), so the request token is
                // surfaced as the mandate reference; a raw-card alias is recovered via PSync.
                let mandate_reference = connector_mandate_reference(
                    None,
                    &item.router_data.request.payment_method_data,
                    true,
                );
                let redirection_data = datatrans_start_redirect(
                    &item.router_data.resource_common_data.connectors.datatrans,
                    &response.transaction_id,
                )?;
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: Some(Box::new(redirection_data)),
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }
            }
            DatatransPaymentsResponse::ThreeDSResponse(response) => {
                // A raw-card registration has no alias yet — a zero-auth `createAlias`
                // only materializes once the transaction completes after the challenge,
                // so the alias is recovered later via PSync (`card.alias`). A Google Pay
                // registration is different: the enrolled response carries no `card`
                // object, but its alias already exists (minted by
                // `/v1/aliases/tokenize`), so surface it now rather than relying on a
                // PSync echo — mirroring the Authorize GPay-alias path.
                let mandate_reference = connector_mandate_reference(
                    None,
                    &item.router_data.request.payment_method_data,
                    true,
                );
                // Native 3DS: redirect the cardholder to the Datatrans challenge page on the
                // configured payment-page host (`datatrans.secondary_base_url`).
                let redirection_data = datatrans_start_redirect(
                    &item.router_data.resource_common_data.connectors.datatrans,
                    &response.transaction_id,
                )?;
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: Some(Box::new(redirection_data)),
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }
            }
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REPEAT PAYMENT (MIT) FLOW =====

/// Datatrans expects a 2-digit `expiryYear`. The stored-card `card_exp_year` supplied in the
/// MIT request's additional card data may arrive as `YY` or `YYYY`; take the last two digits.
/// Vault template tokens (`{{...}}`) pass through unchanged for injector substitution.
fn additional_card_expiry_year_2_digit(
    additional_card: &AdditionalCardInfo,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let year = additional_card.card_exp_year.clone().ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "additional_payment_data.card.card_exp_year",
            context: datatrans_context(
                "Datatrans MIT requires the stored card expiry year for the alias charge",
            ),
        })
    })?;
    let year_value = year.peek();
    let two_digit = if year_value.contains("{{") {
        year_value.to_string()
    } else {
        year_value
            .get(year_value.len().saturating_sub(2)..)
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "additional_payment_data.card.card_exp_year",
                    context: datatrans_context("Expected expiry year format: YY or YYYY"),
                })
            })?
            .to_string()
    };
    Ok(Secret::new(two_digit))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DatatransPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // MIT reuses the Datatrans alias persisted by SetupMandate (createAlias=true), surfaced
        // to HS as the `connector_mandate_id`. Only the connector-stored alias path is chargeable
        // here; network-transaction-id / network-token MIT is not supported by this connector.
        let alias = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_id) => connector_mandate_id
                .get_connector_mandate_id()
                .ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "mandate_reference.connector_mandate_id",
                        context: datatrans_context(
                            "Datatrans MIT requires the stored alias/connector_mandate_id created by SetupMandate",
                        ),
                    })
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                // Datatrans MIT can only charge a connector-stored alias; scheme-level
                // network-transaction-id / network-token mandates are not a Datatrans
                // capability (NotSupported, not merely not-yet-built).
                Err(IntegrationError::NotSupported {
                    message: UNSUPPORTED_MANDATE_REFERENCE_ERROR.to_string(),
                    connector: "datatrans",
                    context: datatrans_context(
                        "Datatrans MIT charges the stored connector alias only; network-transaction-id / network-token mandates are unsupported",
                    ),
                })?
            }
        };

        // Card expiry for the alias charge comes from the stored card's additional data
        // (there is no PAN in a MIT request). MIT requests may carry
        // `PaymentMethodData::MandatePayment`, so use the retained payment_method_type to
        // identify Google Pay wallet aliases.
        let (expiry_month, expiry_year) = match router_data.request.payment_method_type {
            Some(common_enums::PaymentMethodType::GooglePay)
            | Some(common_enums::PaymentMethodType::ApplePay) => (None, None),
            _ => {
                let additional_card = match &router_data.request.additional_payment_data {
                    Some(AdditionalPaymentData::Card(card)) => card,
                    None => Err(error_stack::report!(
                        IntegrationError::MissingRequiredField {
                            field_name: "additional_payment_data.card",
                            context: datatrans_context(
                                "Datatrans MIT requires the stored card details (additional_payment_data.card) for the alias charge",
                            ),
                        }
                    ))?,
                };

                let expiry_month = additional_card.card_exp_month.clone().ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "additional_payment_data.card.card_exp_month",
                        context: datatrans_context(
                            "Datatrans MIT requires the stored card expiry month for the alias charge",
                        ),
                    })
                })?;
                let expiry_year = additional_card_expiry_year_2_digit(additional_card)?;
                (Some(expiry_month), Some(expiry_year))
            }
        };
        let card = DatatransCard {
            alias: Some(Secret::new(alias)),
            expiry_month,
            expiry_year,
            number: None,
            cvv: None,
            card_type: Some(CARD_TYPE_ALIAS.to_string()),
            // MIT charges an already-3DS-authenticated alias; no cardholder challenge / redirect.
            three_ds: None,
        };

        Ok(Self {
            currency: router_data.request.currency,
            refno: datatrans_refno(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )?,
            amount: Some(router_data.request.minor_amount),
            card: Some(card),
            // auto_settle mirrors is_auto_capture(): Automatic/SequentialAutomatic/None -> true,
            // Manual/ManualMultiple/Scheduled -> false.
            auto_settle: Some(router_data.request.is_auto_capture()),
            // MIT never redirects and never re-creates an alias: the charged instrument is
            // the stored alias itself, so `should_create_alias` trivially holds false here.
            redirect: None,
            option: None,
            pay: None,
            apl: None,
            billing: None,
            customer: None,
            shipping: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<DatatransPaymentsResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = item.router_data.request.is_auto_capture();
        // MIT always uses the direct `/v1/transactions/authorize` endpoint.
        let status = get_authorize_status(&item.response, is_auto_capture, false);

        let payments_response_data = match &item.response {
            DatatransPaymentsResponse::TransactionResponse(response) => {
                // Charged (auto-capture) or Authorized: the alias charge settles immediately;
                // no redirect, and the mandate_reference is not re-surfaced on a MIT charge.
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: response.acquirer_authorization_code.clone(),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }
            }
            DatatransPaymentsResponse::ThreeDSResponse(response) => {
                // Not expected for MIT (the alias is already 3DS-authenticated), but the untagged
                // response can technically carry it; surface the challenge redirect defensively
                // rather than treating it as a settled transaction.
                let redirection_data = datatrans_start_redirect(
                    &item.router_data.resource_common_data.connectors.datatrans,
                    &response.transaction_id,
                )?;
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: Some(Box::new(redirection_data)),
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }
            }
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PSYNC FLOW STRUCTURES =====

// PSync Request - Empty for GET-based endpoint
#[derive(Debug, Serialize)]
pub struct DatatransSyncRequest;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for DatatransSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::DatatransRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request body for GET-based sync endpoint
        Ok(Self)
    }
}

// Payment Status Enumeration from Datatrans API.
// Datatrans emits snake_case statuses (e.g. `challenge_ongoing`).
#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatatransPaymentStatus {
    Initialized,
    Authenticated,
    Authorized,
    Settled,
    Transmitted,
    Canceled,
    Failed,
    /// 3DS challenge is in progress — the cardholder has not finished the ACS challenge.
    ChallengeOngoing,
    /// 3DS challenge is required before the transaction can proceed.
    ChallengeRequired,
    /// Any status value Datatrans does not document; parsed instead of failing
    /// deserialization and always resolved non-terminally (`Pending`).
    #[serde(other)]
    Unknown,
}

/// Datatrans transaction `type` reported on a sync response. Datatrans emits
/// snake_case values. The type is required to interpret `status` correctly, because
/// the same status means different things across transaction kinds (see
/// [`sync_attempt_status`]).
#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatatransTransactionType {
    /// Standard Authorize/Capture payment transaction.
    Payment,
    /// Refund/credit transaction. Not an attempt-status carrier — refunds are tracked
    /// via `RefundStatus`/RSync, so for `AttemptStatus` this maps to `Failure`
    /// (mirrors the HS Direct reference).
    Credit,
    /// Zero-auth mandate alias creation (`option.createAlias=true`). A completed
    /// `card_check` has no capture step, so `authorized`/`settled`/`transmitted` all
    /// mean the alias was successfully created (→ `Charged`).
    CardCheck,
    /// Any transaction type Datatrans does not document; parsed instead of failing
    /// deserialization and always resolved non-terminally (`Pending`).
    #[serde(other)]
    Unknown,
}

/// Derives the PSync `AttemptStatus` from BOTH the Datatrans transaction `type` and its
/// `status`, mirroring the HS Direct reference (`impl From<SyncResponse> for AttemptStatus`).
///
/// The mapping is type-aware because a status alone is ambiguous:
/// - `Payment`: `Authorized` stays `Authorized` (a manual-capture auth must not read as
///   captured until Capture settles it — capture-method-aware); `Settled`/`Transmitted` →
///   `Charged`.
/// - `CardCheck` (zero-auth mandate): `Authorized`/`Settled`/`Transmitted` all → `Charged`,
///   because a completed alias creation is a success with no separate capture step. This is
///   what makes a finished zero-auth mandate read as succeeded.
/// - `Credit` (refund): `Failure` for `AttemptStatus` (refunds handled via RSync).
///
/// - `Unknown` type or `Unknown` status (an undocumented value): `Pending`, never terminal.
///
/// Each per-type `status` match is exhaustive (no wildcard) so a new `DatatransPaymentStatus`
/// variant fails to compile rather than silently defaulting.
// spec: Status Mappings — https://api-reference.datatrans.ch/#tag/v1transactions/operation/status
fn sync_attempt_status(
    transaction_type: &DatatransTransactionType,
    status: DatatransPaymentStatus,
) -> AttemptStatus {
    match transaction_type {
        DatatransTransactionType::Payment => match status {
            DatatransPaymentStatus::Authorized => AttemptStatus::Authorized,
            DatatransPaymentStatus::Settled | DatatransPaymentStatus::Transmitted => {
                AttemptStatus::Charged
            }
            DatatransPaymentStatus::ChallengeOngoing
            | DatatransPaymentStatus::ChallengeRequired => AttemptStatus::AuthenticationPending,
            DatatransPaymentStatus::Canceled => AttemptStatus::Voided,
            DatatransPaymentStatus::Failed => AttemptStatus::Failure,
            DatatransPaymentStatus::Initialized
            | DatatransPaymentStatus::Authenticated
            | DatatransPaymentStatus::Unknown => AttemptStatus::Pending,
        },
        DatatransTransactionType::CardCheck => match status {
            DatatransPaymentStatus::Settled
            | DatatransPaymentStatus::Transmitted
            | DatatransPaymentStatus::Authorized => AttemptStatus::Charged,
            DatatransPaymentStatus::ChallengeOngoing
            | DatatransPaymentStatus::ChallengeRequired => AttemptStatus::AuthenticationPending,
            DatatransPaymentStatus::Canceled => AttemptStatus::Voided,
            DatatransPaymentStatus::Failed => AttemptStatus::Failure,
            DatatransPaymentStatus::Initialized
            | DatatransPaymentStatus::Authenticated
            | DatatransPaymentStatus::Unknown => AttemptStatus::Pending,
        },
        DatatransTransactionType::Credit => AttemptStatus::Failure,
        DatatransTransactionType::Unknown => AttemptStatus::Pending,
    }
}

// History entry structure from tech spec
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransHistoryEntry {
    pub action: String,
    pub amount: Option<MinorUnit>,
    pub success: bool,
    pub date: String,
}

// PSync Response structure based on tech spec GET /v1/transactions/{transactionId}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransSyncResponse {
    pub transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: DatatransTransactionType,
    pub status: DatatransPaymentStatus,
    // Optional: the reference does not require these and a minimal Datatrans sync body may
    // omit them; keeping them optional avoids a deserialization failure on such responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<DatatransTransactionDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<DatatransCardResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<DatatransHistoryEntry>>,
}

// Transaction detail structure
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransTransactionDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorize: Option<DatatransActionDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle: Option<DatatransActionDetail>,
    /// Failure detail present on a failed transaction; surfaced as the connector error
    /// code/message so a failed sync reports the reason (mirrors HS Direct).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail: Option<DatatransFailDetail>,
}

// Failure detail block from a failed Datatrans transaction sync.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransFailDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// Action detail structure
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransActionDetail {
    /// Amount for this action, in minor units. Optional: a `card_check` (zero-auth
    /// mandate) transaction's `detail.authorize` carries only the
    /// `acquirerAuthorizationCode` and no `amount`, whereas a `payment`/`settle`
    /// action does include it. `Option` accepts both shapes so PSync deserialization
    /// no longer fails on a card_check sync response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Map Datatrans status to UCS status, type-aware: a `card_check` (zero-auth
        // mandate) `authorized` means the alias was created successfully (→ Charged),
        // whereas a `payment` `authorized` is only an authorization (→ Authorized).
        let status = sync_attempt_status(&response.transaction_type, response.status.clone());

        // On a failed sync, surface the connector failure detail (code/message/reason) instead
        // of a silent Failure with no error — mirrors HS Direct.
        let response = if status == AttemptStatus::Failure {
            let (code, message) = match response.detail.as_ref().and_then(|d| d.fail.as_ref()) {
                Some(fail) => (
                    fail.reason
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    fail.message
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                ),
                None => (NO_ERROR_CODE.to_string(), NO_ERROR_MESSAGE.to_string()),
            };
            Err(ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: item.http_code,
                // Documented terminal `failed` status: mirrors the Failure status of the sync.
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                connector_transaction_id: Some(response.transaction_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            // The merchant reference (`refno`) echoed by the status call is the
            // connector_response_reference_id; resource_id stays the transactionId.
            let connector_response_reference_id = response.refno.clone();

            // Datatrans returns the stored-card `alias` on sync once `createAlias` succeeded
            // (SetupMandate/CIT). Surface it as the `connector_mandate_id` that MIT reuses.
            // `.peek()` exposes the value only at the domain `connector_mandate_id` boundary.
            // Only surfaced on a non-failure sync (a failed transaction has no usable alias).
            let mandate_reference = response
                .card
                .as_ref()
                .and_then(|card| card.alias.as_ref())
                .map(|alias| MandateReference {
                    connector_mandate_id: Some(alias.peek().clone()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                });

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            response,
            ..item.router_data.clone()
        })
    }
}

// ===== CAPTURE FLOW STRUCTURES =====

// Capture Request structure based on tech spec POST /v1/transactions/{transactionId}/settle
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCaptureRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub refno: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno2: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for DatatransCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        // Get the amount to capture from minor_amount_to_capture
        let amount = router_data.request.minor_amount_to_capture;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            refno: datatrans_refno(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )?,
            refno2: None,
        })
    }
}

// Capture Response
// Settle answers 204 No Content (spec "Settle a Transaction"); the framework parses the
// empty body as `{}`, so both fields are optional and normally absent.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCaptureResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Settle returns 204 with no body: the resource id is the settled transaction's own id,
        // taken from the request when the (empty) response does not echo it.
        let transaction_id = match item.response.transaction_id.clone() {
            Some(transaction_id) => transaction_id,
            None => item
                .router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(ConnectorError::ResponseHandlingFailed {
                    context: ResponseTransformationErrorContext {
                        http_status_code: Some(item.http_code),
                        additional_context: Some(
                            "Datatrans settle: no transactionId in the response and none on the capture request"
                                .to_string(),
                        ),
                    },
                })?,
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: item.response.acquirer_authorization_code.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged, // Successful capture means payment is charged
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..item.router_data.clone()
        })
    }
}

// ===== REFUND FLOW STRUCTURES =====

// Refund Request structure based on tech spec POST /v1/transactions/{transactionId}/credit
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransRefundRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub refno: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno2: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for DatatransRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        // Get the refund amount from RefundsData
        let amount = router_data.request.minor_refund_amount;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            // Send the refund's own id as the Datatrans `refno` (mirrors HS Direct, which uses
            // `refund_id`), so the credit is reconciled against the refund rather than the payment.
            refno: datatrans_refno(&router_data.request.refund_id)?,
            refno2: None,
        })
    }
}

// Refund Response structure based on tech spec
// The credit endpoint returns 200 with transaction details on success
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransRefundResponse {
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Datatrans credit endpoint returns 200 on success with transaction details
        // The refund is successful when we get a 200 response with transactionId
        let refunds_response_data = RefundsResponseData {
            connector_refund_id: item.response.transaction_id.clone(),
            refund_status: RefundStatus::Success, // 200 response indicates successful refund
            status_code: item.http_code,
            acquirer_reference_number: None,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..item.router_data
        })
    }
}

// ===== REFUND SYNC (RSync) FLOW STRUCTURES =====

// RSync Request - Empty for GET-based endpoint
#[derive(Debug, Serialize)]
pub struct DatatransRefundSyncRequest;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for DatatransRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::DatatransRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request body for GET-based sync endpoint
        Ok(Self)
    }
}

// Refund Status Enumeration from Datatrans API
/// Type-aware refund-sync status mapping, mirroring HS Direct `From<SyncResponse> for RefundStatus`.
/// A refund settles under the `credit` transaction type; a `payment`/`card_check` transaction
/// synced on the refund endpoint is not a refund and maps to `Failure`. The full
/// `DatatransPaymentStatus` enum is used so credit transactions in challenge/authorized/canceled
/// states map correctly instead of failing to deserialize. An undocumented (`Unknown`) type or
/// credit status resolves to `Pending`, never terminally.
// spec: Status Mappings — https://api-reference.datatrans.ch/#tag/v1transactions/operation/status
fn sync_refund_status(
    transaction_type: &DatatransTransactionType,
    status: DatatransPaymentStatus,
) -> RefundStatus {
    match transaction_type {
        DatatransTransactionType::Credit => match status {
            DatatransPaymentStatus::Settled | DatatransPaymentStatus::Transmitted => {
                RefundStatus::Success
            }
            DatatransPaymentStatus::ChallengeOngoing
            | DatatransPaymentStatus::ChallengeRequired
            | DatatransPaymentStatus::Unknown => RefundStatus::Pending,
            DatatransPaymentStatus::Initialized
            | DatatransPaymentStatus::Authenticated
            | DatatransPaymentStatus::Authorized
            | DatatransPaymentStatus::Canceled
            | DatatransPaymentStatus::Failed => RefundStatus::Failure,
        },
        DatatransTransactionType::Payment | DatatransTransactionType::CardCheck => {
            RefundStatus::Failure
        }
        DatatransTransactionType::Unknown => RefundStatus::Pending,
    }
}

// RSync Response structure - uses the same shape as payment sync but for a refund (credit)
// transaction. `currency`/`refno` are optional (a minimal credit body may omit them).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransRefundSyncResponse {
    pub transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: DatatransTransactionType,
    pub status: DatatransPaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno2: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Map Datatrans refund status to UCS RefundStatus, type-aware (credit vs payment/card_check).
        let refund_status = sync_refund_status(&response.transaction_type, response.status.clone());

        let refunds_response_data = RefundsResponseData {
            connector_refund_id: response.transaction_id.clone(),
            refund_status,
            status_code: item.http_code,
            acquirer_reference_number: None,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..item.router_data
        })
    }
}

// ===== VOID FLOW STRUCTURES =====

// Void Request structure based on tech spec POST /v1/transactions/{transactionId}/cancel
// The tech spec shows "object (CancelRequest)" as request body which appears to be empty/optional
// Using an empty struct to serialize as {} instead of null
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransVoidRequest {
    // Empty struct - will serialize as {} instead of null
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for DatatransVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::DatatransRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request body for cancel endpoint based on tech spec
        // The CancelRequest object appears to be empty - serializes as {}
        Ok(Self {})
    }
}

// Void Response
// Note: API spec says 204 No Content, but Datatrans actually returns 200 with a JSON body
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransVoidResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Datatrans returns 200 with JSON body for successful void
        // Use transaction_id from response if available, otherwise fall back to request
        let transaction_id = item
            .response
            .transaction_id
            .clone()
            .unwrap_or_else(|| item.router_data.request.connector_transaction_id.clone());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: item.response.acquirer_authorization_code.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided, // Successful void/cancel means payment is voided
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..item.router_data.clone()
        })
    }
}

// ===== VOID POST CAPTURE (REVERSE) FLOW STRUCTURES =====

// VoidPC Request structure based on tech spec POST /v1/transactions/{transactionId}/cancel
// Datatrans cancel endpoint works on both authorized and settled (captured) transactions.
// The request body is empty — same as the regular Void flow.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransVoidPCRequest {
    // Empty struct - serializes as {}
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DatatransVoidPCRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::DatatransRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request body for cancel endpoint — same as regular Void
        Ok(Self {})
    }
}

// VoidPC Response
// Datatrans cancel endpoint returns 204 No Content with an empty body on success;
// it does not echo a transactionId, acquirerAuthorizationCode, or status field.
// Error responses (4xx/5xx) are handled separately by `build_error_response`.
// The framework parses an empty body as `{}`, which deserializes to this empty struct.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DatatransVoidPCResponse {}

impl TryFrom<ResponseRouterData<DatatransVoidPCResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransVoidPCResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payments_response_data = PaymentsResponseData::PostCaptureVoidResponse {
            post_capture_void_status: PostCaptureVoidStatus::Succeeded,
            connector_reference_id: Some(item.router_data.request.connector_transaction_id.clone()),
            description: None,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(payments_response_data),
            ..item.router_data
        })
    }
}

// ===== CLIENT AUTHENTICATION TOKEN FLOW STRUCTURES =====

/// Request to initialize a Datatrans Secure Fields transaction.
/// Returns a transactionId that serves as a client authentication token.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransClientAuthRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub return_url: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DatatransClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        Ok(Self {
            amount: router_data.request.amount,
            currency: router_data.request.currency,
            return_url: router_data
                .resource_common_data
                .return_url
                .clone()
                .unwrap_or_else(|| "https://example.com/return".to_string()),
        })
    }
}

/// Datatrans Secure Fields init response — contains the transactionId
/// used as a client authentication token (valid for 30 minutes).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransClientAuthResponse {
    pub transaction_id: String,
}

impl TryFrom<ResponseRouterData<DatatransClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DatatransClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Datatrans(
                DatatransClientAuthenticationResponseDomain {
                    transaction_id: Secret::new(response.transaction_id),
                },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== PAYMENT METHOD TOKEN (GOOGLE PAY ALIAS TOKENIZATION) FLOW STRUCTURES =====
// POST /v1/aliases/tokenize converts the Google Pay payload into a Datatrans alias.
// The alias is then charged as an `ALIAS` card in Authorize — the Datatrans path
// that supports a native 3DS challenge for Google Pay.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransTokenizeRequest {
    pub requests: Vec<DatatransTokenizeRequestItem>,
}

/// A single tokenization request item. The `type` discriminator is always
/// `"GOOGLE_PAY"` for this connector (Datatrans also supports CARD/CVV/CUSTOM items,
/// which this connector does not tokenize).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransTokenizeRequestItem {
    #[serde(rename = "type")]
    pub item_type: String,
    /// The full Google Pay payment-data token, forwarded verbatim as a JSON string.
    pub token: String,
}

/// `type` value of a Google Pay item in a `/v1/aliases/tokenize` request.
const TOKENIZE_ITEM_TYPE_GOOGLE_PAY: &str = "GOOGLE_PAY";

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for DatatransTokenizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => {
                let token = google_pay_data
                    .tokenization_data
                    .get_encrypted_google_pay_token()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "google_pay.tokenization_data.token",
                        context: datatrans_context(
                            "Datatrans Google Pay tokenization requires the encrypted Google Pay tokenization_data.token",
                        ),
                    })?;
                Ok(Self {
                    requests: vec![DatatransTokenizeRequestItem {
                        item_type: TOKENIZE_ITEM_TYPE_GOOGLE_PAY.to_string(),
                        token,
                    }],
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                UNSUPPORTED_PAYMENT_METHOD_ERROR.to_string(),
                datatrans_context(
                    "Datatrans alias tokenization (/v1/aliases/tokenize) supports Google Pay wallets only",
                ),
            ))?,
        }
    }
}

/// Response of POST /v1/aliases/tokenize. Datatrans replies with the bulk container
/// even for a single request; per-item success or failure is carried on each entry
/// (an HTTP 200 response can still contain per-item errors).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransTokenizeResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<DatatransTokenizeOverview>,
    pub responses: Vec<DatatransTokenizeResponseItem>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransTokenizeOverview {
    pub total: u32,
    pub successful: u32,
    pub failed: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransTokenizeResponseItem {
    /// The tokenized alias, charged later as `card.alias`. Masked in logs; the
    /// domain token boundary requires the plain value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<Secret<String>>,
    /// Per-item failure detail (e.g. an invalid Google Pay payload) when the item
    /// could not be tokenized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DatatransErrorDetail>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<DatatransTokenizeResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransTokenizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // We always send exactly one item, so its echo entry decides the outcome.
        match item.response.responses.first() {
            Some(DatatransTokenizeResponseItem {
                alias: Some(alias), ..
            }) => Ok(Self {
                response: Ok(PaymentMethodTokenResponse {
                    token: alias.peek().to_owned(),
                    connector_payment_method_id: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            }),
            Some(DatatransTokenizeResponseItem {
                error: Some(error), ..
            }) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            }),
            // Neither alias nor error on the echoed item — contract violation.
            _ => Err(error_stack::report!(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "Datatrans /v1/aliases/tokenize response item carries neither an alias nor an error"
                            .to_string(),
                    ),
                },
            })),
        }
    }
}

// ===== INCOMING WEBHOOK STRUCTURES =====

/// Header carrying the Datatrans webhook signature: `Datatrans-Signature: t=<timestamp>,s0=<hex>`.
const DATATRANS_SIGNATURE_HEADER: &str = "datatrans-signature";

/// Parsed `Datatrans-Signature` header.
///
/// spec: "Webhook Authentication & Signature Verification" —
/// https://docs.datatrans.ch/docs/api-webhooks. The signed message is
/// `String(timestamp) + rawPayload` (UTF-8), the key is the dashboard HMAC key
/// **hex-decoded** to bytes, the algorithm is HMAC-SHA256 and `s0` is the lowercase hex
/// digest.
#[derive(Debug, Clone)]
pub struct DatatransWebhookSignature {
    /// The `t=` part, used verbatim as the preimage prefix.
    pub timestamp: String,
    /// The `s0=` part, hex-decoded.
    pub s0: Vec<u8>,
}

impl DatatransWebhookSignature {
    /// Reads the `Datatrans-Signature` header (name matched case-insensitively) and parses
    /// `t=<ts>,s0=<hex>`. A missing header, a missing `t`/`s0` part or a non-hex `s0`
    /// yields `None`, so source verification fails closed.
    pub fn from_headers(headers: &HashMap<String, String>) -> Option<Self> {
        let header_value = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(DATATRANS_SIGNATURE_HEADER))
            .map(|(_, value)| value)?;

        let mut timestamp = None;
        let mut s0_hex = None;
        for part in header_value.split(',') {
            if let Some((key, value)) = part.split_once('=') {
                match key.trim() {
                    "t" => timestamp = Some(value.trim().to_string()),
                    "s0" => s0_hex = Some(value.trim()),
                    _ => {}
                }
            }
        }

        let timestamp = timestamp.filter(|t| !t.is_empty())?;
        let s0 = hex::decode(s0_hex?).ok()?;
        Some(Self { timestamp, s0 })
    }

    /// Signed preimage: timestamp bytes followed by the raw (never re-serialized) body.
    pub fn message(&self, raw_body: &[u8]) -> Vec<u8> {
        let mut message = self.timestamp.as_bytes().to_vec();
        message.extend_from_slice(raw_body);
        message
    }
}

/// Decodes a Datatrans webhook body. The webhook carries the same Status payload as
/// `GET /v1/transactions/{transactionId}` (spec: "Webhook Payload Structure"), so it is
/// decoded into [`DatatransSyncResponse`].
pub fn parse_datatrans_webhook_body(
    body: &[u8],
) -> Result<DatatransSyncResponse, error_stack::Report<WebhookError>> {
    serde_json::from_slice::<DatatransSyncResponse>(body)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

/// Derives the webhook `EventType` from the payload `type` + `status` (Datatrans sends no
/// explicit event field — spec: "Event Types"). Reuses the sync mappers so a webhook and a
/// PSync/RSync of the same transaction always agree.
pub fn datatrans_webhook_event_type(body: &DatatransSyncResponse) -> EventType {
    match body.transaction_type {
        DatatransTransactionType::Credit => {
            match sync_refund_status(&body.transaction_type, body.status.clone()) {
                RefundStatus::Success => EventType::RefundSuccess,
                RefundStatus::Failure => EventType::RefundFailure,
                RefundStatus::Pending
                | RefundStatus::ManualReview
                | RefundStatus::TransactionFailure
                | RefundStatus::Unknown => EventType::RefundProcessing,
            }
        }
        DatatransTransactionType::Payment
        | DatatransTransactionType::CardCheck
        | DatatransTransactionType::Unknown => {
            match sync_attempt_status(&body.transaction_type, body.status.clone()) {
                AttemptStatus::Charged => EventType::PaymentIntentSuccess,
                AttemptStatus::Authorized => EventType::PaymentIntentAuthorizationSuccess,
                AttemptStatus::Failure => EventType::PaymentIntentFailure,
                AttemptStatus::Voided => EventType::PaymentIntentCancelled,
                AttemptStatus::AuthenticationPending => EventType::PaymentActionRequired,
                // `Pending` plus every status `sync_attempt_status` never returns: all
                // resolve non-terminally.
                AttemptStatus::Pending
                | AttemptStatus::Started
                | AttemptStatus::AuthenticationFailed
                | AttemptStatus::RouterDeclined
                | AttemptStatus::AuthenticationSuccessful
                | AttemptStatus::PartiallyAuthorized
                | AttemptStatus::AuthorizationFailed
                | AttemptStatus::Authorizing
                | AttemptStatus::CodInitiated
                | AttemptStatus::Expired
                | AttemptStatus::VoidedPostCapture
                | AttemptStatus::VoidInitiated
                | AttemptStatus::VoidPostCaptureInitiated
                | AttemptStatus::CaptureInitiated
                | AttemptStatus::CaptureFailed
                | AttemptStatus::VoidFailed
                | AttemptStatus::AutoRefunded
                | AttemptStatus::PartialCharged
                | AttemptStatus::PartialChargedAndChargeable
                | AttemptStatus::Unresolved
                | AttemptStatus::Unspecified
                | AttemptStatus::PaymentMethodAwaited
                | AttemptStatus::ConfirmationAwaited
                | AttemptStatus::DeviceDataCollectionPending
                | AttemptStatus::IntegrityFailure
                | AttemptStatus::Unknown => EventType::PaymentIntentProcessing,
            }
        }
    }
}

/// Resource reference for the stateless ParseEvent phase: a `credit` carries the refund's own
/// `transactionId` (never overwriting the payment id), everything else is a payment.
pub fn datatrans_webhook_reference(body: &DatatransSyncResponse) -> WebhookResourceReference {
    match body.transaction_type {
        DatatransTransactionType::Credit => {
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: Some(body.transaction_id.clone()),
                merchant_refund_id: body.refno.clone(),
                connector_transaction_id: None,
                merchant_transaction_id: None,
            })
        }
        DatatransTransactionType::Payment
        | DatatransTransactionType::CardCheck
        | DatatransTransactionType::Unknown => {
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: Some(body.transaction_id.clone()),
                merchant_transaction_id: body.refno.clone(),
            })
        }
    }
}

/// Builds the payment webhook details, mirroring the PSync mapping (status, alias mandate,
/// failure detail). A `credit` body is not a payment event.
pub fn build_datatrans_payment_webhook_details(
    body: &DatatransSyncResponse,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    if matches!(body.transaction_type, DatatransTransactionType::Credit) {
        return Err(error_stack::report!(WebhookError::WebhookProcessingFailed));
    }

    let status = sync_attempt_status(&body.transaction_type, body.status.clone());

    let (error_code, error_message, error_reason) = if status == AttemptStatus::Failure {
        let (code, message) = match body.detail.as_ref().and_then(|d| d.fail.as_ref()) {
            Some(fail) => (
                fail.reason
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                fail.message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            ),
            None => (NO_ERROR_CODE.to_string(), NO_ERROR_MESSAGE.to_string()),
        };
        (Some(code), Some(message.clone()), Some(message))
    } else {
        (None, None, None)
    };

    // The stored-card alias surfaces as the connector mandate id, exactly as on PSync;
    // a failed transaction has no usable alias.
    let mandate_reference = if status == AttemptStatus::Failure {
        None
    } else {
        body.card
            .as_ref()
            .and_then(|card| card.alias.as_ref())
            .map(|alias| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(alias.peek().clone()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })
            })
    };

    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(
            body.transaction_id.clone(),
        )),
        status,
        connector_response_reference_id: body.refno.clone(),
        connector_request_reference_id: None,
        mandate_reference,
        error_code,
        error_message,
        error_reason,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: None,
        minor_amount_captured: None,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

/// Builds the refund webhook details from a `credit` body, mirroring the RSync mapping.
pub fn build_datatrans_refund_webhook_details(
    body: &DatatransSyncResponse,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    if !matches!(body.transaction_type, DatatransTransactionType::Credit) {
        return Err(error_stack::report!(WebhookError::WebhookProcessingFailed));
    }

    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(body.transaction_id.clone()),
        merchant_transaction_id: None,
        status: sync_refund_status(&body.transaction_type, body.status.clone()),
        connector_response_reference_id: None,
        error_code: None,
        error_message: None,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}
