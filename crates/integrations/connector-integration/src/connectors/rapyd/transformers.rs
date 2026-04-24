use common_utils::{ext_traits::OptionExt, request::Method, FloatMajorUnit, StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, RepeatPayment, SetupMandate,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference, MandateReferenceId,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData,
        RapydClientAuthenticationResponse as RapydClientAuthenticationResponseDomain,
        RefundFlowData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::CardIssuer,
};
use error_stack;
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use url::Url;

use crate::types::ResponseRouterData;

use super::RapydRouterData;

/// Rapyd `payment_method.type` identifier (`<country>_<network>_card`).
///
/// The `in_` prefix (India) is kept as a placeholder to match the existing
/// Authorize flow until multi-country payment-method resolution lands (see
/// the `[#369]` TODO elsewhere in this file).
///
/// Built via `TryFrom<CardIssuer>` so the wrong identifier can never be
/// assembled ad-hoc — Rapyd rejects requests whose `type` does not match
/// the card BIN.
/// Reference: https://docs.rapyd.net/en/list-payment-methods-by-country.html
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydCardType {
    #[serde(rename = "in_visa_card")]
    InVisaCard,
    #[serde(rename = "in_mastercard_card")]
    InMastercardCard,
    #[serde(rename = "in_amex_card")]
    InAmexCard,
    #[serde(rename = "in_discover_card")]
    InDiscoverCard,
    #[serde(rename = "in_dinersclub_card")]
    InDinersclubCard,
    #[serde(rename = "in_jcb_card")]
    InJcbCard,
    #[serde(rename = "in_maestro_card")]
    InMaestroCard,
}

impl RapydCardType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InVisaCard => "in_visa_card",
            Self::InMastercardCard => "in_mastercard_card",
            Self::InAmexCard => "in_amex_card",
            Self::InDiscoverCard => "in_discover_card",
            Self::InDinersclubCard => "in_dinersclub_card",
            Self::InJcbCard => "in_jcb_card",
            Self::InMaestroCard => "in_maestro_card",
        }
    }
}

impl TryFrom<CardIssuer> for RapydCardType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(issuer: CardIssuer) -> Result<Self, Self::Error> {
        match issuer {
            CardIssuer::Visa => Ok(Self::InVisaCard),
            CardIssuer::Master => Ok(Self::InMastercardCard),
            CardIssuer::AmericanExpress => Ok(Self::InAmexCard),
            CardIssuer::Discover => Ok(Self::InDiscoverCard),
            CardIssuer::DinersClub => Ok(Self::InDinersclubCard),
            CardIssuer::JCB => Ok(Self::InJcbCard),
            CardIssuer::Maestro => Ok(Self::InMaestroCard),
            CardIssuer::CarteBlanche | CardIssuer::CartesBancaires | CardIssuer::UnionPay => {
                Err(IntegrationError::NotImplemented(
                    format!("rapyd card type for {issuer}"),
                    Default::default(),
                ))?
            }
        }
    }
}

/// Rapyd `initiation_type` for `/v1/payments`. MIT replays go out as
/// `recurring`; the full Rapyd vocabulary also includes `customer_present`,
/// `installment`, `moto`, and `unscheduled`, but only `recurring` is used
/// on this path today.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RapydInitiationType {
    Recurring,
}

