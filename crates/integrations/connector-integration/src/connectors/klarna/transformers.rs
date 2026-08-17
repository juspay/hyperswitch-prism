use common_enums::{CountryAlpha2, Currency};
use common_utils::{pii::SecretSerdeValue, types::MinorUnit, Email};
use domain_types::{
    connector_flow::{Authorize, Capture, ClientAuthenticationToken, PSync, RSync, Refund, Void},
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        KlarnaClientAuthenticationResponse as KlarnaClientAuthenticationResponseDomain,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::Address,
    payment_method_data::{PayLaterData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        KlarnaSdkResponse,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// =============================================================================
// AUTHENTICATION
// =============================================================================
// Klarna uses HTTP Basic auth. The credential is a `BodyKey`:
//   username = key1  (Klarna Key ID)
//   password = api_key (Klarna Secret)
// The header is built as `Basic base64(username:password)` in
// `ConnectorCommon::get_auth_header`.
#[derive(Debug, Clone)]
pub struct KlarnaAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for KlarnaAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Klarna { api_key, key1, .. } => Ok(Self {
                username: key1.to_owned(),
                password: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// =============================================================================
// REGION-TEMPLATED BASE URL METADATA
// =============================================================================
// Klarna's base URL contains a `{{klarna_region}}` placeholder resolved at
// request time from the merchant connector account metadata field
// `metadata.klarna_region` (surfaced in UCS via
// `PaymentFlowData::connector_feature_data`). Absent -> InvalidConnectorConfig.
#[derive(Debug, Serialize, Deserialize)]
pub struct KlarnaConnectorMetadataObject {
    pub klarna_region: Option<KlarnaEndpoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum KlarnaEndpoint {
    Europe,
    NorthAmerica,
    Oceania,
}

impl From<KlarnaEndpoint> for String {
    fn from(endpoint: KlarnaEndpoint) -> Self {
        Self::from(match endpoint {
            KlarnaEndpoint::Europe => "",
            KlarnaEndpoint::NorthAmerica => "-na",
            KlarnaEndpoint::Oceania => "-oc",
        })
    }
}

impl TryFrom<&Option<SecretSerdeValue>> for KlarnaConnectorMetadataObject {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(meta_data: &Option<SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata: Self = crate::utils::to_connector_meta_from_secret::<Self>(meta_data.clone())
            .change_context(errors::IntegrationError::InvalidConnectorConfig {
                config: "merchant_connector_account.metadata",
                context: Default::default(),
            })?;
        Ok(metadata)
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================
// Klarna has (at least) two distinct error shapes:
//   1. API validation error:
//        { "correlation_id": "...", "error_code": "BAD_VALUE",
//          "error_messages": ["Bad value: order_lines"] }
//      Some endpoints return a single `error_message` instead of `error_messages`.
//   2. Auth / gateway error (e.g. 401), which has NO `error_code`:
//        { "http_status_code": 401, "http_status_message": "Unauthorized",
//          "internal_message": "authorization_error" }
// Every field is optional so both shapes deserialize; `build_error_response`
// picks whichever is present. Ref: https://docs.klarna.com/api/errors/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlarnaErrorResponse {
    pub correlation_id: Option<String>,
    pub error_code: Option<String>,
    pub error_messages: Option<Vec<String>>,
    pub error_message: Option<String>,
    pub http_status_code: Option<u16>,
    pub http_status_message: Option<String>,
    pub internal_message: Option<String>,
}

// =============================================================================
// SESSION (CreateSessionToken pre-auth) FLOW  —  POST payments/v1/sessions
// =============================================================================
// SDK / embedded Klarna variant. A pre-authorization session is created and the
// returned `client_token` is handed to the client-side Klarna SDK. Faithful
// port of Hyperswitch's `KlarnaSessionRequest` / `KlarnaSessionResponse`; in
// UCS this rides the `ClientAuthenticationToken` flow.

/// Session intent. Hyperswitch hardcodes `buy`; `tokenize` / `buy_and_tokenize`
/// exist in the API but are unused.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KlarnaSessionIntent {
    Buy,
}

/// A single line item of the order sent to Klarna. Tax fields are omitted for
/// the SDK session branch (Hyperswitch sends them as `null`).
#[derive(Debug, Serialize)]
pub struct OrderLines {
    name: String,
    quantity: u16,
    unit_price: MinorUnit,
    total_amount: MinorUnit,
    total_tax_amount: Option<MinorUnit>,
    tax_rate: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct KlarnaSessionRequest {
    intent: KlarnaSessionIntent,
    purchase_country: CountryAlpha2,
    purchase_currency: Currency,
    order_amount: MinorUnit,
    order_lines: Vec<OrderLines>,
    shipping_address: Option<KlarnaShippingAddress>,
}

/// A payment method category returned by Klarna in the session response. Klarna
/// returns these to drive the SDK widget; Hyperswitch does not consume them, so
/// they are deserialized but not propagated to the domain response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlarnaPaymentMethodCategory {
    pub identifier: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct KlarnaSessionResponse {
    pub client_token: Secret<String>,
    pub session_id: String,
    #[serde(default)]
    pub payment_method_categories: Option<Vec<KlarnaPaymentMethodCategory>>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::KlarnaRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for KlarnaSessionRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::KlarnaRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let order_details =
            request
                .order_details
                .clone()
                .ok_or(errors::IntegrationError::MissingRequiredField {
                    field_name: "order_details",
                    context: Default::default(),
                })?;
        let order_amount = super::KlarnaAmountConvertor::convert(request.amount, request.currency)?;

        Ok(Self {
            intent: KlarnaSessionIntent::Buy,
            purchase_country: request.country.ok_or(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "billing.address.country",
                    context: Default::default(),
                },
            )?,
            purchase_currency: request.currency,
            order_amount,
            order_lines: order_details
                .iter()
                .map(|data| OrderLines {
                    name: data.product_name.clone(),
                    quantity: data.quantity,
                    unit_price: data.amount,
                    total_amount: data.amount * data.quantity,
                    total_tax_amount: None,
                    tax_rate: None,
                })
                .collect(),
            shipping_address: get_address_info(request.shipping_address.as_ref()).transpose()?,
        })
    }
}

impl TryFrom<ResponseRouterData<KlarnaSessionResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<KlarnaSessionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Klarna(
                KlarnaClientAuthenticationResponseDomain {
                    session_id: response.session_id,
                    client_token: response.client_token,
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

// =============================================================================
// AUTHORIZE FLOW  —  POST payments/v1/authorizations/{token}/order  (SDK)
//                    POST checkout/v3/orders                        (Redirect)
// =============================================================================
// Klarna's Authorize has two wire shapes, branched by the payment method data:
//   * `PayLaterData::KlarnaSdk`      -> Klarna Payments API. The full order
//     amount is charged (no separate tax field) and `auto_capture` is derived
//     from the capture method. The response is driven by `fraud_status`.
//   * `PayLaterData::KlarnaRedirect` -> Klarna Checkout v3. The order amount is
//     net of tax (`order_amount = amount - order_tax_amount`), order lines
//     carry tax, and merchant URLs + auto-capture ride under `options`. The
//     response returns an HTML snippet surfaced as `RedirectForm::Html`.
// Faithful port of Hyperswitch's `KlarnaPaymentsRequest` / `KlarnaAuthResponse`.

/// Klarna Checkout v3 merchant redirect URLs. `terms`, `checkout`, and
/// `confirmation` all reuse the router return URL; `push` is the webhook URL.
#[derive(Debug, Serialize)]
pub struct MerchantURLs {
    terms: String,
    checkout: String,
    confirmation: String,
    push: String,
}

/// Checkout-only request fields, flattened onto `KlarnaPaymentsRequest` for the
/// redirect variant.
#[derive(Debug, Serialize)]
pub struct KlarnaCheckoutRequestData {
    merchant_urls: MerchantURLs,
    options: CheckoutOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutOptions {
    auto_capture: bool,
}

/// Variant-specific payload flattened into the shared request body. Untagged so
/// the checkout fields serialize inline (SDK variant contributes nothing).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaymentMethodSpecifics {
    KlarnaCheckout(KlarnaCheckoutRequestData),
    KlarnaSdk,
}

/// Shipping address sent to Klarna. All fields are required by Klarna when a
/// shipping address is present; a partially-filled address is rejected upstream.
#[derive(Debug, Serialize)]
pub struct KlarnaShippingAddress {
    city: Secret<String>,
    country: CountryAlpha2,
    email: Email,
    given_name: Secret<String>,
    family_name: Secret<String>,
    phone: Secret<String>,
    postal_code: Secret<String>,
    region: Secret<String>,
    street_address: Secret<String>,
    street_address2: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct KlarnaPaymentsRequest {
    order_lines: Vec<OrderLines>,
    order_amount: MinorUnit,
    purchase_country: CountryAlpha2,
    purchase_currency: Currency,
    merchant_reference1: Option<String>,
    merchant_reference2: Option<String>,
    shipping_address: Option<KlarnaShippingAddress>,
    auto_capture: Option<bool>,
    order_tax_amount: Option<MinorUnit>,
    #[serde(flatten)]
    payment_method_specifics: Option<PaymentMethodSpecifics>,
}

/// Builds a Klarna shipping address from the optional shipping `Address`.
/// Returns `None` when no shipping address is supplied, and `Some(Err(..))`
/// when one is supplied but a required field is missing.
fn get_address_info(
    address: Option<&Address>,
) -> Option<Result<KlarnaShippingAddress, error_stack::Report<errors::IntegrationError>>> {
    address.and_then(|add| {
        add.address.as_ref().map(|address_details| {
            Ok(KlarnaShippingAddress {
                city: address_details.get_city()?.to_owned(),
                country: address_details.get_country()?.to_owned(),
                email: add.get_email()?,
                postal_code: address_details.get_zip()?.to_owned(),
                region: address_details.to_state_code()?,
                street_address: address_details.get_line1()?.to_owned(),
                street_address2: address_details.get_optional_line2(),
                given_name: address_details.get_first_name()?.to_owned(),
                family_name: address_details.get_last_name()?.to_owned(),
                phone: add.get_phone_with_country_code()?,
            })
        })
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::KlarnaRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for KlarnaPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::KlarnaRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common_data = &router_data.resource_common_data;
        let order_details =
            common_data
                .order_details
                .clone()
                .ok_or(errors::IntegrationError::MissingRequiredField {
                    field_name: "order_details",
                    context: Default::default(),
                })?;
        let order_amount = super::KlarnaAmountConvertor::convert(request.amount, request.currency)?;
        let purchase_country = common_data.get_billing_country()?;
        let merchant_reference1 = Some(common_data.connector_request_reference_id.clone());
        let merchant_reference2 = request.merchant_order_id.clone();
        let shipping_address = get_address_info(common_data.get_optional_shipping()).transpose()?;

        match &request.payment_method_data {
            PaymentMethodData::PayLater(PayLaterData::KlarnaSdk { .. }) => Ok(Self {
                purchase_country,
                purchase_currency: request.currency,
                order_amount,
                order_lines: order_details
                    .iter()
                    .map(|data| OrderLines {
                        name: data.product_name.clone(),
                        quantity: data.quantity,
                        unit_price: data.amount,
                        total_amount: data.amount * data.quantity,
                        total_tax_amount: None,
                        tax_rate: None,
                    })
                    .collect(),
                merchant_reference1,
                merchant_reference2,
                auto_capture: Some(request.is_auto_capture()),
                shipping_address,
                order_tax_amount: None,
                payment_method_specifics: None,
            }),
            PaymentMethodData::PayLater(PayLaterData::KlarnaRedirect {}) => {
                let return_url = request.get_router_return_url()?;
                let webhook_url = request.webhook_url.clone().ok_or(
                    errors::IntegrationError::MissingRequiredField {
                        field_name: "webhook_url",
                        context: Default::default(),
                    },
                )?;
                Ok(Self {
                    purchase_country,
                    purchase_currency: request.currency,
                    order_amount: order_amount
                        - request.order_tax_amount.unwrap_or(MinorUnit::zero()),
                    order_tax_amount: request.order_tax_amount,
                    order_lines: order_details
                        .iter()
                        .map(|data| OrderLines {
                            name: data.product_name.clone(),
                            quantity: data.quantity,
                            unit_price: data.amount,
                            total_amount: data.amount * data.quantity,
                            total_tax_amount: data.total_tax_amount,
                            tax_rate: data.tax_rate,
                        })
                        .collect(),
                    payment_method_specifics: Some(PaymentMethodSpecifics::KlarnaCheckout(
                        KlarnaCheckoutRequestData {
                            merchant_urls: MerchantURLs {
                                terms: return_url.clone(),
                                checkout: return_url.clone(),
                                confirmation: return_url,
                                push: webhook_url,
                            },
                            options: CheckoutOptions {
                                auto_capture: request.is_auto_capture(),
                            },
                        },
                    )),
                    shipping_address,
                    merchant_reference1,
                    merchant_reference2,
                    auto_capture: None,
                })
            }
            _ => Err(errors::IntegrationError::NotImplemented(
                "Payment method".to_string(),
                Default::default(),
            )
            .into()),
        }
    }
}

/// Klarna authorize response. Untagged: the SDK variant carries `fraud_status`
/// while the checkout variant carries `status` + `html_snippet`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum KlarnaAuthResponse {
    KlarnaPaymentsAuthResponse(PaymentsResponse),
    KlarnaCheckoutAuthResponse(CheckoutResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaymentsResponse {
    order_id: String,
    fraud_status: KlarnaFraudStatus,
    /// The Klarna sub-product the customer was authorized for. Surfaced to the
    /// caller via `connector_response.additional_payment_method_data`.
    authorized_payment_method: Option<AuthorizedPaymentMethod>,
}

/// The Klarna payment product chosen by the customer during authorization,
/// e.g. `{ "type": "pay_later" }`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedPaymentMethod {
    #[serde(rename = "type")]
    payment_type: String,
}

impl From<AuthorizedPaymentMethod> for AdditionalPaymentMethodConnectorResponse {
    fn from(item: AuthorizedPaymentMethod) -> Self {
        Self::PayLater {
            klarna_sdk: Some(KlarnaSdkResponse {
                payment_type: Some(item.payment_type),
            }),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CheckoutResponse {
    order_id: String,
    status: KlarnaCheckoutStatus,
    html_snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum KlarnaFraudStatus {
    Accepted,
    Pending,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KlarnaCheckoutStatus {
    CheckoutComplete,
    CheckoutIncomplete,
}

/// Maps Klarna's fraud screening result to an attempt status. `Accepted`
/// resolves to `Charged` for auto-capture, otherwise `Authorized`.
fn get_fraud_status(
    klarna_status: KlarnaFraudStatus,
    is_auto_capture: bool,
) -> common_enums::AttemptStatus {
    match klarna_status {
        KlarnaFraudStatus::Accepted => {
            if is_auto_capture {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        KlarnaFraudStatus::Pending => common_enums::AttemptStatus::Pending,
        KlarnaFraudStatus::Rejected => common_enums::AttemptStatus::Failure,
    }
}

/// Maps Klarna Checkout status to an attempt status. `CheckoutIncomplete`
/// resolves to `AuthenticationPending` for auto-capture (customer must complete
/// the hosted checkout), otherwise `Authorized`.
fn get_checkout_status(
    klarna_status: KlarnaCheckoutStatus,
    is_auto_capture: bool,
) -> common_enums::AttemptStatus {
    match klarna_status {
        KlarnaCheckoutStatus::CheckoutIncomplete => {
            if is_auto_capture {
                common_enums::AttemptStatus::AuthenticationPending
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        KlarnaCheckoutStatus::CheckoutComplete => common_enums::AttemptStatus::Charged,
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<KlarnaAuthResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<KlarnaAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = item.router_data.request.is_auto_capture();
        let http_code = item.http_code;

        let (status, response_data, connector_response) = match item.response {
            KlarnaAuthResponse::KlarnaPaymentsAuthResponse(response) => {
                let status = get_fraud_status(response.fraud_status, is_auto_capture);
                // Klarna reports which of its products the customer was
                // authorized for; pass it through so the caller can surface it.
                let connector_response = response.authorized_payment_method.map(|method| {
                    ConnectorResponseData::with_additional_payment_method_data(
                        AdditionalPaymentMethodConnectorResponse::from(method),
                    )
                });
                let data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.order_id.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(response.order_id),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: http_code,
                };
                (status, data, connector_response)
            }
            KlarnaAuthResponse::KlarnaCheckoutAuthResponse(response) => {
                let status = get_checkout_status(response.status, is_auto_capture);
                let data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.order_id.clone()),
                    redirection_data: Some(Box::new(RedirectForm::Html {
                        html_data: response.html_snippet,
                    })),
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(response.order_id),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: http_code,
                };
                // Klarna Checkout does not report an authorized product.
                (status, data, None)
            }
        };

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;
        router_data.resource_common_data.connector_response = connector_response;
        router_data.response = Ok(response_data);
        Ok(router_data)
    }
}

// =============================================================================
// PSYNC FLOW  —  GET ordermanagement/v1/orders/{order_id}  (SDK)
//                GET checkout/v3/orders/{order_id}          (Redirect)
// =============================================================================
// Two wire shapes, deserialized untagged and branched by `payment_experience`
// upstream in `get_url`:
//   * SDK / Klarna Payments  -> Order Management order, whose `status` is a
//     `KlarnaOrderStatus` (AUTHORIZED / PART_CAPTURED / CAPTURED / CANCELLED /
//     EXPIRED / CLOSED).
//   * Klarna Checkout v3     -> checkout order, whose `status` is a
//     `KlarnaCheckoutStatus` (+ `options.auto_capture`), reusing the Authorize
//     mapping via `get_checkout_status`.
// Faithful port of Hyperswitch's `KlarnaPsyncResponse`.

/// Untagged: the SDK variant carries `KlarnaOrderStatus` + optional
/// `klarna_reference`, the checkout variant carries `KlarnaCheckoutStatus` +
/// `options`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum KlarnaPsyncResponse {
    KlarnaSdkPsyncResponse(KlarnaSdkSyncResponse),
    KlarnaCheckoutPSyncResponse(KlarnaCheckoutSyncResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlarnaSdkSyncResponse {
    pub order_id: String,
    pub status: KlarnaOrderStatus,
    pub klarna_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlarnaCheckoutSyncResponse {
    pub order_id: String,
    pub status: KlarnaCheckoutStatus,
    pub options: CheckoutOptions,
}

/// Klarna Order Management order status (SDK / Klarna Payments PSync).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KlarnaOrderStatus {
    Authorized,
    PartCaptured,
    Captured,
    Cancelled,
    Expired,
    Closed,
}

impl From<KlarnaOrderStatus> for common_enums::AttemptStatus {
    fn from(item: KlarnaOrderStatus) -> Self {
        match item {
            KlarnaOrderStatus::Authorized => Self::Authorized,
            KlarnaOrderStatus::PartCaptured => Self::PartialCharged,
            KlarnaOrderStatus::Captured => Self::Charged,
            KlarnaOrderStatus::Cancelled => Self::Voided,
            KlarnaOrderStatus::Expired | KlarnaOrderStatus::Closed => Self::Failure,
        }
    }
}

impl TryFrom<ResponseRouterData<KlarnaPsyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<KlarnaPsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;

        let (status, response_data) = match item.response {
            KlarnaPsyncResponse::KlarnaSdkPsyncResponse(response) => {
                let status = common_enums::AttemptStatus::from(response.status);
                let connector_response_reference_id =
                    response.klarna_reference.or(Some(response.order_id.clone()));
                let data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.order_id),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id,
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: http_code,
                };
                (status, data)
            }
            KlarnaPsyncResponse::KlarnaCheckoutPSyncResponse(response) => {
                let status = get_checkout_status(response.status, response.options.auto_capture);
                let data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.order_id.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(response.order_id),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: http_code,
                };
                (status, data)
            }
        };

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;
        router_data.response = Ok(response_data);
        Ok(router_data)
    }
}

// =============================================================================
// CAPTURE FLOW  —  POST ordermanagement/v1/orders/{order_id}/captures
// =============================================================================
// Captures an authorized Klarna order. The (minor-unit) `captured_amount` and an
// optional merchant `reference` are sent in the JSON body; `order_id` is the
// connector_transaction_id, carried in the URL path.
//
// CRITICAL: Klarna returns the capture id in the `Capture-Id` *response header*
// (a 201 with an often-empty body), not the body. The header is read in
// `handle_response_v2` (see klarna.rs) and injected into `KlarnaCaptureResponse`
// before this `TryFrom` maps it into `connector_metadata`. Faithful port of
// Hyperswitch's `KlarnaCaptureRequest` / `KlarnaCaptureResponse`.
#[derive(Debug, Serialize)]
pub struct KlarnaCaptureRequest {
    captured_amount: MinorUnit,
    reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::KlarnaRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for KlarnaCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::KlarnaRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let captured_amount = super::KlarnaAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;

        Ok(Self {
            captured_amount,
            reference: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}

/// Klarna order-management capture metadata. The capture id is sourced from the
/// `Capture-Id` response header (see `handle_response_v2`) and surfaced under
/// `connector_metadata`.
#[derive(Debug, Serialize, Deserialize)]
pub struct KlarnaMeta {
    pub capture_id: Option<String>,
}

/// Capture response. Klarna returns a 201 with an often-empty body; the capture
/// id lives in the `Capture-Id` response header and is injected here by
/// `handle_response_v2` before this maps into the domain response.
#[derive(Debug, Serialize, Deserialize)]
pub struct KlarnaCaptureResponse {
    pub capture_id: Option<String>,
}

impl TryFrom<ResponseRouterData<KlarnaCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<KlarnaCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let connector_metadata = Some(serde_json::json!(KlarnaMeta {
            capture_id: item.response.capture_id,
        }));
        // https://docs.klarna.com/api/ordermanagement/#operation/captureOrder
        // A 201 means the order was captured; any other status code is routed
        // through the error handler, so the prior attempt status is retained.
        let status = if http_code == 201 {
            common_enums::AttemptStatus::Charged
        } else {
            item.router_data.resource_common_data.status
        };
        let resource_id = item.router_data.request.connector_transaction_id.clone();

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;
        router_data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id,
            redirection_data: None,
            mandate_reference: None,
            connector_metadata,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            splits: None,
            status_code: http_code,
        });
        Ok(router_data)
    }
}

// =============================================================================
// REFUND (Execute) FLOW  —  POST ordermanagement/v1/orders/{order_id}/refunds
// =============================================================================
// Refunds a captured Klarna order. The (minor-unit) `refunded_amount` and an
// optional merchant `reference` (the refund id) are sent in the JSON body;
// `order_id` is the connector_transaction_id, carried in the URL path.
//
// CRITICAL: Klarna returns the refund id in the `Refund-Id` *response header*
// (a 201 with an often-empty body), not the body. The header is read in
// `handle_response_v2` (see klarna.rs) and injected into `KlarnaRefundResponse`
// before this `TryFrom` maps it into `connector_refund_id`. Faithful port of
// Hyperswitch's `KlarnaRefundRequest` / `KlarnaRefundResponse`.
#[derive(Debug, Serialize)]
pub struct KlarnaRefundRequest {
    refunded_amount: MinorUnit,
    reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::KlarnaRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for KlarnaRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::KlarnaRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let refunded_amount =
            super::KlarnaAmountConvertor::convert(request.minor_refund_amount, request.currency)?;

        Ok(Self {
            refunded_amount,
            reference: Some(request.refund_id.clone()),
        })
    }
}

/// Refund response. Klarna returns a 201 with an often-empty body; the refund id
/// lives in the `Refund-Id` response header and is injected here by
/// `handle_response_v2` before this maps into the domain response.
#[derive(Debug, Serialize, Deserialize)]
pub struct KlarnaRefundResponse {
    pub refund_id: String,
}

impl TryFrom<ResponseRouterData<KlarnaRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<KlarnaRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        // https://docs.klarna.com/api/ordermanagement/#operation/refundOrder
        // A 201 means the refund was accepted; RSync confirms success later. Any
        // other status code is routed through the error handler.
        let refund_status = if http_code == 201 {
            common_enums::RefundStatus::Pending
        } else {
            common_enums::RefundStatus::Failure
        };

        let mut router_data = item.router_data;
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id: item.response.refund_id,
            refund_status,
            status_code: http_code,
            acquirer_reference_number: None,
        });
        Ok(router_data)
    }
}

// =============================================================================
// RSYNC FLOW  —  GET ordermanagement/v1/orders/{order_id}/refunds/{refund_id}
// =============================================================================
// Syncs the status of a previously-issued Klarna refund. GET, no body; the
// `order_id` (connector_transaction_id) and `refund_id` (connector_refund_id)
// are carried in the URL path. Klarna returns the refund object with its id.
// Faithful port of Hyperswitch's `KlarnaRefundSyncResponse`.
#[derive(Debug, Deserialize, Serialize)]
pub struct KlarnaRSyncResponse {
    pub refund_id: String,
}

impl TryFrom<ResponseRouterData<KlarnaRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<KlarnaRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        // https://docs.klarna.com/api/ordermanagement/#operation/get
        // A 200 means the refund exists and is successful; any other status code
        // is routed through the error handler.
        let refund_status = if http_code == 200 {
            common_enums::RefundStatus::Success
        } else {
            common_enums::RefundStatus::Failure
        };

        let mut router_data = item.router_data;
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id: item.response.refund_id,
            refund_status,
            status_code: http_code,
            acquirer_reference_number: None,
        });
        Ok(router_data)
    }
}

