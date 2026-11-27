use crate::connectors::revolut::RevolutRouterData;
use domain_types::{
    connector_flow::{Authorize, Capture, ClientAuthenticationToken, PSync, Refund},
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
        RevolutClientAuthenticationResponse as RevolutClientAuthenticationResponseDomain,
        WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};

use crate::types::ResponseRouterData;
use common_enums::AttemptStatus;
use common_utils::{custom_serde, types::MinorUnit};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use time::PrimitiveDateTime;

pub struct RevolutAuthType {
    pub secret_api_key: Secret<String>,
    pub signing_secret: Option<Secret<String>>,
}

/// Optional Revolut settings supplied through connector_feature_data.
#[derive(Debug, Default, Deserialize)]
pub struct RevolutOrderOptions {
    /// Optional flow override; a selected Revolut Pay method never opens hosted checkout.
    pub checkout_mode: Option<RevolutCheckoutMode>,
    pub customer_id: Option<String>,
    pub location_id: Option<String>,
    pub expire_pending_after: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RevolutCheckoutMode {
    Hosted,
    Redirection,
    Sdk,
}

fn checkout_base(sandbox: bool) -> &'static str {
    if sandbox {
        "https://sandbox-checkout.revolut.com"
    } else {
        "https://checkout.revolut.com"
    }
}

fn revolut_pay_redirect(
    token: &str,
    key: &str,
    return_url: &str,
    sandbox: bool,
) -> Result<String, error_stack::Report<ConnectorError>> {
    let mut url = url::Url::parse(checkout_base(sandbox))
        .change_context(ConnectorError::response_handling_failed_http_status_unknown())?;
    url.path_segments_mut()
        .map_err(|_| ConnectorError::response_handling_failed_http_status_unknown())?
        .extend(["revolut-pay", "order", token]);
    url.query_pairs_mut()
        .append_pair("publicToken", key)
        .append_pair("redirectUrls.success", return_url)
        .append_pair("redirectUrls.failure", return_url)
        .append_pair("redirectUrls.cancel", return_url);
    Ok(url.to_string())
}

impl RevolutOrderOptions {
    fn checkout_mode_for<T: PaymentMethodDataTypes>(
        &self,
        payment_method: &PaymentMethodData<T>,
    ) -> RevolutCheckoutMode {
        match (payment_method, self.checkout_mode) {
            (_, Some(RevolutCheckoutMode::Sdk)) => RevolutCheckoutMode::Sdk,
            (PaymentMethodData::Wallet(WalletData::RevolutPay(_)), _) => {
                RevolutCheckoutMode::Redirection
            }
            (_, mode) => mode.unwrap_or(RevolutCheckoutMode::Hosted),
        }
    }

    pub fn parse(
        value: Option<&Secret<serde_json::Value>>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        value
            .map(|value| {
                serde_json::from_value(value.peek().clone()).change_context(
                    IntegrationError::InvalidDataFormat {
                        field_name: "connector_feature_data",
                        context: Default::default(),
                    },
                )
            })
            .transpose()
            .map(|value| value.unwrap_or_default())
    }
}

// Revolut uses the same event names for payments and refunds. Mark new refund
// references so the stateless webhook parser can identify them without a GET.
pub const REFUND_REFERENCE_PREFIX: &str = "prism:revolut:refund:";

