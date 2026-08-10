use std::collections::HashMap;

use crate::types::ResponseRouterData;
use base64::{engine::general_purpose::STANDARD, Engine};
use common_enums::{AttemptStatus, Currency, PostCaptureVoidStatus, RefundStatus};
use common_utils::{pii::Email, request::Method, MinorUnit};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, RSync, Refund, RepeatPayment,
        SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        DatatransClientAuthenticationResponse as DatatransClientAuthenticationResponseDomain,
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    types::{AdditionalCardInfo, AdditionalPaymentData},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// Error message constants
const DEFAULT_ERROR_CODE: &str = "UNKNOWN_ERROR";
const DEFAULT_ERROR_MESSAGE: &str = "Unknown error occurred";
/// Code used when Datatrans returns a non-JSON error body (e.g. an HTML gateway error page)
/// that carries no structured error code. Mirrors HS Direct's HTML fallback.
const NO_ERROR_CODE: &str = "NO_ERROR_CODE";
const UNSUPPORTED_PAYMENT_METHOD_ERROR: &str =
    "Only card, Google Pay and Apple Pay payments are supported for Datatrans";

/// Datatrans hosted redirect/challenge host — sandbox environment
/// (paired with the `api.sandbox.datatrans.com` API base_url).
const REDIRECTION_SBX_URL: &str = "https://pay.sandbox.datatrans.com";
/// Datatrans hosted redirect/challenge host — production environment.
const REDIRECTION_PROD_URL: &str = "https://pay.datatrans.com";

/// Selects the Datatrans hosted challenge host that mirrors the API `base_url`
/// environment: the sandbox API base (`api.sandbox.datatrans.com`) pairs with
/// `pay.sandbox.datatrans.com`, and production with `pay.datatrans.com`.
///
/// Derived from `base_url` (not `test_mode`) because HS does not forward
/// `test_mode` in the SetupRecurring gRPC request; relying on `test_mode` would
/// otherwise route sandbox challenges to the production host.
fn datatrans_redirection_host(base_url: &str) -> &'static str {
    if base_url.contains("sandbox") {
        REDIRECTION_SBX_URL
    } else {
        REDIRECTION_PROD_URL
    }
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
                code: DEFAULT_ERROR_CODE.to_string(),
                message: DEFAULT_ERROR_MESSAGE.to_string(),
            },
        }
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
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CardHolder {
    pub cardholder_name: Secret<String>,
    pub email: Email,
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

/// Redirect return URLs supplied to Datatrans for the native 3DS challenge (`/v1/transactions`).
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RedirectUrls {
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
    pub error_url: Option<String>,
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

/// SetupMandate (zero-auth CIT alias creation) reuses the Authorize request and
/// response shapes verbatim. These aliases give the connector-service Bridge macro
/// distinct templating type names per flow (the Bridge is keyed on request/response
/// type identity) without duplicating the underlying structs.
pub type DatatransSetupMandateRequest<T> = DatatransPaymentsRequest<T>;
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
        // Native 3DS = merchant asks Datatrans to run the challenge (auth_type is ThreeDs and no
        // external authentication artifacts were supplied). This is the only case that needs the
        // redirect return URLs; passthrough external 3DS and no-3DS payments do not redirect.
        let is_native_three_ds = router_data.resource_common_data.is_three_ds()
            && router_data.request.authentication_data.is_none();
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
                    three_ds: build_three_ds_data(router_data)?,
                };
                // Return URLs are required for a native-3DS challenge OR a CIT alias
                // registration (which Datatrans runs through the redirect-capable endpoint).
                let redirect = (is_native_three_ds || is_mandate_payment).then(|| RedirectUrls {
                    success_url: router_data.request.router_return_url.clone(),
                    cancel_url: router_data.request.router_return_url.clone(),
                    error_url: router_data.request.router_return_url.clone(),
                });
                (Some(card), redirect, None, None)
            }
            // TODO: CardToken flow for Datatrans Secure Fields SDK.
            // When the client SDK collects card data via Secure Fields, the transactionId
            // from secureFieldsInit is used as an alias. The authorize-split endpoint
            // (POST /v1/transactions/{transactionId}/authorize) should be called instead
            // of the regular authorize endpoint. The PaymentMethodToken carries the
            // transactionId from the client authentication token response.
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let token = token_data.token.clone();

                let card = DatatransCard {
                    alias: Some(token),
                    number: None,
                    expiry_month: None,
                    expiry_year: None,
                    cvv: None,
                    card_type: None,
                    three_ds: None,
                };
                (Some(card), None, None, None)
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
                | WalletData::PaymayaRedirect(_) => Err(IntegrationError::NotImplemented(
                    domain_types::utils::get_unimplemented_payment_method_error_message(
                        "Datatrans",
                    ),
                    datatrans_context("Datatrans Authorize supports Google Pay and Apple Pay only"),
                ))?,
            }
            _ => Err(IntegrationError::NotImplemented(
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

        Ok(Self {
            currency: router_data.request.currency,
            refno: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: Some(router_data.request.minor_amount),
            card,
            auto_settle,
            redirect,
            // CIT mandate registration asks Datatrans to persist a reusable alias
            // (surfaced later via PSync as `connector_mandate_id`); non-mandate Authorize
            // sends `option: null`.
            option: (is_mandate_payment && router_data.request.is_card()).then_some(
                DatatransPaymentOptions {
                    create_alias: Some(true),
                },
            ),
            pay,
            apl,
        })
    }
}