impl<F, T> TryFrom<ResponseRouterData<RapydPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status =
                    get_status(data.status.to_owned(), data.next_action.to_owned());
                match attempt_status {
                    common_enums::AttemptStatus::Failure => (
                        common_enums::AttemptStatus::Failure,
                        Err(ErrorResponse {
                            code: data
                                .failure_code
                                .to_owned()
                                .unwrap_or(item.response.status.error_code),
                            status_code: item.http_code,
                            message: item.response.status.status.unwrap_or_default(),
                            reason: data.failure_message.to_owned(),
                            attempt_status: None,
                            connector_transaction_id: None,
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                        }),
                    ),
                    _ => {
                        let redirection_url = data
                            .redirect_url
                            .as_ref()
                            .filter(|redirect_str| !redirect_str.is_empty())
                            .map(|url| {
                                Url::parse(url).change_context(
                                    crate::utils::response_handling_fail_for_connector(
                                        item.http_code,
                                        "rapyd",
                                    ),
                                )
                            })
                            .transpose()?;

                        let redirection_data =
                            redirection_url.map(|url| RedirectForm::from((url, Method::Get)));

                        (
                            attempt_status,
                            Ok(PaymentsResponseData::TransactionResponse {
                                resource_id: ResponseId::ConnectorTransactionId(data.id.to_owned()), //transaction_id is also the field but this id is used to initiate a refund
                                redirection_data: redirection_data.map(Box::new),
                                mandate_reference: None,
                                connector_metadata: None,
                                network_txn_id: None,
                                connector_response_reference_id: data
                                    .merchant_reference_id
                                    .to_owned(),
                                incremental_authorization_allowed: None,
                                status_code: item.http_code,
                            }),
                        )
                    }
                }
            }
            None => (
                common_enums::AttemptStatus::Failure,
                Err(ErrorResponse {
                    code: item.response.status.error_code,
                    status_code: item.http_code,
                    message: item.response.status.status.unwrap_or_default(),
                    reason: item.response.status.message,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
            ),
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
pub struct EmptyRequest;

// RapydRouterData is now generated by the macro in rapyd.rs

#[derive(Debug, Serialize)]
pub struct RapydAuthType {
    pub(super) access_key: Secret<String>,
    pub(super) secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for RapydAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Rapyd {
                access_key,
                secret_key,
                ..
            } => Ok(Self {
                access_key: access_key.to_owned(),
                secret_key: secret_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?,
        }
    }
}

#[derive(Default, Debug, Serialize)]
pub struct RapydPaymentsRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub payment_method: RapydPaymentMethodData<T>,
    pub payment_method_options: Option<PaymentMethodOptions>,
    pub merchant_reference_id: Option<String>,
    pub capture: Option<bool>,
    pub description: Option<String>,
    pub complete_payment_url: Option<String>,
    pub error_payment_url: Option<String>,
    /// Rapyd customer — may be either a string id (`cus_*`, for MIT)
    /// or an inline object `{ name, email }` (for SetupMandate, so that
    /// Rapyd creates the customer alongside the payment and issues a
    /// customer-scoped `card_*` token in the response).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<RapydCustomerRef>,
    /// When true and `payment_method` carries card fields, Rapyd saves
    /// the card under the customer and returns a reusable `card_*` id.
    /// Must be paired with `customer`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_payment_method: Option<bool>,
    /// Required on MIT replays so Rapyd bypasses 3DS using the stored
    /// credential. Rapyd's vocabulary also includes `customer_present`,
    /// `installment`, `moto`, and `unscheduled` — only `recurring` is
    /// emitted on the current MIT path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiation_type: Option<RapydInitiationType>,
}

/// Rapyd customer reference: either a raw id string (`cus_*`) for MIT
/// replay, or an inline `{name, email}` object when we want Rapyd to
/// create the customer alongside the payment.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydCustomerRef {
    Id(String),
    Inline(RapydInlineCustomer),
}

#[derive(Debug, Serialize, Default)]
pub struct RapydInlineCustomer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Rapyd payment_method field can be either a token string (for saved/tokenized
/// payment methods) or a full payment method object (for new card / wallet).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydPaymentMethodData<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    Token(Secret<String>),
    PaymentMethod(Box<PaymentMethod<T>>),
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize + Default> Default
    for RapydPaymentMethodData<T>
{
    fn default() -> Self {
        Self::PaymentMethod(Box::default())
    }
}

#[derive(Default, Debug, Serialize)]
pub struct PaymentMethodOptions {
    #[serde(rename = "3d_required")]
    pub three_ds: bool,
}

#[derive(Default, Debug, Serialize)]
pub struct PaymentMethod<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "type")]
    pub pm_type: String,
    pub fields: Option<PaymentFields<T>>,
    pub address: Option<Address>,
    pub digital_wallet: Option<RapydWallet>,
}

#[derive(Default, Debug, Serialize)]
pub struct PaymentFields<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    pub number: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub name: Secret<String>,
    pub cvv: Secret<String>,
}