// =============================================================================
// VOID (Cancel) FLOW  —  POST ordermanagement/v1/orders/{order_id}/cancel
// =============================================================================
// Cancels an authorized Klarna order. Empty POST (no request body); Klarna
// returns a 204 No Content with an empty body on success, so there is nothing
// to deserialize. The attempt status is therefore derived purely from the HTTP
// status code (204 -> Voided, anything else -> VoidFailed) in the `TryFrom`
// below — a faithful port of Hyperswitch's Void `handle_response`, which skips
// response parsing entirely. `order_id` is the connector_transaction_id,
// carried in the URL path.
//
// Because the response has no body, `KlarnaVoidResponse` is a placeholder marker
// synthesized by `handle_response_v2` (see klarna.rs); it carries no wire data,
// and the status is resolved solely from `http_code` here.
#[derive(Debug, Serialize, Deserialize)]
pub struct KlarnaVoidResponse;

impl TryFrom<ResponseRouterData<KlarnaVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<KlarnaVoidResponse, Self>) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        // https://docs.klarna.com/api/ordermanagement/#operation/cancelOrder
        // A 204 (No Content) means the order was cancelled; any other status
        // code marks the void as failed.
        let status = if http_code == 204 {
            common_enums::AttemptStatus::Voided
        } else {
            common_enums::AttemptStatus::VoidFailed
        };
        let resource_id = ResponseId::ConnectorTransactionId(
            item.router_data.request.connector_transaction_id.clone(),
        );

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;
        router_data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id,
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            splits: None,
            status_code: http_code,
        });
        Ok(router_data)
    }
}