/// Builds the optional `3D` object for a raw-card Authorize request.
/// - external/passthrough 3DS (merchant supplied `authentication_data`) -> `Authentication`
/// - Datatrans-native 3DS (`auth_type == ThreeDs`, no external data) -> `Cardholder`
/// - no 3DS -> `None`
fn build_three_ds_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<Option<ThreeDSecureData>, error_stack::Report<IntegrationError>> {
    if let Some(auth_data) = &router_data.request.authentication_data {
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
    } else if router_data.resource_common_data.is_three_ds() {
        Ok(Some(ThreeDSecureData::Cardholder(ThreedsInfo {
            cardholder: CardHolder {
                cardholder_name: router_data.resource_common_data.get_billing_full_name()?,
                email: router_data.resource_common_data.get_billing_email()?,
            },
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
/// Native 3DS responses need a cardholder challenge -> `AuthenticationPending`;
/// a completed transaction is `Charged` when auto-captured, else `Authorized`.
fn get_authorize_status(
    response: &DatatransPaymentsResponse,
    is_auto_capture: bool,
) -> AttemptStatus {
    match response {
        DatatransPaymentsResponse::ThreeDSResponse(_) => AttemptStatus::AuthenticationPending,
        DatatransPaymentsResponse::TransactionResponse(_) => {
            if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        }
    }
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
        let status = get_authorize_status(&item.response, is_auto_capture);

        let payments_response_data = match &item.response {
            DatatransPaymentsResponse::TransactionResponse(response) => {
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

                // Non-3DS / passthrough external 3DS: no redirect. The alias (for mandates)
                // is surfaced later via PSync, not on this response.
                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: mandate_reference.map(Box::new),
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: response.acquirer_authorization_code.clone(),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                }
            }
            DatatransPaymentsResponse::ThreeDSResponse(response) => {
                // Native 3DS: redirect the cardholder to the Datatrans challenge page.
                // Host is derived from the connector's configured API base_url so the
                // sandbox challenge stays on the sandbox host (see
                // `datatrans_redirection_host`).
                let redirection_host = datatrans_redirection_host(
                    &item
                        .router_data
                        .resource_common_data
                        .connectors
                        .datatrans
                        .base_url,
                );
                let redirection_data = RedirectForm::Form {
                    endpoint: format!("{}/v1/start/{}", redirection_host, response.transaction_id),
                    method: Method::Get,
                    form_fields: HashMap::new(),
                };
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
    > for DatatransPaymentsRequest<T>
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

        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => DatatransCard {
                alias: None,
                number: Some(card_data.card_number.clone()),
                expiry_month: Some(card_data.card_exp_month.clone()),
                expiry_year: Some(card_data.get_card_expiry_year_2_digit()?),
                cvv: Some(card_data.card_cvc.clone()),
                card_type: Some(CARD_TYPE_PLAIN.to_string()),
                // Zero-auth alias creation always runs Datatrans-native 3DS: send the
                // cardholder details so Datatrans can drive the ACS challenge.
                three_ds: Some(ThreeDSecureData::Cardholder(ThreedsInfo {
                    cardholder: CardHolder {
                        cardholder_name: router_data
                            .resource_common_data
                            .get_billing_full_name()?,
                        email: router_data.resource_common_data.get_billing_email()?,
                    },
                })),
            },
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
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::NotImplemented(
                    UNSUPPORTED_PAYMENT_METHOD_ERROR.to_string(),
                    datatrans_context(
                        "Datatrans SetupMandate (zero-auth alias creation) supports raw card payment method only",
                    ),
                ))?
            }
        };

        Ok(Self {
            currency: router_data.request.currency,
            refno: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            // Zero-auth: no amount is charged; the field is omitted from the request.
            amount: None,
            card: Some(card),
            // Zero-auth alias creation cannot be manually captured.
            auto_settle: Some(true),
            redirect: Some(RedirectUrls {
                success_url: router_data.request.router_return_url.clone(),
                cancel_url: router_data.request.router_return_url.clone(),
                error_url: router_data.request.router_return_url.clone(),
            }),
            // Ask Datatrans to persist a reusable alias for later MIT/RepeatPayment.
            option: router_data
                .request
                .is_card()
                .then_some(DatatransPaymentOptions {
                    create_alias: Some(true),
                }),
            pay: None,
            apl: None,
        })
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
        // Zero-auth alias creation cannot be manually captured, so status is derived
        // with auto-capture semantics (`Charged` on a completed transaction).
        let status = get_authorize_status(&item.response, true);

        let payments_response_data = match &item.response {
            DatatransPaymentsResponse::TransactionResponse(response) => {
                // The reusable alias is surfaced later via PSync (`card.alias`),
                // not on this setup response — matching the reference behaviour.
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
                }
            }
            DatatransPaymentsResponse::ThreeDSResponse(response) => {
                // Native 3DS: redirect the cardholder to the Datatrans challenge page.
                // Host is derived from the connector's configured API base_url (not
                // `test_mode`, which HS omits from the SetupRecurring request) so the
                // sandbox challenge stays on the sandbox host.
                let redirection_host = datatrans_redirection_host(
                    &item
                        .router_data
                        .resource_common_data
                        .connectors
                        .datatrans
                        .base_url,
                );
                let redirection_data = RedirectForm::Form {
                    endpoint: format!("{}/v1/start/{}", redirection_host, response.transaction_id),
                    method: Method::Get,
                    form_fields: HashMap::new(),
                };
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
            refno: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: Some(router_data.request.minor_amount),
            card: Some(card),
            // auto_settle mirrors is_auto_capture(): Automatic/SequentialAutomatic/None -> true,
            // Manual/ManualMultiple/Scheduled -> false.
            auto_settle: Some(router_data.request.is_auto_capture()),
            // MIT never redirects and never re-creates an alias.
            redirect: None,
            option: None,
            pay: None,
            apl: None,
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
        let status = get_authorize_status(&item.response, is_auto_capture);

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
                }
            }
            DatatransPaymentsResponse::ThreeDSResponse(response) => {
                // Not expected for MIT (the alias is already 3DS-authenticated), but the untagged
                // response can technically carry it; surface the challenge redirect defensively
                // rather than treating it as a settled transaction.
                let redirection_host = datatrans_redirection_host(
                    &item
                        .router_data
                        .resource_common_data
                        .connectors
                        .datatrans
                        .base_url,
                );
                let redirection_data = RedirectForm::Form {
                    endpoint: format!("{}/v1/start/{}", redirection_host, response.transaction_id),
                    method: Method::Get,
                    form_fields: HashMap::new(),
                };
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
/// Each per-type `status` match is exhaustive (no wildcard) so a new `DatatransPaymentStatus`
/// variant fails to compile rather than silently defaulting.
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
            DatatransPaymentStatus::Initialized | DatatransPaymentStatus::Authenticated => {
                AttemptStatus::Pending
            }
        },
        DatatransTransactionType::CardCheck => match status {
            DatatransPaymentStatus::Settled
            | DatatransPaymentStatus::Transmitted
            | DatatransPaymentStatus::Authorized => AttemptStatus::Charged,
            DatatransPaymentStatus::ChallengeOngoing
            | DatatransPaymentStatus::ChallengeRequired => AttemptStatus::AuthenticationPending,
            DatatransPaymentStatus::Canceled => AttemptStatus::Voided,
            DatatransPaymentStatus::Failed => AttemptStatus::Failure,
            DatatransPaymentStatus::Initialized | DatatransPaymentStatus::Authenticated => {
                AttemptStatus::Pending
            }
        },
        DatatransTransactionType::Credit => AttemptStatus::Failure,
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
                        .unwrap_or_else(|| DEFAULT_ERROR_CODE.to_string()),
                    fail.message
                        .clone()
                        .unwrap_or_else(|| DEFAULT_ERROR_MESSAGE.to_string()),
                ),
                None => (
                    DEFAULT_ERROR_CODE.to_string(),
                    DEFAULT_ERROR_MESSAGE.to_string(),
                ),
            };
            Err(ErrorResponse {
                code,
                message: message.clone(),
                reason: Some(message),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(response.transaction_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            // Extract acquirer authorization code from detail
            let connector_response_reference_id = response.detail.as_ref().and_then(|d| {
                d.authorize
                    .as_ref()
                    .and_then(|a| a.acquirer_authorization_code.clone())
                    .or_else(|| {
                        d.settle
                            .as_ref()
                            .and_then(|s| s.acquirer_authorization_code.clone())
                    })
            });

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
            refno: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            refno2: None,
        })
    }
}

// Capture Response
// Note: API spec says 204 No Content, but Datatrans actually returns 200 with a JSON body
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
        // Datatrans returns 200 with JSON body for successful capture
        // Use transaction_id from response if available, otherwise fall back to request
        let transaction_id = item.response.transaction_id.clone().unwrap_or_else(|| {
            item.router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .unwrap_or_default()
        });

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
            refno: router_data.request.refund_id.clone(),
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
/// states map correctly instead of failing to deserialize.
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
            | DatatransPaymentStatus::ChallengeRequired => RefundStatus::Pending,
            DatatransPaymentStatus::Initialized
            | DatatransPaymentStatus::Authenticated
            | DatatransPaymentStatus::Authorized
            | DatatransPaymentStatus::Canceled
            | DatatransPaymentStatus::Failed => RefundStatus::Failure,
        },
        DatatransTransactionType::Payment | DatatransTransactionType::CardCheck => {
            RefundStatus::Failure
        }
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