#[derive(Default, Debug, Serialize)]
pub struct Address {
    name: Secret<String>,
    line_1: Secret<String>,
    line_2: Option<Secret<String>>,
    line_3: Option<Secret<String>>,
    city: Option<String>,
    state: Option<Secret<String>>,
    country: Option<String>,
    zip: Option<Secret<String>>,
    phone_number: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydWallet {
    #[serde(rename = "type")]
    payment_type: String,
    #[serde(rename = "details")]
    token: Option<Secret<String>>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let return_url = item.router_data.request.get_router_return_url()?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let (capture, payment_method_options) =
            match item.router_data.resource_common_data.payment_method {
                common_enums::PaymentMethod::Card => {
                    let three_ds_enabled = matches!(
                        item.router_data.resource_common_data.auth_type,
                        common_enums::AuthenticationType::ThreeDs
                    );
                    let payment_method_options = PaymentMethodOptions {
                        three_ds: three_ds_enabled,
                    };
                    (
                        Some(matches!(
                            item.router_data.request.capture_method,
                            Some(common_enums::CaptureMethod::Automatic)
                                | Some(common_enums::CaptureMethod::SequentialAutomatic)
                                | None
                        )),
                        Some(payment_method_options),
                    )
                }
                _ => (None, None),
            };
        let payment_method = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                Some(RapydPaymentMethodData::PaymentMethod(Box::new(
                    PaymentMethod {
                        pm_type: "in_amex_card".to_owned(), //[#369] Map payment method type based on country
                        fields: Some(PaymentFields {
                            number: ccard.card_number.to_owned(),
                            expiration_month: ccard.card_exp_month.to_owned(),
                            expiration_year: ccard.card_exp_year.to_owned(),
                            name: item
                                .router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                                .to_owned()
                                .unwrap_or(Secret::new("".to_string())),
                            cvv: ccard.card_cvc.to_owned(),
                        }),
                        address: None,
                        digital_wallet: None,
                    },
                )))
            }
            PaymentMethodData::Wallet(ref wallet_data) => {
                let digital_wallet = match wallet_data {
                    WalletData::GooglePay(data) => Some(RapydWallet {
                        payment_type: "google_pay".to_string(),
                        token: Some(Secret::new(
                            data.tokenization_data
                                .get_encrypted_google_pay_token()
                                .change_context(IntegrationError::MissingRequiredField {
                                    field_name: "gpay wallet_token",
                                    context: Default::default(),
                                })?
                                .to_owned(),
                        )),
                    }),
                    WalletData::ApplePay(data) => {
                        let apple_pay_encrypted_data = data
                            .payment_data
                            .get_encrypted_apple_pay_payment_data_mandatory()
                            .change_context(IntegrationError::MissingRequiredField {
                                field_name: "Apple pay encrypted data",
                                context: Default::default(),
                            })?;
                        Some(RapydWallet {
                            payment_type: "apple_pay".to_string(),
                            token: Some(Secret::new(apple_pay_encrypted_data.to_string())),
                        })
                    }
                    _ => None,
                };
                Some(RapydPaymentMethodData::PaymentMethod(Box::new(
                    PaymentMethod {
                        pm_type: "by_visa_card".to_string(), //[#369]
                        fields: None,
                        address: None,
                        digital_wallet,
                    },
                )))
            }
            PaymentMethodData::PaymentMethodToken(token_data) => {
                Some(RapydPaymentMethodData::Token(token_data.token.clone()))
            }
            _ => None,
        }
        .get_required_value("payment_method not implemented")
        .change_context(IntegrationError::NotImplemented(
            "payment_method".to_owned(),
            Default::default(),
        ))?;
        Ok(Self {
            amount,
            currency: item.router_data.request.currency,
            payment_method,
            capture,
            payment_method_options,
            merchant_reference_id: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: None,
            error_payment_url: Some(return_url.clone()),
            complete_payment_url: Some(return_url),
            customer: None,
            save_payment_method: None,
            initiation_type: None,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum RapydPaymentStatus {
    #[serde(rename = "ACT")]
    Active,
    #[serde(rename = "CAN")]
    CanceledByClientOrBank,
    #[serde(rename = "CLO")]
    Closed,
    #[serde(rename = "ERR")]
    Error,
    #[serde(rename = "EXP")]
    Expired,
    #[serde(rename = "REV")]
    ReversedByRapyd,
    #[default]
    #[serde(rename = "NEW")]
    New,
}

fn get_status(status: RapydPaymentStatus, next_action: NextAction) -> common_enums::AttemptStatus {
    match (status, next_action) {
        (RapydPaymentStatus::Closed, _) => common_enums::AttemptStatus::Charged,
        (
            RapydPaymentStatus::Active,
            NextAction::ThreedsVerification | NextAction::PendingConfirmation,
        ) => common_enums::AttemptStatus::AuthenticationPending,
        (RapydPaymentStatus::Active, NextAction::PendingCapture | NextAction::NotApplicable) => {
            common_enums::AttemptStatus::Authorized
        }
        (
            RapydPaymentStatus::CanceledByClientOrBank
            | RapydPaymentStatus::Expired
            | RapydPaymentStatus::ReversedByRapyd,
            _,
        ) => common_enums::AttemptStatus::Voided,
        (RapydPaymentStatus::Error, _) => common_enums::AttemptStatus::Failure,
        (RapydPaymentStatus::New, _) => common_enums::AttemptStatus::Authorizing,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RapydPaymentsResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Status {
    pub error_code: String,
    pub status: Option<String>,
    pub message: Option<String>,
    pub response_code: Option<String>,
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NextAction {
    #[serde(rename = "3d_verification")]
    ThreedsVerification,
    #[serde(rename = "pending_capture")]
    PendingCapture,
    #[serde(rename = "not_applicable")]
    NotApplicable,
    #[serde(rename = "pending_confirmation")]
    PendingConfirmation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseData {
    pub id: String,
    pub amount: FloatMajorUnit,
    pub status: RapydPaymentStatus,
    pub next_action: NextAction,
    pub redirect_url: Option<String>,
    pub original_amount: Option<FloatMajorUnit>,
    pub is_partial: Option<bool>,
    pub currency_code: Option<common_enums::Currency>,
    pub country_code: Option<String>,
    pub captured: Option<bool>,
    pub transaction_id: String,
    pub merchant_reference_id: Option<String>,
    pub paid: Option<bool>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    /// Customer id (`cus_*`) — Rapyd returns the customer id under
    /// the `customer_token` key (NOT `customer`). Present both when the
    /// payment was created with an inline customer object and when it
    /// was created against an existing `cus_*`.
    pub customer_token: Option<String>,
    /// Saved-card token (`card_*`) — populated when the payment was
    /// created with `save_payment_method: true`. Used as the MIT token
    /// on subsequent charges.
    pub payment_method: Option<String>,
}

// Capture Request
#[derive(Debug, Serialize, Clone)]
pub struct CaptureRequest {
    amount: Option<StringMajorUnit>,
    receipt_email: Option<Secret<String>>,
    statement_descriptor: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RapydRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            amount: Some(amount),
            receipt_email: None,
            statement_descriptor: None,
        })
    }
}

// Refund Request
#[derive(Default, Debug, Serialize)]
pub struct RapydRefundRequest {
    pub payment: String,
    pub amount: Option<StringMajorUnit>,
    pub currency: Option<common_enums::Currency>,
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<RapydRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for RapydRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RapydRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            payment: item
                .router_data
                .request
                .connector_transaction_id
                .to_string(),
            amount: Some(amount),
            currency: Some(item.router_data.request.currency),
        })
    }
}