fn build_customer(
    id: Option<String>,
    email: Option<common_utils::pii::Email>,
    name: Option<Secret<String>>,
) -> Option<RevolutCustomer> {
    if let Some(id) = id {
        Some(RevolutCustomer {
            id: Some(id),
            full_name: None,
            phone: None,
            email: None,
            date_of_birth: None,
        })
    } else {
        email.map(|email| RevolutCustomer {
            id: None,
            full_name: name,
            phone: None,
            email: Some(email),
            date_of_birth: None,
        })
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct RevolutOrderCreateRequest {
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub settlement_currency: Option<String>,
    pub description: Option<String>,
    pub customer: Option<RevolutCustomer>,
    pub enforce_challenge: Option<RevolutEnforceChallengeMode>,
    pub line_items: Option<Vec<RevolutLineItem>>,
    pub shipping: Option<RevolutShipping>,
    pub capture_mode: Option<RevolutCaptureMode>,
    pub expire_pending_after: Option<String>,
    pub cancel_authorised_after: Option<String>,
    pub location_id: Option<String>,
    pub metadata: Option<Secret<serde_json::Value>>,
    pub industry_data: Option<Secret<serde_json::Value>>,
    pub merchant_order_data: Option<RevolutMerchantOrderData>,
    pub upcoming_payment_data: Option<serde_json::Value>,
    pub redirect_url: Option<String>,
    pub statement_descriptor_suffix: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutCustomer {
    pub id: Option<String>,
    pub full_name: Option<Secret<String>>,
    pub phone: Option<Secret<String>>,
    pub email: Option<common_utils::pii::Email>,
    pub date_of_birth: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevolutEnforceChallengeMode {
    #[default]
    Automatic,
    Forced,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutLineItem {
    pub name: String,
    pub r#type: RevolutLineItemType,
    pub quantity: RevolutLineItemQuantity,
    pub unit_price_amount: MinorUnit, //integer(int64)
    pub total_amount: MinorUnit,      //integer(int64)
    pub external_id: Option<String>,
    pub discounts: Option<Vec<RevolutLineItemDiscount>>,
    pub taxes: Option<Vec<RevolutLineItemTax>>,
    pub image_urls: Option<Vec<String>>,
    pub description: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevolutLineItemType {
    Physical,
    Service,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutLineItemQuantity {
    pub value: f64, // number(double)
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutLineItemDiscount {
    pub name: String,
    pub amount: MinorUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutLineItemTax {
    pub name: String,
    pub amount: MinorUnit,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutShipping {
    pub address: Option<RevolutAddress>,
    pub contact: Option<RevolutContact>,
    pub shipments: Option<Vec<RevolutShipment>>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutAddress {
    pub street_line_1: Option<String>,
    pub street_line_2: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub country_code: Option<String>,
    pub postcode: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutContact {
    pub full_name: Option<Secret<String>>,
    pub phone: Option<Secret<String>>,
    pub email: Option<common_utils::pii::Email>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutShipment {
    pub shipping_company_name: String,
    pub tracking_number: String,
    pub estimated_delivery_date: Option<String>,
    pub tracking_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RevolutErrorResponse {
    StandardError {
        code: String,
        message: String,
        timestamp: i64,
    },
    ErrorIdResponse {
        #[serde(rename = "errorId")]
        error_id: String,
        timestamp: i64,
        code: Option<i64>,
    },
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevolutOrderCreateResponse {
    pub id: String,
    pub token: Secret<String>,
    pub r#type: RevolutOrderType,
    pub state: RevolutOrderState,
    #[serde(with = "custom_serde::iso8601")]
    pub created_at: PrimitiveDateTime,
    #[serde(with = "custom_serde::iso8601")]
    pub updated_at: PrimitiveDateTime,
    pub description: Option<String>,
    pub capture_mode: Option<RevolutCaptureMode>,
    pub cancel_authorised_after: Option<String>,
    pub amount: MinorUnit,
    pub outstanding_amount: Option<MinorUnit>,
    pub refunded_amount: Option<MinorUnit>,
    pub currency: common_enums::Currency,
    pub settlement_currency: Option<String>,
    pub customer: Option<RevolutCustomer>,
    pub payments: Option<Vec<RevolutPayment>>,
    pub location_id: Option<String>,
    pub metadata: Option<Secret<serde_json::Value>>,
    pub industry_data: Option<Secret<serde_json::Value>>,
    pub merchant_order_data: Option<RevolutMerchantOrderData>,
    pub upcoming_payment_data: Option<RevolutUpcomingPaymentData>,
    pub checkout_url: Option<String>,
    pub redirect_url: Option<String>,
    pub shipping: Option<RevolutShipping>,
    pub enforce_challenge: Option<RevolutEnforceChallengeMode>,
    pub line_items: Option<Vec<RevolutLineItem>>,
    pub statement_descriptor_suffix: Option<String>,
    pub related_order_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutOrderType {
    Payment,
    PaymentRequest,
    Refund,
    Chargeback,
    ChargebackReversal,
    CreditReimbursement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutOrderState {
    Pending,
    Processing,
    Authorised,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevolutCaptureMode {
    Automatic,
    Manual,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutPayment {
    pub id: String,
    pub state: RevolutPaymentState,
    pub decline_reason: Option<RevolutDeclineReason>,
    pub bank_message: Option<String>,
    #[serde(with = "custom_serde::iso8601")]
    pub created_at: PrimitiveDateTime,
    #[serde(with = "custom_serde::iso8601")]
    pub updated_at: PrimitiveDateTime,
    pub token: Option<String>,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub settled_amount: Option<MinorUnit>,
    pub settled_currency: Option<String>,
    pub payment_method: Option<RevolutPaymentMethod>,
    pub authentication_challenge: Option<RevolutAuthenticationChallenge>,
    pub billing_address: Option<RevolutAddress>,
    pub risk_level: Option<RevolutRiskLevel>,
    pub fees: Option<Vec<RevolutFee>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutPaymentState {
    Pending,
    AuthenticationChallenge,
    AuthenticationVerified,
    AuthorisationStarted,
    AuthorisationPassed,
    Authorised,
    CaptureStarted,
    Captured,
    RefundValidated,
    RefundStarted,
    CancellationStarted,
    Declining,
    Completing,
    Cancelling,
    Failing,
    Completed,
    Declined,
    SoftDeclined,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutDeclineReason {
    #[serde(rename = "3ds_challenge_abandoned")]
    ThreeDsChallengeAbandoned,
    #[serde(rename = "3ds_challenge_failed_manually")]
    ThreeDsChallengeFailedManually,
    CardholderNameMissing,
    CustomerChallengeAbandoned,
    CustomerChallengeFailed,
    CustomerNameMismatch,
    DoNotHonour,
    ExpiredCard,
    HighRisk,
    InsufficientFunds,
    InvalidAddress,
    InvalidAmount,
    InvalidCard,
    InvalidCountry,
    InvalidCvv,
    InvalidEmail,
    InvalidExpiry,
    InvalidMerchant,
    InvalidPhone,
    InvalidPin,
    IssuerNotAvailable,
    PickUpCard,
    RejectedByCustomer,
    RestrictedCard,
    SuspectedFraud,
    TechnicalError,
    TransactionNotAllowedForCardholder,
    UnknownCard,
    WithdrawalLimitExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutMerchantOrderData {
    pub url: Option<String>,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutUpcomingPaymentData {
    #[serde(with = "custom_serde::iso8601")]
    pub date: PrimitiveDateTime,
    pub payment_method_id: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutRiskLevel {
    Low,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutFee {
    pub r#type: RevolutFeeType,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutFeeType {
    Fx,
    Acquiring,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevolutPaymentMethod {
    ApplePay(RevolutCardDetails),
    Card(RevolutCardDetails),
    GooglePay(RevolutCardDetails),
    RevolutPayCard(RevolutCardDetails),
    RevolutPayAccount(RevolutAccountDetails),
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutCardDetails {
    pub id: Option<String>,
    pub card_brand: Option<RevolutCardBrand>,
    pub funding: Option<RevolutCardFunding>,
    pub card_country_code: Option<String>,
    pub card_bin: Option<String>,
    pub card_last_four: Option<String>,
    pub card_expiry: Option<String>,
    pub cardholder_name: Option<Secret<String>>,
    pub checks: Option<RevolutPaymentChecks>,
    pub authorisation_code: Option<String>,
    pub arn: Option<String>,
    pub fingerprint: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutAccountDetails {
    pub id: Option<String>,
    pub fingerprint: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutCardBrand {
    Visa,
    Mastercard,
    AmericanExpress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutCardFunding {
    Credit,
    Debit,
    Prepaid,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutPaymentChecks {
    pub three_ds: Option<RevolutThreeDsCheck>,
    pub cvv_verification: Option<RevolutVerificationResult>,
    pub address: Option<RevolutVerificationResult>,
    pub postcode: Option<RevolutVerificationResult>,
    pub cardholder: Option<RevolutVerificationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutThreeDsCheck {
    pub eci: Option<String>,
    pub state: Option<RevolutThreeDsState>,
    pub version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutThreeDsState {
    Verified,
    Failed,
    Challenge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolutVerificationResult {
    Match,
    NotMatch,
    #[serde(rename = "n_a")]
    NA,
    Invalid,
    #[serde(rename = "incorrect")]
    Incorrect,
    #[serde(rename = "not_processed")]
    NotProcessed,
}
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevolutAuthenticationChallenge {
    ThreeDs(RevolutThreeDsChallenge),
    ThreeDsFingerprint(RevolutThreeDsFingerprintChallenge),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutThreeDsChallenge {
    pub acs_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutThreeDsFingerprintChallenge {
    pub fingerprint_url: String,
    pub fingerprint_data: String,
}

impl TryFrom<&ConnectorSpecificConfig> for RevolutAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Revolut {
                secret_api_key,
                signing_secret,
                ..
            } => Ok(Self {
                secret_api_key: secret_api_key.to_owned(),
                signing_secret: signing_secret.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevolutRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RevolutOrderCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RevolutRouterData<
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

        let options =
            RevolutOrderOptions::parse(router_data.request.connector_feature_data.as_ref())?;
        let checkout_mode = options.checkout_mode_for(&router_data.request.payment_method_data);
        if matches!(
            checkout_mode,
            RevolutCheckoutMode::Redirection | RevolutCheckoutMode::Sdk
        ) {
            let has_key = matches!(&router_data.connector_config,
                ConnectorSpecificConfig::Revolut { public_key: Some(key), .. } if !key.peek().is_empty());
            if !has_key
                || (checkout_mode == RevolutCheckoutMode::Redirection
                    && router_data
                        .request
                        .router_return_url
                        .as_ref()
                        .is_none_or(|url| url.is_empty()))
            {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name:
                        "Revolut Pay requires public_key and redirection requires return_url",
                    context: Default::default(),
                }
                .into());
            }
        }
        let customer = build_customer(
            router_data
                .resource_common_data
                .connector_customer
                .clone()
                .or(options.customer_id),
            router_data.request.email.clone(),
            router_data.request.customer_name.clone().map(Secret::new),
        );
        if router_data
            .resource_common_data
            .connector_request_reference_id
            .starts_with(REFUND_REFERENCE_PREFIX)
        {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "merchant_transaction_id (reserved Revolut refund prefix)",
                context: Default::default(),
            }
            .into());
        }

        // Map shipping data from resource_common_data
        let shipping = router_data
            .resource_common_data
            .get_optional_shipping()
            .map(|_shipping_address| {
                // Create contact with at least email or phone
                // Use shipping email if available, otherwise use customer email
                let contact_email = router_data
                    .resource_common_data
                    .get_optional_shipping_email()
                    .or_else(|| router_data.request.email.clone());

                // Only include contact if we have email or phone
                let contact = if contact_email.is_some()
                    || router_data
                        .resource_common_data
                        .get_optional_shipping_phone_number()
                        .is_some()
                {
                    Some(RevolutContact {
                        full_name: router_data
                            .resource_common_data
                            .get_optional_shipping_full_name(),
                        phone: router_data
                            .resource_common_data
                            .get_optional_shipping_phone_number(),
                        email: contact_email,
                    })
                } else {
                    None
                };

                RevolutShipping {
                    address: Some(RevolutAddress {
                        street_line_1: router_data
                            .resource_common_data
                            .get_optional_shipping_line1()
                            .map(|line1| line1.expose()),
                        street_line_2: router_data
                            .resource_common_data
                            .get_optional_shipping_line2()
                            .map(|line2| line2.expose()),
                        region: router_data
                            .resource_common_data
                            .get_optional_shipping_state()
                            .map(|state| state.expose()),
                        city: router_data
                            .resource_common_data
                            .get_optional_shipping_city()
                            .map(|city| city.expose()),
                        country_code: router_data
                            .resource_common_data
                            .get_optional_shipping_country()
                            .map(|country| country.to_string()),
                        postcode: router_data
                            .resource_common_data
                            .get_optional_shipping_zip()
                            .map(|zip| zip.expose()),
                    }),
                    contact,
                    shipments: None,
                }
            });

        // Map merchant_order_data from connector_request_reference_id
        let merchant_order_data = Some(RevolutMerchantOrderData {
            url: router_data.request.router_return_url.clone(),
            reference: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        });

        Ok(Self {
            amount: router_data.request.money.amount(),
            currency: router_data.request.currency,
            settlement_currency: None,
            description: router_data.resource_common_data.description.clone(),
            customer,
            enforce_challenge: None,
            line_items: None,
            shipping,
            capture_mode: router_data.request.capture_method.map(|c| match c {
                common_enums::CaptureMethod::Manual => RevolutCaptureMode::Manual,
                _ => RevolutCaptureMode::Automatic,
            }),
            expire_pending_after: options.expire_pending_after,
            cancel_authorised_after: None,
            location_id: options.location_id,
            metadata: router_data.request.metadata.clone(),
            industry_data: None,
            merchant_order_data,
            upcoming_payment_data: None,
            redirect_url: router_data.request.router_return_url.clone(),
            statement_descriptor_suffix: router_data
                .request
                .billing_descriptor
                .as_ref()
                .and_then(|bd| bd.statement_descriptor_suffix.clone()),
        })
    }
}

fn order_metadata(response: &RevolutOrderCreateResponse) -> Option<serde_json::Value> {
    let customer_id = response
        .customer
        .as_ref()
        .and_then(|customer| customer.id.as_ref());
    let payments: Vec<_> = response
        .payments
        .iter()
        .flatten()
        .filter_map(|payment| {
            payment.payment_method.as_ref().map(|method| {
                serde_json::json!({
                    "id": payment.id,
                    "state": payment.state,
                    "payment_method_type": match method {
                        RevolutPaymentMethod::ApplePay(_) => "apple_pay",
                        RevolutPaymentMethod::Card(_) => "card",
                        RevolutPaymentMethod::GooglePay(_) => "google_pay",
                        RevolutPaymentMethod::RevolutPayCard(_) => "revolut_pay_card",
                        RevolutPaymentMethod::RevolutPayAccount(_) => "revolut_pay_account",
                    },
                })
            })
        })
        .collect();
    let mut metadata = serde_json::Map::new();
    if let Some(id) = customer_id {
        metadata.insert("customer_id".into(), serde_json::json!(id));
    }
    if !payments.is_empty() {
        metadata.insert("payments".into(), serde_json::json!(payments));
    }
    (!metadata.is_empty()).then_some(serde_json::Value::Object(metadata))
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RevolutOrderCreateResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let options =
            RevolutOrderOptions::parse(item.router_data.request.connector_feature_data.as_ref())
                .change_context(ConnectorError::response_handling_failed_http_status_unknown())?;
        let mut metadata: serde_json::Map<String, serde_json::Value> = order_metadata(&response)
            .map(serde_json::from_value)
            .transpose()
            .change_context(ConnectorError::response_handling_failed_http_status_unknown())?
            .unwrap_or_default();
        metadata.insert("order_id".into(), serde_json::json!(response.id));
        metadata.insert(
            "order_token".into(),
            serde_json::json!(response.token.peek()),
        );
        let public_key = match &item.router_data.connector_config {
            ConnectorSpecificConfig::Revolut { public_key, .. } => public_key.as_ref(),
            _ => None,
        };
        let sandbox = item
            .router_data
            .resource_common_data
            .test_mode
            .unwrap_or(false);
        if let Some(key) = public_key {
            metadata.insert("public_key".into(), serde_json::json!(key.peek()));
            metadata.insert(
                "checkout_base_url".into(),
                serde_json::json!(checkout_base(sandbox)),
            );
        }
        let status = match response.state {
            RevolutOrderState::Authorised => AttemptStatus::Authorized,
            RevolutOrderState::Completed => AttemptStatus::Charged,
            RevolutOrderState::Failed => AttemptStatus::Failure,
            RevolutOrderState::Cancelled => AttemptStatus::Voided,
            RevolutOrderState::Pending => AttemptStatus::AuthenticationPending,
            RevolutOrderState::Processing => AttemptStatus::Pending,
        };

        let redirection_data =
            match options.checkout_mode_for(&item.router_data.request.payment_method_data) {
                RevolutCheckoutMode::Redirection => {
                    let key = public_key
                        .ok_or_else(ConnectorError::response_handling_failed_http_status_unknown)?;
                    let return_url = item
                        .router_data
                        .request
                        .router_return_url
                        .as_deref()
                        .ok_or_else(ConnectorError::response_handling_failed_http_status_unknown)?;
                    Some(Box::new(RedirectForm::Uri {
                        uri: revolut_pay_redirect(
                            response.token.peek(),
                            key.peek(),
                            return_url,
                            sandbox,
                        )?,
                    }))
                }
                RevolutCheckoutMode::Sdk => None,
                _ => response
                    .checkout_url
                    .as_ref()
                    .map(|url| Box::new(RedirectForm::Uri { uri: url.clone() })),
            };

        let merchant_reference = response
            .merchant_order_data
            .as_ref()
            .and_then(|m| m.reference.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: Some(serde_json::Value::Object(metadata)),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: merchant_reference,
                incremental_authorization_allowed: None,
                status_code: 200,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RevolutOrderCreateResponse, RevolutRouterData<Self, T>>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutOrderCreateResponse, RevolutRouterData<Self, T>>,
    ) -> Result<Self, Self::Error> {
        let connector = item.router_data.connector;
        let item = ResponseRouterData {
            response: item.response,
            router_data: item.router_data.router_data,
            http_code: item.http_code,
        };
        let response = item.response;

        let metadata = order_metadata(&response);
        let status = match &response.payments {
            Some(payments) => match payments.first() {
                Some(first_payment) => match first_payment.state {
                    RevolutPaymentState::Authorised => AttemptStatus::Authorized,
                    RevolutPaymentState::Captured | RevolutPaymentState::Completed => {
                        AttemptStatus::Charged
                    }
                    RevolutPaymentState::Failed | RevolutPaymentState::Declined => {
                        AttemptStatus::Failure
                    }
                    RevolutPaymentState::Cancelled => AttemptStatus::Voided,
                    RevolutPaymentState::Pending => AttemptStatus::Pending,
                    RevolutPaymentState::AuthenticationChallenge => {
                        AttemptStatus::AuthenticationPending
                    }
                    _ => AttemptStatus::Pending,
                },
                None => map_order_state(response.state),
            },
            None => map_order_state(response.state),
        };

        let amount = Some(crate::utils::response_amount_to_money(
            &connector,
            response.amount,
            response.currency,
            item.http_code,
            domain_types::connector_types::ConnectorEnum::Revolut,
        )?);

        let merchant_reference = response
            .merchant_order_data
            .as_ref()
            .and_then(|m| m.reference.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: metadata,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: merchant_reference,
                incremental_authorization_allowed: None,
                status_code: 200,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                money: amount,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

fn map_order_state(state: RevolutOrderState) -> AttemptStatus {
    match state {
        RevolutOrderState::Authorised => AttemptStatus::Authorized,
        RevolutOrderState::Completed => AttemptStatus::Charged,
        RevolutOrderState::Failed => AttemptStatus::Failure,
        RevolutOrderState::Cancelled => AttemptStatus::Voided,
        RevolutOrderState::Pending => AttemptStatus::AuthenticationPending,
        RevolutOrderState::Processing => AttemptStatus::Pending,
    }
}

#[derive(Debug, Serialize)]
pub struct RevolutCaptureRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevolutRefundResponse {
    pub id: String,
    pub r#type: RevolutOrderType,
    pub state: RevolutOrderState,
    #[serde(with = "custom_serde::iso8601")]
    pub created_at: PrimitiveDateTime,
    #[serde(with = "custom_serde::iso8601")]
    pub updated_at: PrimitiveDateTime,
    pub description: Option<String>,
    pub capture_mode: Option<RevolutCaptureMode>,
    pub cancel_authorised_after: Option<String>,
    pub amount: MinorUnit,
    pub outstanding_amount: Option<MinorUnit>,
    pub refunded_amount: Option<MinorUnit>,
    pub currency: common_enums::Currency,
    pub settlement_currency: Option<String>,
    pub customer: Option<RevolutCustomer>,
    pub payments: Option<Vec<RevolutPayment>>,
    pub location_id: Option<String>,
    pub metadata: Option<Secret<serde_json::Value>>,
    pub industry_data: Option<Secret<serde_json::Value>>,
    pub merchant_order_data: Option<RevolutMerchantOrderData>,
    pub upcoming_payment_data: Option<RevolutUpcomingPaymentData>,
    pub checkout_url: Option<String>,
    pub redirect_url: Option<String>,
    pub shipping: Option<RevolutShipping>,
    pub enforce_challenge: Option<RevolutEnforceChallengeMode>,
    pub line_items: Option<Vec<RevolutLineItem>>,
    pub statement_descriptor_suffix: Option<String>,
    pub related_order_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RevolutRefundRequest {
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub merchant_order_data: Option<RevolutMerchantOrderData>,
    pub metadata: Option<Secret<serde_json::Value>>,
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevolutRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for RevolutRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RevolutRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        Ok(Self {
            amount: router_data.request.refund_money.amount(),
            currency: router_data.request.currency,
            merchant_order_data: Some(RevolutMerchantOrderData {
                reference: Some(format!(
                    "{}{}",
                    REFUND_REFERENCE_PREFIX,
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                )),
                url: None,
            }),
            metadata: router_data.request.connector_feature_data.clone(),
            description: router_data.request.reason.clone(),
        })
    }
}

impl<F> TryFrom<ResponseRouterData<RevolutRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = match response.state {
            RevolutOrderState::Completed => common_enums::RefundStatus::Success,
            RevolutOrderState::Processing => common_enums::RefundStatus::Pending,
            RevolutOrderState::Failed => common_enums::RefundStatus::Failure,
            RevolutOrderState::Cancelled => common_enums::RefundStatus::Failure,
            _ => common_enums::RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<RevolutRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevolutRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = match response.state {
            RevolutOrderState::Completed => common_enums::RefundStatus::Success,
            RevolutOrderState::Processing => common_enums::RefundStatus::Pending,
            RevolutOrderState::Failed => common_enums::RefundStatus::Failure,
            RevolutOrderState::Cancelled => common_enums::RefundStatus::Failure,
            _ => common_enums::RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevolutRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for RevolutCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RevolutRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Amount is optional - if not provided, Revolut captures full authorized amount
        let amount = Some(item.router_data.request.capture_money.amount());

        Ok(Self { amount })
    }
}

impl<F> TryFrom<ResponseRouterData<RevolutOrderCreateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<RevolutOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let metadata = order_metadata(&response);
        let status = match response.state {
            RevolutOrderState::Completed => AttemptStatus::Charged,
            RevolutOrderState::Authorised => AttemptStatus::Authorized,
            RevolutOrderState::Processing => AttemptStatus::Pending,
            RevolutOrderState::Pending => AttemptStatus::Pending,
            RevolutOrderState::Failed => AttemptStatus::Failure,
            RevolutOrderState::Cancelled => AttemptStatus::Voided,
        };

        let connector_transaction_id = response
            .payments
            .and_then(|payments| payments.first().map(|p| p.id.clone()))
            .unwrap_or_else(|| response.id.clone());

        let merchant_reference = response
            .merchant_order_data
            .as_ref()
            .and_then(|m| m.reference.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                connector_metadata: metadata,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: merchant_reference,
                incremental_authorization_allowed: None,
                mandate_reference: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// Webhook related structures and implementations

#[derive(Debug, Serialize, Deserialize)]
pub struct RevolutWebhookBody {
    pub event: RevolutWebhookEvent,
    pub order_id: String,
    pub merchant_order_ext_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RevolutWebhookEvent {
    OrderCompleted,
    OrderAuthorised,
    OrderCancelled,
    OrderFailed,
    OrderPaymentAuthenticated,
    OrderPaymentDeclined,
    OrderPaymentFailed,
    PayoutInitiated,
    PayoutCompleted,
    PayoutFailed,
    DisputeActionRequired,
    DisputeUnderReview,
    DisputeWon,
    DisputeLost,
}

/// Maps Revolut webhook event to AttemptStatus for webhook processing
fn map_webhook_event_to_attempt_status(
    event: RevolutWebhookEvent,
) -> Result<AttemptStatus, IntegrationError> {
    match event {
        RevolutWebhookEvent::OrderCompleted => Ok(AttemptStatus::Charged),
        RevolutWebhookEvent::OrderAuthorised => Ok(AttemptStatus::Authorized),
        RevolutWebhookEvent::OrderCancelled => Ok(AttemptStatus::Voided),
        RevolutWebhookEvent::OrderFailed
        | RevolutWebhookEvent::OrderPaymentFailed
        | RevolutWebhookEvent::OrderPaymentDeclined => Ok(AttemptStatus::Failure),
        RevolutWebhookEvent::OrderPaymentAuthenticated => {
            Ok(AttemptStatus::AuthenticationSuccessful)
        }
        _ => Err(IntegrationError::NotImplemented(
            "webhook event type not found".to_string(),
            Default::default(),
        )),
    }
}

impl TryFrom<RevolutWebhookBody> for WebhookDetailsResponse {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(webhook_body: RevolutWebhookBody) -> Result<Self, Self::Error> {
        let status = map_webhook_event_to_attempt_status(webhook_body.event)?;

        Ok(Self {
            raw_connector_status: None,
            amount: None,
            connector_returned_payment_method_details: None,
            resource_id: Some(ResponseId::ConnectorTransactionId(
                webhook_body.order_id.clone(),
            )),
            status,
            error_code: None,
            error_message: None,
            error_reason: None,
            status_code: 200,
            connector_response_reference_id: webhook_body.merchant_order_ext_ref.clone(),
            connector_request_reference_id: webhook_body.merchant_order_ext_ref,
            mandate_reference: None,
            raw_connector_response: None,
            response_headers: None,
            captured_money: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Revolut order for client-side SDK initialization.
/// The order's `token` is returned to the frontend for Revolut Pay
/// widget or Revolut checkout initialization.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct RevolutClientAuthRequest {
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub customer: Option<RevolutCustomer>,
    pub location_id: Option<String>,
    pub expire_pending_after: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevolutRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RevolutClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: RevolutRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let options = RevolutOrderOptions::parse(
            router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        )?;
        let customer = router_data.request.customer.as_ref();
        Ok(Self {
            amount: router_data.request.money.amount(),
            currency: router_data.request.currency,
            customer: build_customer(
                options.customer_id,
                customer.and_then(|c| c.customer_email.clone()),
                customer.and_then(|c| c.get_full_name()),
            ),
            location_id: options.location_id,
            expire_pending_after: options.expire_pending_after,
        })
    }
}

/// Revolut order response containing order id and token for SDK initialization.
#[derive(Debug, Deserialize, Serialize)]
pub struct RevolutClientAuthResponse {
    pub customer: Option<RevolutCustomer>,
    pub id: String,
    pub token: Secret<String>,
}

impl TryFrom<ResponseRouterData<RevolutClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<RevolutClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Revolut(
                RevolutClientAuthenticationResponseDomain {
                    customer_id: response.customer.and_then(|customer| customer.id),
                    order_id: response.id,
                    token: response.token,
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selected_revolut_pay_routes_without_bridge_options() {
        use domain_types::payment_method_data::{DefaultPCIHolder, RevolutPayData};
        let selected = PaymentMethodData::<DefaultPCIHolder>::Wallet(WalletData::RevolutPay(
            RevolutPayData {},
        ));
        let other = PaymentMethodData::<DefaultPCIHolder>::Reward;
        let mut options = RevolutOrderOptions::default();
        assert_eq!(
            options.checkout_mode_for(&selected),
            RevolutCheckoutMode::Redirection
        );
        assert_eq!(
            options.checkout_mode_for(&other),
            RevolutCheckoutMode::Hosted
        );
        options.checkout_mode = Some(RevolutCheckoutMode::Hosted);
        assert_eq!(
            options.checkout_mode_for(&selected),
            RevolutCheckoutMode::Redirection
        );
        options.checkout_mode = Some(RevolutCheckoutMode::Sdk);
        assert_eq!(
            options.checkout_mode_for(&selected),
            RevolutCheckoutMode::Sdk
        );
    }

    #[test]
    fn dedicated_redirect_encodes_tokens_and_callbacks_and_uses_correct_environment() {
        for sandbox in [true, false] {
            let callback = "https://merchant.example/return?attempt=1&next=/orders";
            let link =
                revolut_pay_redirect("token/with?reserved", "pk_+&", callback, sandbox).unwrap();
            let parsed = url::Url::parse(&link).unwrap();
            assert_eq!(
                parsed.host_str(),
                Some(if sandbox {
                    "sandbox-checkout.revolut.com"
                } else {
                    "checkout.revolut.com"
                })
            );
            assert!(parsed.path().contains("token%2Fwith%3Freserved"));
            let params: std::collections::HashMap<_, _> = parsed.query_pairs().collect();
            assert_eq!(params["publicToken"], "pk_+&");
            for key in [
                "redirectUrls.success",
                "redirectUrls.failure",
                "redirectUrls.cancel",
            ] {
                assert_eq!(params[key], callback);
            }
        }
    }

    #[test]
    fn checkout_modes_are_explicit_and_invalid_modes_fail() {
        assert!(RevolutOrderOptions::parse(None)
            .unwrap()
            .checkout_mode
            .is_none());
        for mode in ["hosted", "redirection", "sdk"] {
            assert!(
                RevolutOrderOptions::parse(Some(&Secret::new(json!({"checkout_mode": mode}))))
                    .is_ok()
            );
        }
        assert!(
            RevolutOrderOptions::parse(Some(&Secret::new(json!({"checkout_mode": "typo"}))))
                .is_err()
        );
    }

    #[test]
    fn returning_customer_sends_only_the_existing_id_even_without_email() {
        let customer = build_customer(Some("customer-123".into()), None, None);
        assert_eq!(
            serde_json::to_value(customer).unwrap(),
            json!({"id": "customer-123"})
        );
        let email = "customer@example.com".parse().unwrap();
        let customer = build_customer(
            Some("customer-123".into()),
            Some(email),
            Some(Secret::new("New Name".into())),
        );
        assert_eq!(
            serde_json::to_value(customer).unwrap(),
            json!({"id": "customer-123"})
        );
    }

    #[test]
    fn new_customer_retains_email_and_name_and_anonymous_customer_is_omitted() {
        let email = "customer@example.com".parse().unwrap();
        let customer = build_customer(None, Some(email), Some(Secret::new("Customer".into())));
        assert_eq!(
            serde_json::to_value(customer).unwrap(),
            json!({"email": "customer@example.com", "full_name": "Customer"})
        );
        assert!(build_customer(None, None, None).is_none());
    }

    #[test]
    fn checkout_options_are_optional_and_invalid_types_are_rejected() {
        let options = RevolutOrderOptions::parse(None).unwrap();
        assert!(options.customer_id.is_none());
        assert!(options.expire_pending_after.is_none());
        let options = RevolutOrderOptions::parse(Some(&Secret::new(json!({
            "customer_id": "customer-123", "location_id": "store-123", "expire_pending_after": "PT30M"
        })))).unwrap();
        let request = RevolutClientAuthRequest {
            amount: MinorUnit::new(100),
            currency: common_enums::Currency::GBP,
            customer: build_customer(options.customer_id, None, None),
            location_id: options.location_id,
            expire_pending_after: options.expire_pending_after,
        };
        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({
                "amount": 100, "currency": "GBP", "customer": {"id": "customer-123"},
                "location_id": "store-123", "expire_pending_after": "PT30M"
            })
        );
        assert!(RevolutOrderOptions::parse(Some(&Secret::new(
            json!({"expire_pending_after": 30})
        )))
        .is_err());
    }

    #[test]
    fn sync_metadata_preserves_each_attempts_method_without_card_details() {
        let methods = [
            "card",
            "apple_pay",
            "google_pay",
            "revolut_pay_card",
            "revolut_pay_account",
        ];
        let payments: Vec<_> = methods.iter().enumerate().map(|(index, method)| json!({
            "id": format!("payment-{index}"), "state": if index == 4 { "completed" } else { "failed" },
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
            "amount": 100, "currency": "GBP", "payment_method": {"type": method, "card_last_four": "4242"}
        })).collect();
        let response: RevolutOrderCreateResponse = serde_json::from_value(json!({
            "id": "order-123", "token": "token", "type": "payment", "state": "completed",
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
            "amount": 100, "currency": "GBP", "customer": {"id": "customer-123"}, "payments": payments
        })).unwrap();
        let metadata = order_metadata(&response).unwrap();
        assert_eq!(metadata["customer_id"], "customer-123");
        for (index, method) in methods.iter().enumerate() {
            assert_eq!(metadata["payments"][index]["payment_method_type"], *method);
        }
        assert_eq!(metadata["payments"][4]["state"], "completed");
        assert!(!metadata.to_string().contains("4242"));
    }

    #[test]
    fn all_payment_failure_events_and_authentication_events_are_processed() {
        for event in [
            RevolutWebhookEvent::OrderFailed,
            RevolutWebhookEvent::OrderPaymentFailed,
            RevolutWebhookEvent::OrderPaymentDeclined,
        ] {
            assert_eq!(
                map_webhook_event_to_attempt_status(event).unwrap(),
                AttemptStatus::Failure
            );
        }
        assert_eq!(
            map_webhook_event_to_attempt_status(RevolutWebhookEvent::OrderPaymentAuthenticated)
                .unwrap(),
            AttemptStatus::AuthenticationSuccessful
        );
    }
}