// Refund Response
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum RefundStatus {
    Completed,
    Error,
    Rejected,
    #[default]
    Pending,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Completed => Self::Success,
            RefundStatus::Error | RefundStatus::Rejected => Self::Failure,
            RefundStatus::Pending => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub status: Status,
    pub data: Option<RefundResponseData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefundResponseData {
    pub id: String,
    pub payment: String,
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub status: RefundStatus,
    pub created_at: Option<i64>,
    pub failure_reason: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, T, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let (connector_refund_id, refund_status) = match item.response.data {
            Some(data) => (data.id, common_enums::RefundStatus::from(data.status)),
            None => (
                item.response.status.error_code,
                common_enums::RefundStatus::Failure,
            ),
        };
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Rapyd checkout page/session. The checkout id and redirect_url
/// are returned to the frontend for client-side payment completion.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct RapydClientAuthRequest {
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub country: Option<String>,
    pub merchant_reference_id: Option<String>,
    pub complete_checkout_url: Option<String>,
    pub cancel_checkout_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Verify that the checkout amount and currency are valid.".to_owned(),
                    ),
                    doc_url: Some("https://docs.rapyd.net/en/create-checkout-page.html".to_owned()),
                    additional_context: Some(
                        "Rapyd checkout requires the amount in major-unit string format."
                            .to_owned(),
                    ),
                },
            })?;

        let country = router_data.request.country.map(|c| c.to_string());
        let return_url = router_data.resource_common_data.return_url.clone();

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            country,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            complete_checkout_url: return_url.clone(),
            cancel_checkout_url: return_url,
        })
    }
}

/// Rapyd checkout response containing checkout id and redirect_url for SDK initialization.
#[derive(Debug, Deserialize, Serialize)]
pub struct RapydClientAuthResponse {
    pub status: Status,
    pub data: Option<RapydCheckoutResponseData>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RapydCheckoutResponseData {
    pub id: String,
    pub redirect_url: String,
}

impl TryFrom<ResponseRouterData<RapydClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RapydClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let data = response.data.ok_or(
            ConnectorError::response_deserialization_failed_with_context(
                item.http_code,
                Some(
                    "Rapyd checkout response is missing the 'data' field containing \
                     checkout_id and redirect_url."
                        .to_owned(),
                ),
            ),
        )?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Rapyd(
                RapydClientAuthenticationResponseDomain {
                    checkout_id: data.id,
                    redirect_url: data.redirect_url,
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

// ============================================================================
// CreateOrder Flow - Request/Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct RapydCreateOrderRequest {
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_reference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complete_payment_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_payment_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydCreateOrderResponse {
    pub status: Status,
    pub data: Option<RapydCheckoutData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydCheckoutData {
    pub id: String,
    pub status: String,
    pub redirect_url: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
    pub merchant_reference_id: Option<String>,
    pub page_expiration: Option<i64>,
    pub timestamp: Option<i64>,
}

/// Metadata for CreateOrder flow, passed via connector_feature_data
#[derive(Debug, Clone, Deserialize)]
pub struct RapydCreateOrderMetadata {
    /// Country code for the checkout page (ISO 3166-1 alpha-2)
    pub country: Option<String>,
}

// ============================================================================
// CreateOrder Flow - Request Transformation
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for RapydCreateOrderRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // Try to get country from billing address first, then fallback to connector_feature_data
        let country = router_data
            .resource_common_data
            .get_optional_billing_country()
            .map(|c| c.to_string())
            .or_else(|| {
                // Fallback: try to get country from connector_feature_data
                router_data
                    .resource_common_data
                    .connector_feature_data
                    .as_ref()
                    .and_then(|meta| {
                        serde_json::from_value::<RapydCreateOrderMetadata>(meta.clone().expose())
                            .ok()
                    })
                    .and_then(|m| m.country)
            })
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "billing_country or connector_feature_data.country",
                    context: Default::default(),
                })
            })?;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            country,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            complete_payment_url: router_data.resource_common_data.return_url.clone(),
            error_payment_url: router_data.resource_common_data.return_url.clone(),
            language: Some("en".to_string()),
        })
    }
}

// ============================================================================
// CreateOrder Flow - Response Transformation
// ============================================================================

impl TryFrom<ResponseRouterData<RapydCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RapydCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        match response.data {
            Some(data) => {
                let status = match data.status.as_str() {
                    "NEW" | "INP" => common_enums::AttemptStatus::Pending,
                    "DON" => common_enums::AttemptStatus::Charged,
                    "EXP" | "DEC" => common_enums::AttemptStatus::Failure,
                    _ => common_enums::AttemptStatus::Pending,
                };

                // Extract checkout_id for use in resource_common_data
                let checkout_id = data.id.clone();

                Ok(Self {
                    response: Ok(PaymentCreateOrderResponse {
                        connector_order_id: checkout_id.clone(),
                        session_data: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status,
                        reference_id: Some(checkout_id.clone()),
                        // Store order ID so Authorize flow can use it via connector_order_id
                        connector_order_id: Some(checkout_id),
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            None => Ok(Self {
                response: Err(ErrorResponse {
                    code: response.status.error_code,
                    status_code: item.http_code,
                    message: response.status.status.unwrap_or_default(),
                    reason: response.status.message,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
        }
    }
}

// ============================================================================
// SetupMandate (zero/low-amount COF verification) — Rapyd
// ============================================================================
// Rapyd has no dedicated mandate endpoint. To capture a reusable card token
// we call POST /v1/payments with `save_payment_method: true` plus an inline
// `customer: { name, email }` object so Rapyd creates `cus_*` in the same
// call and attaches a reusable `card_*` to it.
//
// Using `/v1/payments` (rather than `/v1/customers`) avoids the
// `complete_payment_url` whitelist check that the customer-create
// endpoint enforces on sandbox accounts.

/// SetupMandate request – reuses the `/v1/payments` shape but asks Rapyd
/// to save the card under a newly-created customer.
pub type RapydSetupMandateRequest<T> = RapydPaymentsRequest<T>;

/// SetupMandate response – structurally identical to `RapydPaymentsResponse`
/// but defined as a distinct newtype so the SetupMandate `TryFrom` does
/// not collide with the blanket Authorize-style conversion (E0119).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydSetupMandateResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RapydRouterData<
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
        let request = &router_data.request;

        // Rapyd rejects mandate-setup calls with no amount and silently
        // defaulting here would charge an arbitrary value in the caller's
        // currency (e.g. ¥100 vs $1.00). Require the caller to pass an
        // explicit verification amount — zero-amount is allowed if the
        // Rapyd account supports zero-auth.
        let minor_amount = request
            .minor_amount
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "minor_amount",
                context: Default::default(),
            })?;
        let amount = item
            .connector
            .amount_converter
            .convert(minor_amount, request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let payment_method = match &request.payment_method_data {
            PaymentMethodData::Card(ccard) => {
                let card_issuer = domain_types::utils::get_card_issuer(ccard.card_number.peek())?;
                let pm_type = RapydCardType::try_from(card_issuer)?.as_str().to_owned();
                // Rapyd documents `payment_method.fields.name` as required
                // (https://docs.rapyd.net/en/create-card-payment-method.html).
                // Prefer the cardholder name on the card itself; fall back to
                // the billing full name. We deliberately do not fall back to
                // `customer_name`, which describes the Rapyd customer object
                // (inline `{name, email}`) — not the cardholder.
                let cardholder_name = ccard
                    .card_holder_name
                    .clone()
                    .or_else(|| {
                        router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                    })
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "card.card_holder_name / billing.full_name",
                        context: Default::default(),
                    })?;
                RapydPaymentMethodData::PaymentMethod(Box::new(PaymentMethod {
                    pm_type,
                    fields: Some(PaymentFields {
                        number: ccard.card_number.to_owned(),
                        expiration_month: ccard.card_exp_month.to_owned(),
                        expiration_year: ccard.card_exp_year.to_owned(),
                        name: cardholder_name,
                        cvv: ccard.card_cvc.to_owned(),
                    }),
                    address: None,
                    digital_wallet: None,
                }))
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "payment_method for rapyd SetupMandate".to_owned(),
                    Default::default(),
                ))?;
            }
        };

        let three_ds_enabled = matches!(
            router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        let payment_method_options = Some(PaymentMethodOptions {
            three_ds: three_ds_enabled,
        });

        // Rapyd REQUIRES a customer to save a payment method. We pass an
        // inline `{name, email}` object so Rapyd creates `cus_*` in the
        // same call and attaches the saved `card_*` to it. Both fields
        // must come from the customer payload — billing address describes
        // the cardholder, not the customer-of-record, and mixing them
        // would attach the card to the wrong customer on repeat use.
        let customer_name =
            request
                .customer_name
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "customer.name",
                    context: Default::default(),
                })?;
        let customer_email = request.email.as_ref().map(|e| e.peek().to_string()).ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "customer.email",
                context: Default::default(),
            },
        )?;
        let inline_customer = RapydInlineCustomer {
            name: Some(customer_name),
            email: Some(customer_email),
        };

        let return_url = router_data.resource_common_data.return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            },
        )?;

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method,
            // Zero-auth: authorize the card so Rapyd can mint the
            // `card_*` / `cus_*` tokens, but do not capture funds.
            capture: Some(false),
            payment_method_options,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: router_data.resource_common_data.description.clone(),
            complete_payment_url: Some(return_url.clone()),
            error_payment_url: Some(return_url),
            customer: Some(RapydCustomerRef::Inline(inline_customer)),
            save_payment_method: Some(true),
            initiation_type: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RapydSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RapydSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Emitted only on the success path; threaded back through PaymentFlowData
        // so the gRPC layer exposes it via `state.connector_customer_id` and the
        // next RepeatPayment can forward it as `connector_customer_id`.
        let mut connector_customer: Option<String> = None;

        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status =
                    get_status(data.status.to_owned(), data.next_action.to_owned());
                match attempt_status {
                    common_enums::AttemptStatus::Failure => (
                        common_enums::AttemptStatus::Failure,
                        Err(ErrorResponse {
                            code: data
                                .failure_code
                                .to_owned()
                                .unwrap_or(item.response.status.error_code.clone()),
                            status_code: item.http_code,
                            message: item.response.status.status.clone().unwrap_or_else(|| {
                                common_utils::consts::NO_ERROR_MESSAGE.to_string()
                            }),
                            reason: data.failure_message.clone(),
                            attempt_status: None,
                            connector_transaction_id: Some(data.id.clone()),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                        }),
                    ),
                    _ => {
                        // `customer_token` is the Rapyd connector-customer id; it
                        // belongs in `PaymentFlowData.connector_customer` (surfaced
                        // to the caller via `state.connector_customer_id`), not
                        // `mandate_reference`. The card token (`card_*`) is the
                        // actual mandate reference used on MIT replays.
                        match (data.customer_token.as_deref(), data.payment_method.as_deref()) {
                            (Some(cus), Some(card))
                                if cus.starts_with("cus_") && card.starts_with("card_") =>
                            {
                                connector_customer = Some(cus.to_owned());
                                let mandate_reference = Some(Box::new(MandateReference {
                                    connector_mandate_id: Some(card.to_owned()),
                                    payment_method_id: None,
                                    connector_mandate_request_reference_id: None,
                                }));
                                // Promote Authorized → Charged so zero/low-amount
                                // verification attempts reach a terminal state.
                                let terminal_status = match attempt_status {
                                    common_enums::AttemptStatus::Authorized => {
                                        common_enums::AttemptStatus::Charged
                                    }
                                    other => other,
                                };
                                (
                                    terminal_status,
                                    Ok(PaymentsResponseData::TransactionResponse {
                                        resource_id: ResponseId::ConnectorTransactionId(
                                            data.id.clone(),
                                        ),
                                        redirection_data: None,
                                        mandate_reference,
                                        connector_metadata: None,
                                        network_txn_id: None,
                                        connector_response_reference_id: data
                                            .merchant_reference_id
                                            .clone(),
                                        incremental_authorization_allowed: None,
                                        status_code: item.http_code,
                                    }),
                                )
                            }
                            _ => (
                                common_enums::AttemptStatus::Failure,
                                Err(ErrorResponse {
                                    code: item.response.status.error_code.clone(),
                                    status_code: item.http_code,
                                    message: item
                                        .response
                                        .status
                                        .status
                                        .clone()
                                        .unwrap_or_else(|| {
                                            common_utils::consts::NO_ERROR_MESSAGE.to_string()
                                        }),
                                    reason: Some(format!(
                                        "rapyd payment succeeded but did not return a reusable card_* token (customer_token={:?}, payment_method={:?})",
                                        data.customer_token, data.payment_method
                                    )),
                                    attempt_status: None,
                                    connector_transaction_id: Some(data.id.clone()),
                                    network_advice_code: None,
                                    network_decline_code: None,
                                    network_error_message: None,
                                }),
                            ),
                        }
                    }
                }
            }
            None => (
                common_enums::AttemptStatus::Failure,
                Err(ErrorResponse {
                    code: item.response.status.error_code.clone(),
                    status_code: item.http_code,
                    message: item
                        .response
                        .status
                        .status
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: item.response.status.message.clone(),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
            ),
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_customer: connector_customer
                    .or(item.router_data.resource_common_data.connector_customer),
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// ---------------------------------------------------------------------------
// RepeatPayment (MIT) — Rapyd reuses /v1/payments with a stored
// `payment_method` token (the payment id returned by SetupMandate). The
// request body is structurally identical to `RapydPaymentsRequest`, but we
// use a distinct response newtype so the TryFrom impls don't collide with
// the blanket Authorize conversion.
// ---------------------------------------------------------------------------

pub type RapydRepeatPaymentRequest<T> = RapydPaymentsRequest<T>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydRepeatPaymentResponse {
    pub status: Status,
    pub data: Option<ResponseData>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydRepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RapydRouterData<
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

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // SetupMandate stored the `card_*` token in `connector_mandate_id`.
        // The `cus_*` it returned was routed to `PaymentFlowData.connector_customer`
        // and arrives back on this Charge via `RecurringPaymentServiceChargeRequest
        // .connector_customer_id` (proto field 14).
        let card_id = match &request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(cmr) => {
                cmr.get_connector_mandate_id()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "mandate_reference.connector_mandate_id",
                        context: Default::default(),
                    })?
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "non-connector mandate for rapyd RepeatPayment".to_owned(),
                    Default::default(),
                ))?;
            }
        };
        let customer_id = router_data
            .resource_common_data
            .connector_customer
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_customer_id",
                context: Default::default(),
            })?;

        let three_ds_enabled = matches!(
            router_data.resource_common_data.auth_type,
            common_enums::AuthenticationType::ThreeDs
        );
        let payment_method_options = Some(PaymentMethodOptions {
            three_ds: three_ds_enabled,
        });

        // On Charge, return_url arrives on `request.router_return_url`;
        // `PaymentFlowData.return_url` is hardcoded to None for this flow.
        let return_url = request
            .router_return_url
            .clone()
            .or_else(|| router_data.resource_common_data.return_url.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
            currency: request.currency,
            payment_method: RapydPaymentMethodData::Token(Secret::new(card_id)),
            // Honor the caller's capture intent; SequentialAutomatic and
            // unspecified default to auto-capture, matching the Authorize flow.
            capture: Some(request.is_auto_capture()),
            payment_method_options,
            merchant_reference_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: None,
            error_payment_url: Some(return_url.clone()),
            complete_payment_url: Some(return_url),
            customer: Some(RapydCustomerRef::Id(customer_id)),
            save_payment_method: None,
            initiation_type: Some(RapydInitiationType::Recurring),
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RapydRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RapydRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = match &item.response.data {
            Some(data) => {
                let attempt_status =
                    get_status(data.status.to_owned(), data.next_action.to_owned());
                match attempt_status {
                    common_enums::AttemptStatus::Failure => (
                        common_enums::AttemptStatus::Failure,
                        Err(ErrorResponse {
                            code: data
                                .failure_code
                                .to_owned()
                                .unwrap_or(item.response.status.error_code.clone()),
                            status_code: item.http_code,
                            message: item.response.status.status.clone().unwrap_or_else(|| {
                                common_utils::consts::NO_ERROR_MESSAGE.to_string()
                            }),
                            reason: data.failure_message.to_owned(),
                            attempt_status: None,
                            // Preserve the connector's transaction id on
                            // failure so reconciliation / support lookups
                            // can locate the attempt in Rapyd's dashboard.
                            connector_transaction_id: Some(data.id.clone()),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                        }),
                    ),
                    _ => (
                        attempt_status,
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(data.id.clone()),
                            redirection_data: None,
                            // MIT replay does not mint a new mandate — the
                            // `connector_customer` + `card_*` from SetupMandate
                            // stay valid. `data.id` is a one-shot payment id
                            // and must never be stored as a mandate.
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: data.merchant_reference_id.to_owned(),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        }),
                    ),
                }
            }
            None => (
                common_enums::AttemptStatus::Failure,
                Err(ErrorResponse {
                    code: item.response.status.error_code.clone(),
                    status_code: item.http_code,
                    message: item
                        .response
                        .status
                        .status
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: item.response.status.message.clone(),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
            ),
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}
