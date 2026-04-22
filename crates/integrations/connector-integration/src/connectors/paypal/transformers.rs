use super::PaypalRouterData;
use crate::{
    types::ResponseRouterData,
    utils::{to_connector_meta, ErrorCodeAndMessage},
};
use base64::Engine;
use cards;
use common_enums;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::StringMajorUnit,
    CustomResult, Method,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, PostAuthenticate,
        RepeatPayment, VerifyWebhookSource,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData, MandateReference,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        PaypalClientAuthenticationResponse as PaypalClientAuthenticationResponseDomain,
        PaypalFlow as PaypalFlowDomain, PaypalTransactionInfo as PaypalTransactionInfoDomain,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SdkNextAction, ServerAuthenticationTokenResponseData, SetupMandateRequestData,
        VerifyWebhookSourceFlowData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        BankDebitData, BankRedirectData, BankTransferData, CardRedirectData, GiftCardData,
        GpayTokenizationData, PayLaterData, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber, VoucherData, WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{RedirectForm, VerifyWebhookSourceResponseData, VerifyWebhookStatus},
    utils,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use url::Url;
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
trait GetRequestIncrementalAuthorization {
    fn get_request_incremental_authorization(&self) -> Option<bool>;
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    GetRequestIncrementalAuthorization for PaymentsAuthorizeData<T>
{
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        self.request_incremental_authorization
    }
}

impl GetRequestIncrementalAuthorization for PaymentsSyncData {
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        None
    }
}

impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > GetRequestIncrementalAuthorization for RepeatPaymentData<T>
{
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        Some(false)
    }
}

pub mod auth_headers {
    pub const PAYPAL_PARTNER_ATTRIBUTION_ID: &str = "PayPal-Partner-Attribution-Id";
    pub const PREFER: &str = "Prefer";
    pub const PAYPAL_REQUEST_ID: &str = "PayPal-Request-Id";
    pub const PAYPAL_AUTH_ASSERTION: &str = "PayPal-Auth-Assertion";
}

const ORDER_QUANTITY: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaypalPaymentIntent {
    Capture,
    Authorize,
    Authenticate,
}

#[derive(Default, Debug, Clone, Serialize, Eq, PartialEq, Deserialize)]
pub struct OrderAmount {
    pub currency_code: common_enums::Currency,
    pub value: StringMajorUnit,
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OrderRequestAmount {
    pub currency_code: common_enums::Currency,
    pub value: StringMajorUnit,
    pub breakdown: AmountBreakdown,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for OrderRequestAmount
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: &PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let value = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        let shipping = item
            .router_data
            .request
            .shipping_cost
            .map(|cost| {
                item.connector
                    .amount_converter
                    .convert(cost, item.router_data.request.currency)
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: Default::default(),
                    })
                    .map(|shipping_value| OrderAmount {
                        currency_code: item.router_data.request.currency,
                        value: shipping_value,
                    })
            })
            .transpose()?;
        Ok(Self {
            currency_code: item.router_data.request.currency,
            value: value.clone(),
            breakdown: AmountBreakdown {
                item_total: OrderAmount {
                    currency_code: item.router_data.request.currency,
                    value,
                },
                tax_total: None,
                shipping,
            },
        })
    }
}

// OrderRequestAmount for RepeatPayment - RepeatPaymentData doesn't have shipping_cost
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &PaypalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for OrderRequestAmount
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: &PaypalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let value = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        let shipping_value = item
            .connector
            .amount_converter
            .convert(
                item.router_data
                    .request
                    .shipping_cost
                    .unwrap_or(common_utils::types::MinorUnit::zero()),
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            currency_code: item.router_data.request.currency,
            value: value.clone(),
            breakdown: AmountBreakdown {
                item_total: OrderAmount {
                    currency_code: item.router_data.request.currency,
                    value,
                },
                tax_total: None,
                shipping: Some(OrderAmount {
                    currency_code: item.router_data.request.currency,
                    value: shipping_value,
                }),
            },
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AmountBreakdown {
    item_total: OrderAmount,
    tax_total: Option<OrderAmount>,
    shipping: Option<OrderAmount>,
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct PurchaseUnitRequest {
    reference_id: Option<String>, //reference for an item in purchase_units
    invoice_id: Option<String>, //The API caller-provided external invoice number for this order. Appears in both the payer's transaction history and the emails that the payer receives.
    custom_id: Option<String>,  //Used to reconcile client transactions with PayPal transactions.
    amount: OrderRequestAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    payee: Option<Payee>,
    shipping: Option<ShippingAddress>,
    items: Vec<ItemDetails>,
}

#[derive(Default, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Payee {
    merchant_id: Secret<String>,
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct ItemDetails {
    name: String,
    quantity: u16,
    unit_amount: OrderAmount,
    tax: Option<OrderAmount>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ItemDetails
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: &PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let value = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            name: format!(
                "Payment for invoice {}",
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
            ),
            quantity: ORDER_QUANTITY,
            unit_amount: OrderAmount {
                currency_code: item.router_data.request.currency,
                value,
            },
            tax: None,
        })
    }
}

// ItemDetails for RepeatPayment
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &PaypalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ItemDetails
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: &PaypalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let value = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            name: format!(
                "Payment for invoice {}",
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
            ),
            quantity: ORDER_QUANTITY,
            unit_amount: OrderAmount {
                currency_code: item.router_data.request.currency,
                value,
            },
            tax: None,
        })
    }
}

#[derive(Default, Debug, Serialize, Eq, PartialEq, Deserialize)]
pub struct Address {
    address_line_1: Option<Secret<String>>,
    postal_code: Option<Secret<String>>,
    country_code: common_enums::CountryAlpha2,
    admin_area_2: Option<Secret<String>>,
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct ShippingAddress {
    address: Option<Address>,
    name: Option<ShippingName>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    From<
        &PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ShippingAddress
{
    fn from(
        item: &PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Self {
        Self {
            address: get_address_info(
                item.router_data
                    .resource_common_data
                    .get_optional_shipping(),
            ),
            name: Some(ShippingName {
                full_name: item
                    .router_data
                    .resource_common_data
                    .get_optional_shipping()
                    .and_then(|inner_data| inner_data.address.as_ref())
                    .and_then(|inner_data| inner_data.first_name.clone()),
            }),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    From<
        &PaypalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ShippingAddress
{
    fn from(
        item: &PaypalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Self {
        Self {
            address: get_address_info(
                item.router_data
                    .resource_common_data
                    .get_optional_shipping(),
            ),
            name: Some(ShippingName {
                full_name: item
                    .router_data
                    .resource_common_data
                    .get_optional_shipping()
                    .and_then(|inner_data| inner_data.address.as_ref())
                    .and_then(|inner_data| inner_data.first_name.clone()),
            }),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PaypalUpdateOrderRequest(Vec<Operation>);

impl PaypalUpdateOrderRequest {
    pub fn get_inner_value(self) -> Vec<Operation> {
        self.0
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Operation {
    pub op: PaypalOperationType,
    pub path: String,
    pub value: Value,
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum PaypalOperationType {
    Add,
    Remove,
    Replace,
    Move,
    Copy,
    Test,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Value {
    Amount(OrderRequestAmount),
    Items(Vec<ItemDetails>),
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct ShippingName {
    full_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct CardRequestStruct<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    billing_address: Option<Address>,
    expiry: Option<Secret<String>>,
    name: Option<Secret<String>>,
    number: Option<RawCardNumber<T>>,
    security_code: Option<Secret<String>>,
    attributes: Option<CardRequestAttributes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStruct {
    vault_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CardRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    CardRequestStruct(CardRequestStruct<T>),
    CardVaultStruct(VaultStruct),
}
#[derive(Debug, Serialize)]
pub struct CardRequestAttributes {
    vault: Option<PaypalVault>,
    verification: Option<ThreeDsMethod>,
}

#[derive(Debug, Serialize)]
pub struct ThreeDsMethod {
    method: ThreeDsType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDsType {
    ScaAlways,
}

#[derive(Debug, Serialize)]
pub struct RedirectRequest {
    name: Secret<String>,
    country_code: common_enums::CountryAlpha2,
    experience_context: ContextStruct,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContextStruct {
    return_url: Option<String>,
    cancel_url: Option<String>,
    user_action: Option<UserAction>,
    shipping_preference: ShippingPreference,
}

#[derive(Debug, Serialize)]
pub struct GooglePayRequest {
    pub decrypted_token: GooglePayDecryptedToken,
}

#[derive(Debug, Serialize)]
pub struct ApplePayRequest {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    pub decrypted_token: ApplePayDecryptedToken,
}

#[derive(Debug, Serialize)]
pub struct ApplePayDecryptedToken {
    pub tokenized_card: ApplePayTokenizedCard,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_data_type: Option<String>,
    pub payment_data: ApplePayPaymentDataRequest,
    pub transaction_amount: OrderAmount,
    // Required by PayPal for customer-initiated payments.
    // This is the deviceManufacturerIdentifier from Apple's decrypted token.
    // UCS domain_types does not currently carry this field through the gRPC layer,
    // so we use the well-known Apple Pay identifier. A future improvement would
    // thread the real value from ApplePayDecryptedData.
    pub device_manufacturer_id: String,
}

#[derive(Debug, Serialize)]
pub struct ApplePayTokenizedCard {
    pub number: Secret<String>,
    pub expiry: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct ApplePayPaymentDataRequest {
    pub cryptogram: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GooglePayPaymentMethod {
    Card,
}

#[derive(Debug, Serialize)]
pub struct GooglePayDecryptedToken {
    pub message_id: String,
    pub message_expiration: String,
    pub payment_method: GooglePayPaymentMethod,
    pub authentication_method: common_enums::GooglePayAuthMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
    pub card: GooglePayDecryptedCard,
}

#[derive(Debug, Serialize)]
pub struct GooglePayDecryptedCard {
    pub number: Secret<String>,
    pub expiry: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum UserAction {
    #[serde(rename = "PAY_NOW")]
    PayNow,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ShippingPreference {
    #[serde(rename = "SET_PROVIDED_ADDRESS")]
    SetProvidedAddress,
    #[serde(rename = "GET_FROM_FILE")]
    GetFromFile,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaypalRedirectionRequest {
    PaypalRedirectionStruct(PaypalRedirectionStruct),
    PaypalVaultStruct(VaultStruct),
}

#[derive(Debug, Serialize)]
pub struct PaypalRedirectionStruct {
    experience_context: ContextStruct,
    attributes: Option<Attributes>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attributes {
    vault: PaypalVault,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaypalRedirectionResponse {
    attributes: Option<AttributeResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpsRedirectionResponse {
    name: Option<Secret<String>>,
    country_code: Option<common_enums::CountryAlpha2>,
    bic: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdealRedirectionResponse {
    name: Option<Secret<String>>,
    country_code: Option<common_enums::CountryAlpha2>,
    bic: Option<Secret<String>>,
    iban_last_chars: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttributeResponse {
    vault: PaypalVaultResponse,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaypalVault {
    store_in_vault: StoreInVault,
    usage_type: UsageType,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaypalVaultResponse {
    id: String,
    status: String,
    customer: CustomerId,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomerId {
    id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoreInVault {
    OnSuccess,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UsageType {
    Merchant,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentSourceItem<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Card(CardRequest<T>),
    Paypal(PaypalRedirectionRequest),
    #[serde(rename = "google_pay")]
    GooglePay(GooglePayRequest),
    #[serde(rename = "apple_pay")]
    ApplePay(ApplePayRequest),
    IDeal(RedirectRequest),
    Eps(RedirectRequest),
    Giropay(RedirectRequest),
    Sofort(RedirectRequest),
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CardVaultResponse {
    attributes: Option<AttributeResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GooglePaySourceResponse {
    card: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApplePaySourceResponse {
    pub name: Option<Secret<String>>,
    pub card: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PaymentSourceItemResponse {
    Card(CardVaultResponse),
    Paypal(PaypalRedirectionResponse),
    Eps(EpsRedirectionResponse),
    Ideal(IdealRedirectionResponse),
    GooglePay(GooglePaySourceResponse),
    ApplePay(ApplePaySourceResponse),
}

// ============================================================================
// OrderAuthorize Request — used when authorizing an existing order (from CreateOrder)
// Only sends payment_source; intent and purchase_units were set during CreateOrder.
// ============================================================================

#[derive(Debug, Serialize)]
pub struct PaypalOrderAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    payment_source: PaymentSourceItem<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaypalOrderAuthorizeRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_source = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                let card = item.router_data.request.get_card()?;
                let expiry = Some(card.get_expiry_date_as_yyyymm("-"));

                let verification = match item.router_data.resource_common_data.auth_type {
                    common_enums::AuthenticationType::ThreeDs => Some(ThreeDsMethod {
                        method: ThreeDsType::ScaAlways,
                    }),
                    common_enums::AuthenticationType::NoThreeDs => None,
                };

                PaymentSourceItem::Card(CardRequest::CardRequestStruct(CardRequestStruct {
                    billing_address: get_address_info(
                        item.router_data
                            .resource_common_data
                            .get_optional_payment_billing(),
                    ),
                    expiry,
                    name: item
                        .router_data
                        .resource_common_data
                        .get_optional_payment_billing_full_name(),
                    number: Some(ccard.card_number.clone()),
                    security_code: Some(ccard.card_cvc.clone()),
                    attributes: Some(CardRequestAttributes {
                        vault: match item.router_data.request.setup_future_usage {
                            Some(common_enums::FutureUsage::OffSession) => Some(PaypalVault {
                                store_in_vault: StoreInVault::OnSuccess,
                                usage_type: UsageType::Merchant,
                            }),
                            _ => None,
                        },
                        verification,
                    }),
                }))
            }
            PaymentMethodData::Wallet(WalletData::PaypalRedirect(_)) => PaymentSourceItem::Paypal(
                PaypalRedirectionRequest::PaypalRedirectionStruct(PaypalRedirectionStruct {
                    experience_context: ContextStruct {
                        return_url: item.router_data.request.complete_authorize_url.clone(),
                        cancel_url: item.router_data.request.complete_authorize_url.clone(),
                        shipping_preference: if item
                            .router_data
                            .resource_common_data
                            .get_optional_shipping()
                            .is_some()
                        {
                            ShippingPreference::SetProvidedAddress
                        } else {
                            ShippingPreference::GetFromFile
                        },
                        user_action: Some(UserAction::PayNow),
                    },
                    attributes: match item.router_data.request.setup_future_usage {
                        Some(common_enums::FutureUsage::OffSession) => Some(Attributes {
                            vault: PaypalVault {
                                store_in_vault: StoreInVault::OnSuccess,
                                usage_type: UsageType::Merchant,
                            },
                        }),
                        _ => None,
                    },
                }),
            ),
            PaymentMethodData::Wallet(WalletData::ApplePay(ref apple_pay_data)) => {
                let amount_value = item
                    .connector
                    .amount_converter
                    .convert(
                        item.router_data.request.minor_amount,
                        item.router_data.request.currency,
                    )
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: Default::default(),
                    })?;
                build_paypal_apple_pay_source(
                    apple_pay_data,
                    &item.router_data,
                    amount_value,
                    item.router_data.request.currency,
                )?
            }
            PaymentMethodData::Wallet(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                    connector: "Paypal",
                    context: Default::default(),
                }))?
            }
            PaymentMethodData::BankRedirect(ref bank_redirection_data) => {
                get_payment_source(item.router_data.clone(), bank_redirection_data)?
            }
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                connector: "Paypal",
                context: Default::default(),
            }))?,
        };

        Ok(Self { payment_source })
    }
}

// ============================================================================
// CreateOrder Request/Response Types
// ============================================================================

/// CreateOrder request — creates a PayPal order without payment source.
/// The order_id returned is then used in the Authorize step.
#[derive(Debug, Serialize)]
pub struct PaypalOrderCreateRequest {
    intent: PaypalPaymentIntent,
    purchase_units: Vec<PaypalOrderCreatePurchaseUnit>,
}

/// Simplified purchase unit for CreateOrder (no payment-method-specific fields).
#[derive(Debug, Serialize)]
pub struct PaypalOrderCreatePurchaseUnit {
    reference_id: Option<String>,
    invoice_id: Option<String>,
    amount: PaypalOrderCreateAmount,
}

/// Amount block for CreateOrder (no breakdown required).
#[derive(Debug, Serialize)]
pub struct PaypalOrderCreateAmount {
    currency_code: common_enums::Currency,
    value: StringMajorUnit,
}

/// CreateOrder response — we only need the order id and status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalOrderCreateResponse {
    pub id: String,
    pub status: PaypalOrderStatus,
}

// --- TryFrom: RouterDataV2 -> PaypalOrderCreateRequest (via macro wrapper) ---

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for PaypalOrderCreateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PaypalRouterData<
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

        let value = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let connector_request_reference_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let purchase_units = vec![PaypalOrderCreatePurchaseUnit {
            reference_id: Some(connector_request_reference_id.clone()),
            invoice_id: Some(connector_request_reference_id),
            amount: PaypalOrderCreateAmount {
                currency_code: router_data.request.currency,
                value,
            },
        }];

        // Default to Authorize intent so we can capture later
        let intent = PaypalPaymentIntent::Authorize;

        Ok(Self {
            intent,
            purchase_units,
        })
    }
}

// --- TryFrom: PaypalOrderCreateResponse -> PaymentCreateOrderResponse ---

impl TryFrom<PaypalOrderCreateResponse> for PaymentCreateOrderResponse {
    type Error = Report<ConnectorError>;

    fn try_from(response: PaypalOrderCreateResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            connector_order_id: response.id,
            session_data: None,
        })
    }
}

// --- TryFrom: ResponseRouterData -> RouterDataV2 (CreateOrder response handler) ---

impl TryFrom<ResponseRouterData<PaypalOrderCreateResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaypalOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let order_response = PaymentCreateOrderResponse::try_from(response.clone())?;

        // Extract order_id before moving
        let order_id = order_response.connector_order_id.clone();

        Ok(Self {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::Pending,
                // KEY: Store order ID so Authorize flow can use it via connector_order_id
                connector_order_id: Some(order_id),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================================

#[derive(Debug, Serialize)]
pub struct PaypalPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    intent: PaypalPaymentIntent,
    purchase_units: Vec<PurchaseUnitRequest>,
    payment_source: Option<PaymentSourceItem<T>>,
}

#[derive(Debug, Serialize)]
pub struct PaypalZeroMandateRequest {
    payment_source: ZeroMandateSourceItem,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ZeroMandateSourceItem {
    Card(CardMandateRequest),
    Paypal(PaypalMandateStruct),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalMandateStruct {
    experience_context: Option<ContextStruct>,
    usage_type: UsageType,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CardMandateRequest {
    billing_address: Option<Address>,
    expiry: Option<Secret<String>>,
    name: Option<Secret<String>>,
    number: Option<cards::CardNumber>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalSetupMandatesResponse {
    id: String,
    customer: Customer,
    payment_source: ZeroMandateSourceItem,
    links: Vec<PaypalLinks>,
}

// RepeatPayment - reuses Authorize request/response types
pub type PaypalRepeatPaymentRequest<T> = PaypalPaymentsRequest<T>;
pub type PaypalRepeatPaymentResponse = PaypalAuthResponse;

// Response handling for RepeatPayment - delegates to PaypalOrdersResponse
impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<PaypalAuthResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaypalAuthResponse, Self>) -> Result<Self, Self::Error> {
        // RepeatPayment returns PaypalOrdersResponse variant (direct capture)
        match item.response {
            PaypalAuthResponse::PaypalOrdersResponse(orders_response) => {
                Self::try_from(ResponseRouterData {
                    response: orders_response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            PaypalAuthResponse::PaypalRedirectResponse(_)
            | PaypalAuthResponse::PaypalThreeDsResponse(_) => Err(
                crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
            )?,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Customer {
    id: String,
}

// Apple Pay (decrypted) for PayPal.
//
// Uses PayPal's dedicated `payment_source.apple_pay.decrypted_token` structure
// (https://developer.paypal.com/docs/api/orders/v2/#definition-apple_pay_decrypted_token_data)
// which carries the DPAN, expiry, cryptogram, and ECI indicator — preserving
// the 3DS liability shift and correct interchange treatment.
// Encrypted-only Apple Pay tokens are not supported and will return MissingRequiredField.
fn build_paypal_apple_pay_source<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    apple_pay_data: &domain_types::payment_method_data::ApplePayWalletData,
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    amount_value: StringMajorUnit,
    currency: common_enums::Currency,
) -> Result<PaymentSourceItem<T>, Report<IntegrationError>> {
    let apple_pay_decrypted_data = apple_pay_data
        .payment_data
        .get_decrypted_apple_pay_payment_data_optional()
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "apple_pay_decrypted_data",
                context: Default::default(),
            })
            .attach_printable(
                "PayPal requires pre-decrypted Apple Pay data; \
                 encrypted Apple Pay tokens are not supported.",
            )
        })?;

    // PayPal decrypted_token expiry format is YYYY-MM.
    let expiry = apple_pay_decrypted_data.get_expiry_date_as_yyyymm("-");

    let card_number = Secret::new(
        apple_pay_decrypted_data
            .application_primary_account_number
            .get_card_no(),
    );

    let cardholder_name = router_data
        .resource_common_data
        .get_optional_payment_billing_full_name();

    let cryptogram = apple_pay_decrypted_data
        .payment_data
        .online_payment_cryptogram
        .clone();
    let eci_indicator = apple_pay_decrypted_data.payment_data.eci_indicator.clone();

    Ok(PaymentSourceItem::ApplePay(ApplePayRequest {
        id: apple_pay_data.transaction_identifier.clone(),
        name: cardholder_name.clone(),
        decrypted_token: ApplePayDecryptedToken {
            tokenized_card: ApplePayTokenizedCard {
                number: card_number,
                expiry,
                name: cardholder_name,
            },
            // PayPal supports two payment_data_type values: "3DSECURE" (cryptogram + ECI,
            // standard outside China) and "EMV" (emv_data + pin, China only).
            // UCS exclusively receives pre-decrypted Apple Pay tokens over gRPC, which
            // always carry online_payment_cryptogram + eci_indicator — the 3DS Secure
            // path. The EMV/China path is not supported by UCS.
            payment_data_type: Some("3DSECURE".to_string()),
            payment_data: ApplePayPaymentDataRequest {
                cryptogram,
                eci_indicator,
            },
            transaction_amount: OrderAmount {
                currency_code: currency,
                value: amount_value,
            },
            // PayPal requires device_manufacturer_id for Apple Pay — omitting it returns
            // REQUIRED_PARAMETER_FOR_CUSTOMER_INITIATED_PAYMENT (HTTP 422).
            // "040010030273" is Apple's standard device manufacturer identifier and is used
            // as a fallback when the field is absent from the decrypted token.
            device_manufacturer_id: apple_pay_decrypted_data
                .device_manufacturer_identifier
                .clone()
                .unwrap_or_else(|| {
                    tracing::warn!(
                        "device_manufacturer_identifier missing from Apple Pay decrypted token; \
                        falling back to Apple standard identifier '040010030273'"
                    );
                    "040010030273".to_string()
                }),
        },
    }))
}

fn get_address_info(
    payment_address: Option<&domain_types::payment_address::Address>,
) -> Option<Address> {
    let address = payment_address.and_then(|payment_address| payment_address.address.as_ref());
    match address {
        Some(address) => address.get_optional_country().map(|country| Address {
            country_code: country.to_owned(),
            address_line_1: address.line1.clone(),
            postal_code: address.zip.clone(),
            admin_area_2: address.city.clone(),
        }),
        None => None,
    }
}
fn get_payment_source<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    bank_redirection_data: &BankRedirectData,
) -> Result<PaymentSourceItem<T>, Report<IntegrationError>> {
    match bank_redirection_data {
        BankRedirectData::Eps { bank_name: _, .. } => Ok(PaymentSourceItem::Eps(RedirectRequest {
            name: item.resource_common_data.get_billing_full_name()?,
            country_code: item.resource_common_data.get_billing_country()?,
            experience_context: ContextStruct {
                return_url: item.request.complete_authorize_url.clone(),
                cancel_url: item.request.complete_authorize_url.clone(),
                shipping_preference: if item
                    .resource_common_data
                    .get_optional_shipping_country()
                    .is_some()
                {
                    ShippingPreference::SetProvidedAddress
                } else {
                    ShippingPreference::GetFromFile
                },
                user_action: Some(UserAction::PayNow),
            },
        })),
        BankRedirectData::Giropay { .. } => Ok(PaymentSourceItem::Giropay(RedirectRequest {
            name: item.resource_common_data.get_billing_full_name()?,
            country_code: item.resource_common_data.get_billing_country()?,
            experience_context: ContextStruct {
                return_url: item.request.complete_authorize_url.clone(),
                cancel_url: item.request.complete_authorize_url.clone(),
                shipping_preference: if item
                    .resource_common_data
                    .get_optional_shipping_country()
                    .is_some()
                {
                    ShippingPreference::SetProvidedAddress
                } else {
                    ShippingPreference::GetFromFile
                },
                user_action: Some(UserAction::PayNow),
            },
        })),
        BankRedirectData::Ideal { bank_name: _, .. } => {
            Ok(PaymentSourceItem::IDeal(RedirectRequest {
                name: item.resource_common_data.get_billing_full_name()?,
                country_code: item.resource_common_data.get_billing_country()?,
                experience_context: ContextStruct {
                    return_url: item.request.complete_authorize_url.clone(),
                    cancel_url: item.request.complete_authorize_url.clone(),
                    shipping_preference: if item
                        .resource_common_data
                        .get_optional_shipping_country()
                        .is_some()
                    {
                        ShippingPreference::SetProvidedAddress
                    } else {
                        ShippingPreference::GetFromFile
                    },
                    user_action: Some(UserAction::PayNow),
                },
            }))
        }
        BankRedirectData::Sofort {
            preferred_language: _,
            ..
        } => Ok(PaymentSourceItem::Sofort(RedirectRequest {
            name: item.resource_common_data.get_billing_full_name()?,
            country_code: item.resource_common_data.get_billing_country()?,
            experience_context: ContextStruct {
                return_url: item.request.complete_authorize_url.clone(),
                cancel_url: item.request.complete_authorize_url.clone(),
                shipping_preference: if item
                    .resource_common_data
                    .get_optional_shipping_country()
                    .is_some()
                {
                    ShippingPreference::SetProvidedAddress
                } else {
                    ShippingPreference::GetFromFile
                },
                user_action: Some(UserAction::PayNow),
            },
        })),
        BankRedirectData::BancontactCard { .. }
        | BankRedirectData::Blik { .. }
        | BankRedirectData::Przelewy24 { .. } => Err(IntegrationError::NotImplemented(
            utils::get_unimplemented_payment_method_error_message("Paypal"),
            Default::default(),
        )
        .into()),
        BankRedirectData::Bizum {}
        | BankRedirectData::Eft { .. }
        | BankRedirectData::Interac { .. }
        | BankRedirectData::OnlineBankingCzechRepublic { .. }
        | BankRedirectData::OnlineBankingFinland { .. }
        | BankRedirectData::OnlineBankingPoland { .. }
        | BankRedirectData::OnlineBankingSlovakia { .. }
        | BankRedirectData::OpenBankingUk { .. }
        | BankRedirectData::Trustly { .. }
        | BankRedirectData::OnlineBankingFpx { .. }
        | BankRedirectData::OnlineBankingThailand { .. }
        | BankRedirectData::LocalBankRedirect {}
        | BankRedirectData::OpenBanking {}
        | BankRedirectData::Netbanking { .. } => Err(IntegrationError::NotImplemented(
            utils::get_unimplemented_payment_method_error_message("Paypal"),
            Default::default(),
        ))?,
    }
}

fn get_payee(auth_type: &PaypalAuthType) -> Option<Payee> {
    auth_type
        .get_credentials()
        .ok()
        .and_then(|credentials| credentials.get_payer_id())
        .map(|payer_id| Payee {
            merchant_id: payer_id,
        })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let paypal_auth: PaypalAuthType =
            PaypalAuthType::try_from(&item.router_data.connector_config)?;
        let payee = get_payee(&paypal_auth);

        let amount = OrderRequestAmount::try_from(&item)?;

        let intent = if item.router_data.request.is_auto_capture() {
            PaypalPaymentIntent::Capture
        } else {
            PaypalPaymentIntent::Authorize
        };

        let connector_request_reference_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let shipping_address = ShippingAddress::from(&item);
        let item_details = vec![ItemDetails::try_from(&item)?];

        let purchase_units = vec![PurchaseUnitRequest {
            reference_id: Some(connector_request_reference_id.clone()),
            custom_id: item.router_data.request.merchant_order_id.clone(),
            invoice_id: Some(connector_request_reference_id),
            amount,
            payee,
            shipping: Some(shipping_address),
            items: item_details,
        }];

        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                let card = item.router_data.request.get_card()?;
                let expiry = Some(card.get_expiry_date_as_yyyymm("-"));

                let verification = match item.router_data.resource_common_data.auth_type {
                    common_enums::AuthenticationType::ThreeDs => Some(ThreeDsMethod {
                        method: ThreeDsType::ScaAlways,
                    }),
                    common_enums::AuthenticationType::NoThreeDs => None,
                };

                let payment_source = Some(PaymentSourceItem::Card(CardRequest::CardRequestStruct(
                    CardRequestStruct {
                        billing_address: get_address_info(
                            item.router_data
                                .resource_common_data
                                .get_optional_payment_billing(),
                        ),
                        expiry,
                        name: item
                            .router_data
                            .resource_common_data
                            .get_optional_payment_billing_full_name(),
                        number: Some(ccard.card_number.clone()),
                        security_code: Some(ccard.card_cvc.clone()),
                        attributes: Some(CardRequestAttributes {
                            vault: match item.router_data.request.setup_future_usage {
                                Some(setup_future_usage) => match setup_future_usage {
                                    common_enums::FutureUsage::OffSession => Some(PaypalVault {
                                        store_in_vault: StoreInVault::OnSuccess,
                                        usage_type: UsageType::Merchant,
                                    }),

                                    common_enums::FutureUsage::OnSession => None,
                                },
                                None => None,
                            },
                            verification,
                        }),
                    },
                )));

                Ok(Self {
                    intent,
                    purchase_units,
                    payment_source,
                })
            }
            PaymentMethodData::Wallet(ref wallet_data) => match wallet_data {
                WalletData::PaypalRedirect(_) => {
                    let payment_source = Some(PaymentSourceItem::Paypal(
                        PaypalRedirectionRequest::PaypalRedirectionStruct(
                            PaypalRedirectionStruct {
                                experience_context: ContextStruct {
                                    return_url: item
                                        .router_data
                                        .request
                                        .complete_authorize_url
                                        .clone(),
                                    cancel_url: item
                                        .router_data
                                        .request
                                        .complete_authorize_url
                                        .clone(),
                                    shipping_preference: if item
                                        .router_data
                                        .resource_common_data
                                        .get_optional_shipping()
                                        .is_some()
                                    {
                                        ShippingPreference::SetProvidedAddress
                                    } else {
                                        ShippingPreference::GetFromFile
                                    },
                                    user_action: Some(UserAction::PayNow),
                                },
                                attributes: match item.router_data.request.setup_future_usage {
                                    Some(setup_future_usage) => match setup_future_usage {
                                        common_enums::FutureUsage::OffSession => Some(Attributes {
                                            vault: PaypalVault {
                                                store_in_vault: StoreInVault::OnSuccess,
                                                usage_type: UsageType::Merchant,
                                            },
                                        }),
                                        common_enums::FutureUsage::OnSession => None,
                                    },
                                    None => None,
                                },
                            },
                        ),
                    ));

                    Ok(Self {
                        intent,
                        purchase_units,
                        payment_source,
                    })
                }
                WalletData::PaypalSdk(_) => {
                    let payment_source = Some(PaymentSourceItem::Paypal(
                        PaypalRedirectionRequest::PaypalRedirectionStruct(
                            PaypalRedirectionStruct {
                                experience_context: ContextStruct {
                                    return_url: None,
                                    cancel_url: None,
                                    shipping_preference: ShippingPreference::GetFromFile,
                                    user_action: Some(UserAction::PayNow),
                                },
                                attributes: match item.router_data.request.setup_future_usage {
                                    Some(setup_future_usage) => match setup_future_usage {
                                        common_enums::FutureUsage::OffSession => Some(Attributes {
                                            vault: PaypalVault {
                                                store_in_vault: StoreInVault::OnSuccess,
                                                usage_type: UsageType::Merchant,
                                            },
                                        }),
                                        common_enums::FutureUsage::OnSession => None,
                                    },
                                    None => None,
                                },
                            },
                        ),
                    ));

                    Ok(Self {
                        intent,
                        purchase_units,
                        payment_source,
                    })
                }
                WalletData::GooglePay(gpay_data) => match &gpay_data.tokenization_data {
                    GpayTokenizationData::Decrypted(decrypted_data) => {
                        let expiry = decrypted_data
                            .get_expiry_date_as_yyyymm("-")
                            .change_context(IntegrationError::InvalidWalletToken {
                                wallet_name: "Google Pay".to_string(),
                                context: Default::default(),
                            })?;
                        let authentication_method = if decrypted_data.cryptogram.is_some() {
                            common_enums::GooglePayAuthMethod::Cryptogram
                        } else {
                            common_enums::GooglePayAuthMethod::PanOnly
                        };
                        let payment_source = Some(PaymentSourceItem::GooglePay(GooglePayRequest {
                            decrypted_token: GooglePayDecryptedToken {
                                message_id: uuid::Uuid::new_v4().to_string(),
                                // TODO: message_expiration is hardcoded because HS does not currently
                                // forward this field from the decrypted GPay payload through the gRPC
                                // interface. Tracked in https://github.com/juspay/hyperswitch/issues/11684
                                message_expiration: "9999999999999".to_string(),
                                payment_method: GooglePayPaymentMethod::Card,
                                authentication_method,
                                cryptogram: decrypted_data.cryptogram.clone(),
                                eci_indicator: decrypted_data.eci_indicator.clone(),
                                card: GooglePayDecryptedCard {
                                    number: Secret::new(
                                        decrypted_data
                                            .application_primary_account_number
                                            .get_card_no(),
                                    ),
                                    expiry,
                                },
                            },
                        }));
                        Ok(Self {
                            intent,
                            purchase_units,
                            payment_source,
                        })
                    }
                    GpayTokenizationData::Encrypted(_) => Err(IntegrationError::NotImplemented(
                        "PayPal GooglePay encrypted flow".to_string(),
                        Default::default(),
                    )
                    .into()),
                },
                WalletData::ApplePay(apple_pay_data) => {
                    let amount_value = item
                        .connector
                        .amount_converter
                        .convert(
                            item.router_data.request.minor_amount,
                            item.router_data.request.currency,
                        )
                        .change_context(IntegrationError::AmountConversionFailed {
                            context: Default::default(),
                        })?;
                    let payment_source = Some(build_paypal_apple_pay_source(
                        apple_pay_data,
                        &item.router_data,
                        amount_value,
                        item.router_data.request.currency,
                    )?);
                    Ok(Self {
                        intent,
                        purchase_units,
                        payment_source,
                    })
                }
                WalletData::AliPayQr(_)
                | WalletData::AliPayRedirect(_)
                | WalletData::AliPayHkRedirect(_)
                | WalletData::AmazonPayRedirect(_)
                | WalletData::MomoRedirect(_)
                | WalletData::KakaoPayRedirect(_)
                | WalletData::GoPayRedirect(_)
                | WalletData::GcashRedirect(_)
                | WalletData::ApplePayRedirect(_)
                | WalletData::ApplePayThirdPartySdk(_)
                | WalletData::DanaRedirect {}
                | WalletData::BluecodeRedirect {}
                | WalletData::GooglePayRedirect(_)
                | WalletData::GooglePayThirdPartySdk(_)
                | WalletData::MbWayRedirect(_)
                | WalletData::MobilePayRedirect(_)
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
                | WalletData::Paze(_)
                | WalletData::MbWay(_)
                | WalletData::Satispay(_)
                | WalletData::Wero(_)
                | WalletData::LazyPayRedirect(_)
                | WalletData::PhonePeRedirect(_)
                | WalletData::BillDeskRedirect(_)
                | WalletData::CashfreeRedirect(_)
                | WalletData::PayURedirect(_)
                | WalletData::EaseBuzzRedirect(_) => {
                    Err(error_stack::report!(IntegrationError::NotSupported {
                        message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                        connector: "Paypal",
                        context: Default::default(),
                    }))?
                }
            },
            PaymentMethodData::BankRedirect(ref bank_redirection_data) => {
                let payment_source = Some(get_payment_source(
                    item.router_data.clone(),
                    bank_redirection_data,
                )?);

                let bank_redirect_intent = if item.router_data.request.is_auto_capture() {
                    PaypalPaymentIntent::Capture
                } else {
                    Err(IntegrationError::FlowNotSupported {
                        flow: "Manual capture method for Bank Redirect".to_string(),
                        connector: "Paypal".to_string(),
                        context: Default::default(),
                    })?
                };

                Ok(Self {
                    intent: bank_redirect_intent,
                    purchase_units,
                    payment_source,
                })
            }
            PaymentMethodData::CardRedirect(ref card_redirect_data) => {
                Self::try_from(card_redirect_data)
            }
            PaymentMethodData::PayLater(ref paylater_data) => Self::try_from(paylater_data),
            PaymentMethodData::BankDebit(ref bank_debit_data) => Self::try_from(bank_debit_data),
            PaymentMethodData::BankTransfer(ref bank_transfer_data) => {
                Self::try_from(bank_transfer_data.as_ref())
            }
            PaymentMethodData::Voucher(ref voucher_data) => Self::try_from(voucher_data),
            PaymentMethodData::GiftCard(ref giftcard_data) => {
                Self::try_from(giftcard_data.as_ref())
            }
            PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                    connector: "Paypal",
                    context: Default::default(),
                }))
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&CardRedirectData> for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(value: &CardRedirectData) -> Result<Self, Self::Error> {
        match value {
            CardRedirectData::Knet {}
            | CardRedirectData::Benefit {}
            | CardRedirectData::MomoAtm {}
            | CardRedirectData::CardRedirect {} => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                    connector: "Paypal",
                    context: Default::default(),
                }))
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&PayLaterData> for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(value: &PayLaterData) -> Result<Self, Self::Error> {
        match value {
            PayLaterData::KlarnaRedirect { .. }
            | PayLaterData::KlarnaSdk { .. }
            | PayLaterData::AffirmRedirect {}
            | PayLaterData::AfterpayClearpayRedirect { .. }
            | PayLaterData::PayBrightRedirect {}
            | PayLaterData::WalleyRedirect {}
            | PayLaterData::AlmaRedirect {}
            | PayLaterData::AtomeRedirect {} => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                    connector: "Paypal",
                    context: Default::default(),
                }))
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&BankDebitData> for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(value: &BankDebitData) -> Result<Self, Self::Error> {
        match value {
            BankDebitData::AchBankDebit { .. }
            | BankDebitData::SepaBankDebit { .. }
            | BankDebitData::EftBankDebit { .. }
            | BankDebitData::SepaGuaranteedBankDebit { .. }
            | BankDebitData::BecsBankDebit { .. }
            | BankDebitData::BacsBankDebit { .. } => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                    connector: "Paypal",
                    context: Default::default(),
                }))
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&BankTransferData> for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(value: &BankTransferData) -> Result<Self, Self::Error> {
        match value {
            BankTransferData::AchBankTransfer { .. }
            | BankTransferData::SepaBankTransfer { .. }
            | BankTransferData::BacsBankTransfer { .. }
            | BankTransferData::MultibancoBankTransfer { .. }
            | BankTransferData::PermataBankTransfer { .. }
            | BankTransferData::BcaBankTransfer { .. }
            | BankTransferData::BniVaBankTransfer { .. }
            | BankTransferData::BriVaBankTransfer { .. }
            | BankTransferData::CimbVaBankTransfer { .. }
            | BankTransferData::DanamonVaBankTransfer { .. }
            | BankTransferData::MandiriVaBankTransfer { .. }
            | BankTransferData::Pix { .. }
            | BankTransferData::Pse {}
            | BankTransferData::InstantBankTransfer {}
            | BankTransferData::InstantBankTransferFinland {}
            | BankTransferData::InstantBankTransferPoland {}
            | BankTransferData::IndonesianBankTransfer { .. }
            | BankTransferData::LocalBankTransfer { .. } => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Paypal"),
                Default::default(),
            )
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&VoucherData> for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(value: &VoucherData) -> Result<Self, Self::Error> {
        match value {
            VoucherData::Boleto(_)
            | VoucherData::Efecty
            | VoucherData::PagoEfectivo
            | VoucherData::RedCompra
            | VoucherData::RedPagos
            | VoucherData::Alfamart(_)
            | VoucherData::Indomaret(_)
            | VoucherData::Oxxo
            | VoucherData::SevenEleven(_)
            | VoucherData::Lawson(_)
            | VoucherData::MiniStop(_)
            | VoucherData::FamilyMart(_)
            | VoucherData::Seicomart(_)
            | VoucherData::PayEasy(_) => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Paypal"),
                Default::default(),
            )
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&GiftCardData> for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(value: &GiftCardData) -> Result<Self, Self::Error> {
        match value {
            GiftCardData::Givex(_) | GiftCardData::PaySafeCard {} => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                    connector: "Paypal",
                    context: Default::default(),
                }))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PaypalAuthUpdateRequest {
    grant_type: String,
    client_id: Secret<String>,
    client_secret: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerAuthenticationToken,
                PaymentFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for PaypalAuthUpdateRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerAuthenticationToken,
                PaymentFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PaypalAuthType::try_from(&item.router_data.connector_config)?;
        let credentials = auth.get_credentials()?;
        Ok(Self {
            grant_type: item.router_data.request.grant_type,
            client_id: credentials.get_client_id(),
            client_secret: credentials.get_client_secret(),
        })
    }
}

#[derive(Default, Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct PaypalAuthUpdateResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
}

impl<F, T> TryFrom<ResponseRouterData<PaypalAuthUpdateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in),
                token_type: None,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalIncrementalStatus {
    CREATED,
    CAPTURED,
    DENIED,
    PARTIALLYCAPTURED,
    VOIDED,
    PENDING,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalNetworkTransactionReference {
    id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalIncrementalAuthStatusDetails {
    reason: Option<PaypalStatusPendingReason>,
}

#[derive(Debug, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalStatusPendingReason {
    PENDINGREVIEW,
    DECLINEDBYRISKFRAUDFILTERS,
}

impl From<PaypalIncrementalStatus> for common_enums::AuthorizationStatus {
    fn from(item: PaypalIncrementalStatus) -> Self {
        match item {
            PaypalIncrementalStatus::CREATED
            | PaypalIncrementalStatus::CAPTURED
            | PaypalIncrementalStatus::PARTIALLYCAPTURED => Self::Success,
            PaypalIncrementalStatus::PENDING => Self::Processing,
            PaypalIncrementalStatus::DENIED | PaypalIncrementalStatus::VOIDED => Self::Failure,
        }
    }
}

impl From<PaypalIncrementalStatus> for common_enums::AttemptStatus {
    fn from(item: PaypalIncrementalStatus) -> Self {
        match item {
            PaypalIncrementalStatus::CREATED
            | PaypalIncrementalStatus::CAPTURED
            | PaypalIncrementalStatus::PARTIALLYCAPTURED => Self::Authorized,
            PaypalIncrementalStatus::PENDING => Self::Pending,
            PaypalIncrementalStatus::DENIED | PaypalIncrementalStatus::VOIDED => Self::Failure,
        }
    }
}

#[derive(Debug)]
pub enum PaypalAuthType {
    TemporaryAuth,
    AuthWithDetails(PaypalConnectorCredentials),
}

#[derive(Debug)]
pub enum PaypalConnectorCredentials {
    StandardIntegration(StandardFlowCredentials),
    PartnerIntegration(PartnerFlowCredentials),
}

impl PaypalConnectorCredentials {
    pub fn get_client_id(&self) -> Secret<String> {
        match self {
            Self::StandardIntegration(item) => item.client_id.clone(),
            Self::PartnerIntegration(item) => item.client_id.clone(),
        }
    }

    pub fn get_client_secret(&self) -> Secret<String> {
        match self {
            Self::StandardIntegration(item) => item.client_secret.clone(),
            Self::PartnerIntegration(item) => item.client_secret.clone(),
        }
    }

    pub fn get_payer_id(&self) -> Option<Secret<String>> {
        match self {
            Self::StandardIntegration(_) => None,
            Self::PartnerIntegration(item) => Some(item.payer_id.clone()),
        }
    }

    pub fn generate_authorization_value(&self) -> String {
        let auth_id = format!(
            "{}:{}",
            self.get_client_id().expose(),
            self.get_client_secret().expose(),
        );
        format!("Basic {}", BASE64_ENGINE.encode(auth_id))
    }
}

#[derive(Debug)]
pub struct StandardFlowCredentials {
    pub(super) client_id: Secret<String>,
    pub(super) client_secret: Secret<String>,
}

#[derive(Debug)]
pub struct PartnerFlowCredentials {
    pub(super) client_id: Secret<String>,
    pub(super) client_secret: Secret<String>,
    pub(super) payer_id: Secret<String>,
}

impl PaypalAuthType {
    pub fn get_credentials(&self) -> CustomResult<&PaypalConnectorCredentials, IntegrationError> {
        match self {
            Self::TemporaryAuth => Err(IntegrationError::InvalidConnectorConfig {
                config: "TemporaryAuth found in connector_account_details",
                context: Default::default(),
            }
            .into()),
            Self::AuthWithDetails(credentials) => Ok(credentials),
        }
    }
}

impl TryFrom<&ConnectorSpecificConfig> for PaypalAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paypal {
                client_id,
                client_secret,
                payer_id,
                ..
            } => match payer_id {
                None => Ok(Self::AuthWithDetails(
                    PaypalConnectorCredentials::StandardIntegration(StandardFlowCredentials {
                        client_id: client_id.to_owned(),
                        client_secret: client_secret.to_owned(),
                    }),
                )),
                Some(payer_id) => Ok(Self::AuthWithDetails(
                    PaypalConnectorCredentials::PartnerIntegration(PartnerFlowCredentials {
                        client_id: client_id.to_owned(),
                        client_secret: client_secret.to_owned(),
                        payer_id: payer_id.to_owned(),
                    }),
                )),
            },
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalOrderStatus {
    Pending,
    Completed,
    Voided,
    Created,
    Saved,
    PayerActionRequired,
    Approved,
}

pub(crate) fn get_order_status(
    item: PaypalOrderStatus,
    intent: PaypalPaymentIntent,
) -> common_enums::AttemptStatus {
    match item {
        PaypalOrderStatus::Completed => {
            if intent == PaypalPaymentIntent::Authorize {
                common_enums::AttemptStatus::Authorized
            } else {
                common_enums::AttemptStatus::Charged
            }
        }
        PaypalOrderStatus::Voided => common_enums::AttemptStatus::Voided,
        PaypalOrderStatus::Created | PaypalOrderStatus::Saved | PaypalOrderStatus::Pending => {
            common_enums::AttemptStatus::Pending
        }
        PaypalOrderStatus::Approved => common_enums::AttemptStatus::AuthenticationSuccessful,
        PaypalOrderStatus::PayerActionRequired => {
            common_enums::AttemptStatus::AuthenticationPending
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentsCollectionItem {
    amount: OrderAmount,
    expiration_time: Option<String>,
    id: String,
    final_capture: Option<bool>,
    status: PaypalPaymentStatus,
    processor_response: Option<ProcessorResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorResponse {
    avs_code: Option<String>,
    cvv_code: Option<String>,
    response_code: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PaymentsCollection {
    authorizations: Option<Vec<PaymentsCollectionItem>>,
    captures: Option<Vec<PaymentsCollectionItem>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseUnitItem {
    pub reference_id: Option<String>,
    pub invoice_id: Option<String>,
    pub payments: PaymentsCollection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalThreeDsResponse {
    id: String,
    status: PaypalOrderStatus,
    links: Vec<PaypalLinks>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PaypalPostAuthenticateResponse {
    PaypalLiabilityResponse(PaypalLiabilityResponse),
    PaypalNonLiabilityResponse(PaypalNonLiabilityResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalLiabilityResponse {
    pub payment_source: CardParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalNonLiabilityResponse {
    payment_source: CardsData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardParams {
    pub card: AuthResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub authentication_result: PaypalThreeDsParams,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalThreeDsParams {
    pub liability_shift: LiabilityShift,
    pub three_d_secure: ThreeDsCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeDsCheck {
    pub enrollment_status: Option<EnrollmentStatus>,
    pub authentication_status: Option<AuthenticationStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LiabilityShift {
    Possible,
    No,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnrollmentStatus {
    Null,
    #[serde(rename = "Y")]
    Ready,
    #[serde(rename = "N")]
    NotReady,
    #[serde(rename = "U")]
    Unavailable,
    #[serde(rename = "B")]
    Bypassed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthenticationStatus {
    Null,
    #[serde(rename = "Y")]
    Success,
    #[serde(rename = "N")]
    Failed,
    #[serde(rename = "R")]
    Rejected,
    #[serde(rename = "A")]
    Attempted,
    #[serde(rename = "U")]
    Unable,
    #[serde(rename = "C")]
    ChallengeRequired,
    #[serde(rename = "I")]
    InfoOnly,
    #[serde(rename = "D")]
    Decoupled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalOrdersResponse {
    id: String,
    intent: PaypalPaymentIntent,
    status: PaypalOrderStatus,
    purchase_units: Vec<PurchaseUnitItem>,
    payment_source: Option<PaymentSourceItemResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalLinks {
    href: Option<Url>,
    rel: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RedirectPurchaseUnitItem {
    pub invoice_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalRedirectResponse {
    id: String,
    intent: PaypalPaymentIntent,
    status: PaypalOrderStatus,
    purchase_units: Vec<RedirectPurchaseUnitItem>,
    links: Vec<PaypalLinks>,
    payment_source: Option<PaymentSourceItemResponse>,
}

// Note: Don't change order of deserialization of variant, priority is in descending order
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaypalAuthResponse {
    PaypalOrdersResponse(PaypalOrdersResponse),
    PaypalRedirectResponse(PaypalRedirectResponse),
    PaypalThreeDsResponse(PaypalThreeDsResponse),
}

// Note: Don't change order of deserialization of variant, priority is in descending order
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaypalSyncResponse {
    PaypalOrdersSyncResponse(PaypalOrdersResponse),
    PaypalThreeDsSyncResponse(PaypalThreeDsSyncResponse),
    PaypalRedirectSyncResponse(PaypalRedirectResponse),
    PaypalPaymentsSyncResponse(PaypalPaymentsSyncResponse),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextActionCall {
    CompleteAuthorize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalPaymentsSyncResponse {
    id: String,
    status: PaypalPaymentStatus,
    amount: OrderAmount,
    invoice_id: Option<String>,
    supplementary_data: PaypalSupplementaryData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalThreeDsSyncResponse {
    id: String,
    status: PaypalOrderStatus,
    // provided to separated response of card's 3DS from other
    payment_source: CardsData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardsData {
    card: CardDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDetails {
    last_digits: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalMeta {
    pub authorize_id: Option<String>,
    pub capture_id: Option<String>,
    pub incremental_authorization_id: Option<String>,
    pub psync_flow: PaypalPaymentIntent,
    pub next_action: Option<NextActionCall>,
    pub order_id: Option<String>,
}

fn get_id_based_on_intent(
    intent: &PaypalPaymentIntent,
    purchase_unit: &PurchaseUnitItem,
    http_status: u16,
) -> Result<String, Report<ConnectorError>> {
    let id = || -> Option<String> {
        match intent {
            PaypalPaymentIntent::Capture => Some(
                purchase_unit
                    .payments
                    .captures
                    .clone()?
                    .into_iter()
                    .next()?
                    .id,
            ),
            PaypalPaymentIntent::Authorize => Some(
                purchase_unit
                    .payments
                    .authorizations
                    .clone()?
                    .into_iter()
                    .next()?
                    .id,
            ),
            PaypalPaymentIntent::Authenticate => None,
        }
    }();
    id.ok_or_else(|| {
        Report::new(ConnectorError::response_handling_failed_with_context(
            http_status,
            Some("missing capture or authorization id for PayPal order intent".to_string()),
        ))
    })
}

fn extract_incremental_authorization_id(response: &PaypalOrdersResponse) -> Option<String> {
    for unit in &response.purchase_units {
        if let Some(authorizations) = &unit.payments.authorizations {
            if let Some(first_auth) = authorizations.first() {
                return Some(first_auth.id.clone());
            }
        }
    }
    None
}

impl<F, Req> TryFrom<ResponseRouterData<PaypalOrdersResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
where
    Req: GetRequestIncrementalAuthorization,
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaypalOrdersResponse, Self>) -> Result<Self, Self::Error> {
        let purchase_units = item.response.purchase_units.first().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("missing purchase_units in PayPal order response".to_string()),
            ))
        })?;

        let id = get_id_based_on_intent(&item.response.intent, purchase_units, item.http_code)?;
        let (connector_meta, order_id) = match item.response.intent.clone() {
            PaypalPaymentIntent::Capture => (
                serde_json::json!(PaypalMeta {
                    authorize_id: None,
                    capture_id: Some(id),
                    incremental_authorization_id: None,
                    psync_flow: item.response.intent.clone(),
                    next_action: None,
                    order_id: None
                }),
                ResponseId::ConnectorTransactionId(item.response.id.clone()),
            ),

            PaypalPaymentIntent::Authorize => (
                serde_json::json!(PaypalMeta {
                    authorize_id: Some(id),
                    capture_id: None,
                    incremental_authorization_id: extract_incremental_authorization_id(
                        &item.response
                    ),
                    psync_flow: item.response.intent.clone(),
                    next_action: None,
                    order_id: None
                }),
                ResponseId::ConnectorTransactionId(item.response.id.clone()),
            ),

            PaypalPaymentIntent::Authenticate => Err(
                crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
            )?,
        };
        //payment collection will always have only one element as we only make one transaction per order.
        let payment_collection = &item
            .response
            .purchase_units
            .first()
            .ok_or(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "paypal",
            ))?
            .payments;
        //payment collection item will either have "authorizations" field or "capture" field, not both at a time.
        let payment_collection_item = match (
            &payment_collection.authorizations,
            &payment_collection.captures,
        ) {
            (Some(authorizations), None) => authorizations.first(),
            (None, Some(captures)) => captures.first(),
            (Some(_), Some(captures)) => captures.first(),
            _ => None,
        }
        .ok_or(crate::utils::response_handling_fail_for_connector(
            item.http_code,
            "paypal",
        ))?;
        let status = payment_collection_item.status.clone();
        let status = common_enums::AttemptStatus::from(status);

        match utils::is_payment_failure(status) {
            true => {
                let error_code = payment_collection_item
                    .processor_response
                    .as_ref()
                    .and_then(|response| response.response_code.clone());

                let error_message = error_code
                    .as_deref()
                    .and_then(get_paypal_error_message)
                    .map(|message| message.to_string());

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Err(domain_types::router_data::ErrorResponse {
                        code: error_code.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                        message: error_message
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                        reason: error_message,
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: Some(item.response.id.clone()),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    ..item.router_data
                })
            }
            false => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: order_id,
                    redirection_data: None,
                    mandate_reference: Some(Box::new(MandateReference {
                        connector_mandate_id: match item.response.payment_source.clone() {
                            Some(paypal_source) => match paypal_source {
                                PaymentSourceItemResponse::Paypal(paypal_source) => {
                                    paypal_source.attributes.map(|attr| attr.vault.id)
                                }
                                PaymentSourceItemResponse::Card(card) => {
                                    card.attributes.map(|attr| attr.vault.id)
                                }
                                PaymentSourceItemResponse::Eps(_)
                                | PaymentSourceItemResponse::Ideal(_)
                                | PaymentSourceItemResponse::GooglePay(_)
                                | PaymentSourceItemResponse::ApplePay(_) => None,
                            },
                            None => None,
                        },
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                    })),
                    status_code: item.http_code,
                    connector_metadata: Some(connector_meta),
                    network_txn_id: None,
                    connector_response_reference_id: purchase_units
                        .invoice_id
                        .clone()
                        .or(Some(item.response.id)),
                    incremental_authorization_allowed: item
                        .router_data
                        .request
                        .get_request_incremental_authorization(),
                }),
                ..item.router_data
            }),
        }
    }
}

fn get_redirect_url(link_vec: Vec<PaypalLinks>) -> Result<Option<Url>, Report<ConnectorError>> {
    let mut link: Option<Url> = None;
    for item2 in link_vec.iter() {
        if item2.rel == "payer-action" {
            link.clone_from(&item2.href)
        }
    }
    Ok(link)
}

impl TryFrom<ResponseRouterData<PaypalSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaypalSyncResponse, Self>) -> Result<Self, Self::Error> {
        match item.response {
            PaypalSyncResponse::PaypalOrdersSyncResponse(response) => {
                Self::try_from(ResponseRouterData {
                    response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            PaypalSyncResponse::PaypalRedirectSyncResponse(response) => {
                Self::try_from(ResponseRouterData {
                    response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            PaypalSyncResponse::PaypalPaymentsSyncResponse(response) => {
                Self::try_from(ResponseRouterData {
                    response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            PaypalSyncResponse::PaypalThreeDsSyncResponse(response) => {
                Self::try_from(ResponseRouterData {
                    response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
        }
    }
}

impl TryFrom<ResponseRouterData<PaypalRedirectResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalRedirectResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payment_experience = item.router_data.request.payment_experience;
        let status = get_order_status(item.response.clone().status, item.response.intent.clone());
        let link = get_redirect_url(item.response.links.clone())?;

        // For Paypal SDK flow, we need to trigger SDK client and then complete authorize
        let next_action =
            if let Some(common_enums::PaymentExperience::InvokeSdkClient) = payment_experience {
                Some(NextActionCall::CompleteAuthorize)
            } else {
                None
            };
        let connector_meta = serde_json::json!(PaypalMeta {
            authorize_id: None,
            capture_id: None,
            incremental_authorization_id: None,
            psync_flow: item.response.intent,
            next_action,
            order_id: None
        });
        let purchase_units = item.response.purchase_units.first();
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: Some(Box::new(RedirectForm::from((
                    link.ok_or(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "paypal",
                    ))?,
                    Method::Get,
                )))),
                mandate_reference: None,
                connector_metadata: Some(connector_meta),
                network_txn_id: None,
                connector_response_reference_id: Some(
                    purchase_units.map_or(item.response.id, |item| item.invoice_id.clone()),
                ),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<F, T> TryFrom<ResponseRouterData<PaypalThreeDsSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalThreeDsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            // status is hardcoded because this try_from will only be reached in card 3ds before the completion of complete authorize flow.
            // also force sync won't be hit in terminal status thus leaving us with only one status to get here.
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::AuthenticationPending,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaypalAuthResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaypalAuthResponse, Self>) -> Result<Self, Self::Error> {
        match item.response {
            PaypalAuthResponse::PaypalOrdersResponse(orders_response) => {
                Self::try_from(ResponseRouterData {
                    response: orders_response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            PaypalAuthResponse::PaypalRedirectResponse(redirect_response) => {
                Self::try_from(ResponseRouterData {
                    response: redirect_response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            PaypalAuthResponse::PaypalThreeDsResponse(threeds_response) => {
                Self::try_from(ResponseRouterData {
                    response: threeds_response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaypalThreeDsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalThreeDsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let connector_meta = serde_json::json!(PaypalMeta {
            authorize_id: None,
            capture_id: None,
            incremental_authorization_id: None,
            psync_flow: PaypalPaymentIntent::Authenticate, // when there is no capture or auth id present
            next_action: None,
            order_id: None
        });

        let status = get_order_status(
            item.response.clone().status,
            PaypalPaymentIntent::Authenticate,
        );
        let link = get_redirect_url(item.response.links.clone())?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: Some(Box::new(paypal_threeds_link(
                    (
                        link,
                        item.router_data.request.complete_authorize_url.clone(),
                    ),
                    item.http_code,
                )?)),
                mandate_reference: None,
                connector_metadata: Some(connector_meta),
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaypalRedirectResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalRedirectResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_order_status(item.response.clone().status, item.response.intent.clone());
        let link = get_redirect_url(item.response.links.clone())?;

        let connector_meta = serde_json::json!(PaypalMeta {
            authorize_id: None,
            capture_id: None,
            incremental_authorization_id: None,
            psync_flow: item.response.intent,
            next_action: None,
            order_id: None
        });
        let purchase_units = item.response.purchase_units.first();
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: link.map(|url| Box::new(RedirectForm::from((url, Method::Get)))),
                mandate_reference: None,
                connector_metadata: Some(connector_meta),
                network_txn_id: None,
                connector_response_reference_id: Some(
                    purchase_units.map_or(item.response.id, |item| item.invoice_id.clone()),
                ),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

fn paypal_threeds_link(
    (redirect_url, complete_auth_url): (Option<Url>, Option<String>),
    http_status: u16,
) -> Result<RedirectForm, Report<ConnectorError>> {
    let mut redirect_url = redirect_url.ok_or_else(|| {
        Report::new(ConnectorError::response_handling_failed_with_context(
            http_status,
            Some("missing redirect URL for PayPal 3DS".to_string()),
        ))
    })?;
    let complete_auth_url = complete_auth_url.ok_or_else(|| {
        Report::new(ConnectorError::response_handling_failed_with_context(
            http_status,
            Some("complete_authorize_url missing for PayPal 3DS".to_string()),
        ))
    })?;
    let mut form_fields = std::collections::HashMap::from_iter(
        redirect_url
            .query_pairs()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    );

    // paypal requires return url to be passed as a field along with payer_action_url
    form_fields.insert(String::from("redirect_uri"), complete_auth_url);

    // Do not include query params in the endpoint
    redirect_url.set_query(None);

    Ok(RedirectForm::Form {
        endpoint: redirect_url.to_string(),
        method: Method::Get,
        form_fields,
    })
}

impl<F, T> TryFrom<ResponseRouterData<PaypalPaymentsSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalPaymentsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response
                        .supplementary_data
                        .related_ids
                        .order_id
                        .clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item
                    .response
                    .invoice_id
                    .clone()
                    .or(Some(item.response.supplementary_data.related_ids.order_id)),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
pub struct PaypalPaymentsCaptureRequest {
    amount: OrderAmount,
    final_capture: bool,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PaypalPaymentsCaptureRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let value = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        let amount = OrderAmount {
            currency_code: item.router_data.request.currency,
            value,
        };
        Ok(Self {
            amount,
            final_capture: true,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalPaymentStatus {
    Created,
    Captured,
    Completed,
    Declined,
    Voided,
    Failed,
    Pending,
    Denied,
    Expired,
    PartiallyCaptured,
    Refunded,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalCaptureResponse {
    id: String,
    status: PaypalPaymentStatus,
    amount: Option<OrderAmount>,
    invoice_id: Option<String>,
    final_capture: bool,
    payment_source: Option<PaymentSourceItemResponse>,
}

impl From<PaypalPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: PaypalPaymentStatus) -> Self {
        match item {
            PaypalPaymentStatus::Created => Self::Authorized,
            PaypalPaymentStatus::Completed
            | PaypalPaymentStatus::Captured
            | PaypalPaymentStatus::Refunded => Self::Charged,
            PaypalPaymentStatus::Declined => Self::Failure,
            PaypalPaymentStatus::Failed => Self::CaptureFailed,
            PaypalPaymentStatus::Pending => Self::Pending,
            PaypalPaymentStatus::Denied | PaypalPaymentStatus::Expired => Self::Failure,
            PaypalPaymentStatus::PartiallyCaptured => Self::PartialCharged,
            PaypalPaymentStatus::Voided => Self::Voided,
        }
    }
}

impl TryFrom<ResponseRouterData<PaypalCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = common_enums::AttemptStatus::from(item.response.status);
        let amount_captured = match status {
            common_enums::AttemptStatus::Pending
            | common_enums::AttemptStatus::Authorized
            | common_enums::AttemptStatus::Failure
            | common_enums::AttemptStatus::RouterDeclined
            | common_enums::AttemptStatus::AuthenticationFailed
            | common_enums::AttemptStatus::CaptureFailed
            | common_enums::AttemptStatus::Started
            | common_enums::AttemptStatus::AuthenticationPending
            | common_enums::AttemptStatus::AuthenticationSuccessful
            | common_enums::AttemptStatus::AuthorizationFailed
            | common_enums::AttemptStatus::Authorizing
            | common_enums::AttemptStatus::VoidInitiated
            | common_enums::AttemptStatus::CodInitiated
            | common_enums::AttemptStatus::CaptureInitiated
            | common_enums::AttemptStatus::VoidFailed
            | common_enums::AttemptStatus::AutoRefunded
            | common_enums::AttemptStatus::Unresolved
            | common_enums::AttemptStatus::Unspecified
            | common_enums::AttemptStatus::PaymentMethodAwaited
            | common_enums::AttemptStatus::ConfirmationAwaited
            | common_enums::AttemptStatus::DeviceDataCollectionPending
            | common_enums::AttemptStatus::Voided
            | common_enums::AttemptStatus::VoidedPostCapture
            | common_enums::AttemptStatus::VoidPostCaptureInitiated
            | common_enums::AttemptStatus::Expired
            | common_enums::AttemptStatus::Unknown
            | common_enums::AttemptStatus::PartiallyAuthorized => 0,
            common_enums::AttemptStatus::Charged
            | common_enums::AttemptStatus::PartialCharged
            | common_enums::AttemptStatus::PartialChargedAndChargeable
            | common_enums::AttemptStatus::IntegrityFailure => {
                item.router_data.request.amount_to_capture
            }
        };
        let connector_payment_id: PaypalMeta = match to_connector_meta(
            item.router_data
                .request
                .connector_feature_data
                .clone()
                .map(|m| m.expose()),
        ) {
            Ok(v) => v,
            Err(_) => {
                return Err(Report::new(
                    ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some(
                            "invalid or missing connector_meta_data on capture response"
                                .to_string(),
                        ),
                    ),
                ));
            }
        };

        // Simplified capture_id logic to use response.id directly
        let capture_id = item.response.id.clone();
        let invoice_id = item.response.invoice_id.clone();

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: Some(amount_captured),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item.router_data.request.connector_transaction_id.clone(),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(serde_json::json!(PaypalMeta {
                    authorize_id: connector_payment_id.authorize_id,
                    capture_id: Some(capture_id.clone()),
                    incremental_authorization_id: None,
                    psync_flow: PaypalPaymentIntent::Capture,
                    next_action: None,
                    order_id: None
                })),
                network_txn_id: None,
                connector_response_reference_id: invoice_id.or(Some(capture_id)),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalCancelStatus {
    Voided,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PaypalPaymentsCancelResponse {
    id: String,
    status: PaypalCancelStatus,
    amount: Option<OrderAmount>,
    invoice_id: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<PaypalPaymentsCancelResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalPaymentsCancelResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.status {
            PaypalCancelStatus::Voided => common_enums::AttemptStatus::Voided,
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item
                    .response
                    .invoice_id
                    .or(Some(item.response.id)),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<F, T> TryFrom<ResponseRouterData<PaypalSetupMandatesResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalSetupMandatesResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let info_response = item.response;

        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(info_response.id.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        }));
        // https://developer.paypal.com/docs/api/payment-tokens/v3/#payment-tokens_create
        // If 201 status code, then order is captured, other status codes are handled by the error handler
        let status = if item.http_code == 201 {
            common_enums::AttemptStatus::Charged
        } else {
            common_enums::AttemptStatus::Failure
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(info_response.id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(info_response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                domain_types::connector_flow::SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaypalZeroMandateRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<
                domain_types::connector_flow::SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_source = match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ccard) => ZeroMandateSourceItem::Card(CardMandateRequest {
                billing_address: get_address_info(
                    item.router_data.resource_common_data.get_optional_billing(),
                ),
                expiry: Some(ccard.get_expiry_date_as_yyyymm("-")),
                name: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_full_name(),
                number: ccard.card_number.peek().parse().ok(),
            }),

            PaymentMethodData::Wallet(_)
            | PaymentMethodData::CardRedirect(_)
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
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::MobilePayment(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                    connector: "Paypal",
                    context: Default::default(),
                }))?
            }
        };

        Ok(Self { payment_source })
    }
}

// TryFrom implementation for PostAuthenticate response
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaypalPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            // if card supports 3DS check for liability
            PaypalPostAuthenticateResponse::PaypalLiabilityResponse(liability_response) => {
                // permutation for status to continue payment
                match (
                    liability_response
                        .payment_source
                        .card
                        .authentication_result
                        .three_d_secure
                        .enrollment_status
                        .as_ref(),
                    liability_response
                        .payment_source
                        .card
                        .authentication_result
                        .three_d_secure
                        .authentication_status
                        .as_ref(),
                    &liability_response
                        .payment_source
                        .card
                        .authentication_result
                        .liability_shift,
                ) {
                    (
                        Some(EnrollmentStatus::Ready),
                        Some(AuthenticationStatus::Success),
                        LiabilityShift::Possible,
                    )
                    | (
                        Some(EnrollmentStatus::Ready),
                        Some(AuthenticationStatus::Attempted),
                        LiabilityShift::Possible,
                    )
                    | (Some(EnrollmentStatus::NotReady), None, LiabilityShift::No)
                    | (Some(EnrollmentStatus::Unavailable), None, LiabilityShift::No)
                    | (Some(EnrollmentStatus::Bypassed), None, LiabilityShift::No) => {
                        // Success: Authentication checks passed
                        Ok(Self {
                            flow: item.router_data.flow,
                            resource_common_data: PaymentFlowData {
                                status: common_enums::AttemptStatus::AuthenticationSuccessful,
                                ..item.router_data.resource_common_data
                            },
                            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                                authentication_data: None,
                                connector_response_reference_id: None,
                                status_code: item.http_code,
                            }),
                            connector_config: item.router_data.connector_config,
                            request: item.router_data.request,
                        })
                    }
                    _ => {
                        // Failed: Authentication checks failed
                        let error_message = format!(
                            "Cannot continue authentication. Connector Responded with LiabilityShift: {:?}, EnrollmentStatus: {:?}, and AuthenticationStatus: {:?}",
                            liability_response.payment_source.card.authentication_result.liability_shift,
                            liability_response
                                .payment_source
                                .card
                                .authentication_result
                                .three_d_secure
                                .enrollment_status
                                .unwrap_or(EnrollmentStatus::Null),
                            liability_response
                                .payment_source
                                .card
                                .authentication_result
                                .three_d_secure
                                .authentication_status
                                .unwrap_or(AuthenticationStatus::Null),
                        );

                        Ok(Self {
                            flow: item.router_data.flow,
                            resource_common_data: PaymentFlowData {
                                status: common_enums::AttemptStatus::Failure,
                                ..item.router_data.resource_common_data
                            },
                            response: Err(domain_types::router_data::ErrorResponse {
                                attempt_status: Some(common_enums::AttemptStatus::Failure),
                                code: "authentication_failed".to_string(),
                                message: "3DS authentication failed".to_string(),
                                connector_transaction_id: None,
                                reason: Some(error_message),
                                status_code: item.http_code,
                                network_advice_code: None,
                                network_decline_code: None,
                                network_error_message: None,
                            }),
                            connector_config: item.router_data.connector_config,
                            request: item.router_data.request,
                        })
                    }
                }
            }
            // if card does not support 3DS
            PaypalPostAuthenticateResponse::PaypalNonLiabilityResponse(_) => Ok(Self {
                flow: item.router_data.flow,
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthenticationSuccessful,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                    authentication_data: None,
                    connector_response_reference_id: None,
                    status_code: item.http_code,
                }),
                connector_config: item.router_data.connector_config,
                request: item.router_data.request,
            }),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum RefundStatus {
    Completed,
    Failed,
    Cancelled,
    Pending,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Completed => Self::Success,
            RefundStatus::Failed | RefundStatus::Cancelled => Self::Failure,
            RefundStatus::Pending => Self::Pending,
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OrderErrorDetails {
    pub issue: String,
    pub description: String,
    pub value: Option<String>,
    pub field: Option<String>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ErrorDetails {
    pub issue: String,
    pub description: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalSupplementaryData {
    pub related_ids: PaypalRelatedIds,
}
#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalRelatedIds {
    pub order_id: String,
}

#[derive(Default, Debug, Serialize)]
pub struct PaypalRefundRequest {
    pub amount: OrderAmount,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    > for PaypalRefundRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let value = item
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
            amount: OrderAmount {
                currency_code: item.router_data.request.currency,
                value,
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefundResponse {
    id: String,
    status: RefundStatus,
    amount: Option<OrderAmount>,
}

impl TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::Refund,
        RefundFlowData,
        RefundsData,
        RefundsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status: common_enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefundSyncResponse {
    id: String,
    status: RefundStatus,
}

impl TryFrom<ResponseRouterData<RefundSyncResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::RSync,
        RefundFlowData,
        RefundSyncData,
        RefundsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundSyncResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status: common_enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// RepeatPayment - TryFrom implementation for MIT payments
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaypalPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract connector mandate ID (vault_id) from mandate_reference
        let connector_mandate_id = match &item.router_data.request.mandate_reference {
            domain_types::connector_types::MandateReferenceId::ConnectorMandateId(data) => data
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })?,
            domain_types::connector_types::MandateReferenceId::NetworkMandateId(_)
            | domain_types::connector_types::MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Network mandate ID not supported for PayPal repeat payments"
                        .to_string(),
                    connector: "paypal",
                    context: Default::default()
                }));
            }
        };

        // Determine intent based on capture_method
        let intent = if item.router_data.request.is_auto_capture() {
            PaypalPaymentIntent::Capture
        } else {
            PaypalPaymentIntent::Authorize
        };
        let paypal_auth: PaypalAuthType =
            PaypalAuthType::try_from(&item.router_data.connector_config)?;
        let payee = get_payee(&paypal_auth);

        let amount = OrderRequestAmount::try_from(&item)?;
        let connector_request_reference_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let shipping_address = ShippingAddress::from(&item);
        let item_details = vec![ItemDetails::try_from(&item)?];

        let purchase_units = vec![PurchaseUnitRequest {
            reference_id: Some(connector_request_reference_id.clone()),
            custom_id: item.router_data.request.merchant_order_id.clone(),
            invoice_id: Some(connector_request_reference_id),
            amount,
            payee,
            shipping: Some(shipping_address),
            items: item_details,
        }];

        let payment_method_type = item.router_data.request.payment_method_type.ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "payment_method_type",
                context: Default::default(),
            },
        )?;

        let payment_source = match payment_method_type {
            common_enums::PaymentMethodType::Card => Some(PaymentSourceItem::Card(
                CardRequest::CardVaultStruct(VaultStruct {
                    vault_id: Secret::new(connector_mandate_id),
                }),
            )),
            common_enums::PaymentMethodType::Paypal => Some(PaymentSourceItem::Paypal(
                PaypalRedirectionRequest::PaypalVaultStruct(VaultStruct {
                    vault_id: Secret::new(connector_mandate_id),
                }),
            )),
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: format!(
                        "Payment method type {payment_method_type:?} not supported for PayPal repeat payments"
                    ),
                    connector: "paypal",
                context: Default::default()
                }));
            }
        };

        Ok(Self {
            intent,
            purchase_units,
            payment_source,
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct PaypalPaymentErrorResponse {
    pub name: Option<String>,
    pub message: String,
    pub debug_id: Option<String>,
    pub details: Option<Vec<ErrorDetails>>,
}

// ----------------------------------------------------------------------------
// Webhooks (Payments / Refunds / Disputes)
// ----------------------------------------------------------------------------

pub mod webhook_headers {
    // PayPal transmission headers used for signature verification
    pub const PAYPAL_TRANSMISSION_ID: &str = "paypal-transmission-id";
    pub const PAYPAL_TRANSMISSION_TIME: &str = "paypal-transmission-time";
    pub const PAYPAL_CERT_URL: &str = "paypal-cert-url";
    pub const PAYPAL_TRANSMISSION_SIG: &str = "paypal-transmission-sig";
    pub const PAYPAL_AUTH_ALGO: &str = "paypal-auth-algo";
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalWebhooksBody {
    pub event_type: PaypalWebhookEventType,
    pub resource: PaypalResource,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalWebooksEventType {
    pub event_type: PaypalWebhookEventType,
}

#[derive(Clone, Deserialize, Debug, strum::Display, Serialize)]
pub enum PaypalWebhookEventType {
    #[serde(rename = "PAYMENT.CAPTURE.COMPLETED")]
    PaymentCaptureCompleted,
    #[serde(rename = "PAYMENT.CAPTURE.PENDING")]
    PaymentCapturePending,
    #[serde(rename = "PAYMENT.CAPTURE.DECLINED")]
    PaymentCaptureDeclined,
    #[serde(rename = "PAYMENT.CAPTURE.REFUNDED")]
    PaymentCaptureRefunded,

    #[serde(rename = "CHECKOUT.ORDER.COMPLETED")]
    CheckoutOrderCompleted,
    #[serde(rename = "CHECKOUT.ORDER.PROCESSED")]
    CheckoutOrderProcessed,
    #[serde(rename = "CHECKOUT.ORDER.APPROVED")]
    CheckoutOrderApproved,

    #[serde(rename = "CUSTOMER.DISPUTE.CREATED")]
    CustomerDisputeCreated,
    #[serde(rename = "CUSTOMER.DISPUTE.RESOLVED")]
    CustomerDisputeResolved,
    #[serde(rename = "CUSTOMER.DISPUTE.UPDATED")]
    CustomerDisputedUpdated,
    #[serde(rename = "RISK.DISPUTE.CREATED")]
    RiskDisputeCreated,

    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(untagged)]
pub enum PaypalResource {
    PaypalCardWebhooks(Box<PaypalCardWebhooks>),
    PaypalRedirectsWebhooks(Box<PaypalRedirectsWebhooks>),
    PaypalRefundWebhooks(Box<PaypalRefundWebhooks>),
    PaypalDisputeWebhooks(Box<PaypalDisputeWebhooks>),
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalRefundWebhooks {
    pub id: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalCardWebhooks {
    pub supplementary_data: PaypalSupplementaryData,
    pub amount: OrderAmount,
    pub invoice_id: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalRedirectsWebhooks {
    pub purchase_units: Vec<PurchaseUnitItem>,
    pub id: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalDisputeWebhooks {
    pub dispute_id: String,
    pub disputed_transactions: Vec<DisputeTransaction>,
    pub dispute_amount: OrderAmount,
    pub dispute_outcome: Option<DisputeOutcome>,
    pub dispute_life_cycle_stage: DisputeLifeCycleStage,
    pub status: DisputeStatus,
    pub reason: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct DisputeTransaction {
    pub seller_transaction_id: String,
}

#[derive(Clone, Deserialize, Debug, strum::Display, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisputeLifeCycleStage {
    Inquiry,
    Chargeback,
    PreArbitration,
    Arbitration,
}

#[derive(Deserialize, Debug, strum::Display, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisputeStatus {
    Open,
    WaitingForBuyerResponse,
    WaitingForSellerResponse,
    UnderReview,
    Resolved,
    Other,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct DisputeOutcome {
    pub outcome_code: OutcomeCode,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutcomeCode {
    ResolvedBuyerFavour,
    ResolvedSellerFavour,
    ResolvedWithPayout,
    CanceledByBuyer,
    ACCEPTED,
    DENIED,
    NONE,
}

// ----------------------------------------------------------------------------
// Webhook Source Verification
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct PaypalSourceVerificationRequest {
    pub transmission_id: String,
    pub transmission_time: String,
    pub cert_url: String,
    pub transmission_sig: String,
    pub auth_algo: String,
    pub webhook_id: String,
    pub webhook_event: serde_json::Value,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PaypalSourceVerificationResponse {
    pub verification_status: PaypalSourceVerificationStatus,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalSourceVerificationStatus {
    Success,
    Failure,
}

#[derive(Deserialize, Debug)]
pub struct PaypalAccessTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

// Normalize headers to lowercase keys (HTTP header names are case-insensitive per RFC 7230).
fn webhook_headers_lowercase(
    headers: &std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, String> {
    headers
        .iter()
        .map(|(k, v)| (k.to_lowercase(), v.clone()))
        .collect()
}

// Transformers for VerifyWebhookSource flow
impl TryFrom<&VerifyWebhookSourceRequestData> for PaypalSourceVerificationRequest {
    type Error = Report<IntegrationError>;
    fn try_from(req: &VerifyWebhookSourceRequestData) -> Result<Self, Self::Error> {
        // Parse the webhook body into serde_json::Value
        // With preserve_order feature enabled, this preserves field order (uses IndexMap, not BTreeMap)
        let webhook_event = serde_json::from_slice(&req.webhook_body)
            .change_context(IntegrationError::NotImplemented(
                "webhook body decoding failed".to_string(),
                Default::default(),
            ))
            .attach_printable("Webhook body is not valid JSON")?;

        let headers = webhook_headers_lowercase(&req.webhook_headers);

        Ok(Self {
            transmission_id: headers
                .get(webhook_headers::PAYPAL_TRANSMISSION_ID)
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: webhook_headers::PAYPAL_TRANSMISSION_ID,
                    context: Default::default(),
                })?
                .clone(),
            transmission_time: headers
                .get(webhook_headers::PAYPAL_TRANSMISSION_TIME)
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: webhook_headers::PAYPAL_TRANSMISSION_TIME,
                    context: Default::default(),
                })?
                .clone(),
            cert_url: headers
                .get(webhook_headers::PAYPAL_CERT_URL)
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: webhook_headers::PAYPAL_CERT_URL,
                    context: Default::default(),
                })?
                .clone(),
            transmission_sig: headers
                .get(webhook_headers::PAYPAL_TRANSMISSION_SIG)
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: webhook_headers::PAYPAL_TRANSMISSION_SIG,
                    context: Default::default(),
                })?
                .clone(),
            auth_algo: headers
                .get(webhook_headers::PAYPAL_AUTH_ALGO)
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: webhook_headers::PAYPAL_AUTH_ALGO,
                    context: Default::default(),
                })?
                .clone(),
            webhook_id: String::from_utf8(req.merchant_secret.secret.to_vec())
                .change_context(IntegrationError::NotImplemented(
                    "webhook verification secret not found".to_string(),
                    Default::default(),
                ))
                .attach_printable("Could not convert secret to UTF-8")?,
            webhook_event,
        })
    }
}

impl From<PaypalSourceVerificationStatus> for VerifyWebhookStatus {
    fn from(item: PaypalSourceVerificationStatus) -> Self {
        match item {
            PaypalSourceVerificationStatus::Success => Self::SourceVerified,
            PaypalSourceVerificationStatus::Failure => Self::SourceNotVerified,
        }
    }
}

impl TryFrom<ResponseRouterData<PaypalSourceVerificationResponse, Self>>
    for RouterDataV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaypalSourceVerificationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(VerifyWebhookSourceResponseData {
                verify_webhook_status: VerifyWebhookStatus::from(item.response.verification_status),
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct PaypalOrderErrorResponse {
    pub name: Option<String>,
    pub message: String,
    pub debug_id: Option<String>,
    pub details: Option<Vec<OrderErrorDetails>>,
}

impl From<OrderErrorDetails> for ErrorCodeAndMessage {
    fn from(error: OrderErrorDetails) -> Self {
        Self {
            error_code: error.issue.to_string(),
            error_message: error.issue,
        }
    }
}

impl From<ErrorDetails> for ErrorCodeAndMessage {
    fn from(error: ErrorDetails) -> Self {
        Self {
            error_code: error.issue.to_string(),
            error_message: error.issue.to_string(),
        }
    }
}

fn get_paypal_error_message(error_code: &str) -> Option<&str> {
    match error_code {
        "00N7" | "RESPONSE_00N7" => Some("CVV2_FAILURE_POSSIBLE_RETRY_WITH_CVV."),
        "0390" | "RESPONSE_0390" => Some("ACCOUNT_NOT_FOUND."),
        "0500" | "RESPONSE_0500" => Some("DO_NOT_HONOR."),
        "0580" | "RESPONSE_0580" => Some("UNAUTHORIZED_TRANSACTION."),
        "0800" | "RESPONSE_0800" => Some("BAD_RESPONSE_REVERSAL_REQUIRED."),
        "0880" | "RESPONSE_0880" => Some("CRYPTOGRAPHIC_FAILURE."),
        "0890" | "RESPONSE_0890" => Some("UNACCEPTABLE_PIN."),
        "0960" | "RESPONSE_0960" => Some("SYSTEM_MALFUNCTION."),
        "0R00" | "RESPONSE_0R00" => Some("CANCELLED_PAYMENT."),
        "1000" | "RESPONSE_1000" => Some("PARTIAL_AUTHORIZATION."),
        "10BR" | "RESPONSE_10BR" => Some("ISSUER_REJECTED."),
        "1300" | "RESPONSE_1300" => Some("INVALID_DATA_FORMAT."),
        "1310" | "RESPONSE_1310" => Some("INVALID_AMOUNT."),
        "1312" | "RESPONSE_1312" => Some("INVALID_TRANSACTION_CARD_ISSUER_ACQUIRER."),
        "1317" | "RESPONSE_1317" => Some("INVALID_CAPTURE_DATE."),
        "1320" | "RESPONSE_1320" => Some("INVALID_CURRENCY_CODE."),
        "1330" | "RESPONSE_1330" => Some("INVALID_ACCOUNT."),
        "1335" | "RESPONSE_1335" => Some("INVALID_ACCOUNT_RECURRING."),
        "1340" | "RESPONSE_1340" => Some("INVALID_TERMINAL."),
        "1350" | "RESPONSE_1350" => Some("INVALID_MERCHANT."),
        "1352" | "RESPONSE_1352" => Some("RESTRICTED_OR_INACTIVE_ACCOUNT."),
        "1360" | "RESPONSE_1360" => Some("BAD_PROCESSING_CODE."),
        "1370" | "RESPONSE_1370" => Some("INVALID_MCC."),
        "1380" | "RESPONSE_1380" => Some("INVALID_EXPIRATION."),
        "1382" | "RESPONSE_1382" => Some("INVALID_CARD_VERIFICATION_VALUE."),
        "1384" | "RESPONSE_1384" => Some("INVALID_LIFE_CYCLE_OF_TRANSACTION."),
        "1390" | "RESPONSE_1390" => Some("INVALID_ORDER."),
        "1393" | "RESPONSE_1393" => Some("TRANSACTION_CANNOT_BE_COMPLETED."),
        "5100" | "RESPONSE_5100" => Some("GENERIC_DECLINE."),
        "5110" | "RESPONSE_5110" => Some("CVV2_FAILURE."),
        "5120" | "RESPONSE_5120" => Some("INSUFFICIENT_FUNDS."),
        "5130" | "RESPONSE_5130" => Some("INVALID_PIN."),
        "5135" | "RESPONSE_5135" => Some("DECLINED_PIN_TRY_EXCEEDED."),
        "5140" | "RESPONSE_5140" => Some("CARD_CLOSED."),
        "5150" | "RESPONSE_5150" => Some(
            "PICKUP_CARD_SPECIAL_CONDITIONS. Try using another card. Do not retry the same card.",
        ),
        "5160" | "RESPONSE_5160" => Some("UNAUTHORIZED_USER."),
        "5170" | "RESPONSE_5170" => Some("AVS_FAILURE."),
        "5180" | "RESPONSE_5180" => {
            Some("INVALID_OR_RESTRICTED_CARD. Try using another card. Do not retry the same card.")
        }
        "5190" | "RESPONSE_5190" => Some("SOFT_AVS."),
        "5200" | "RESPONSE_5200" => Some("DUPLICATE_TRANSACTION."),
        "5210" | "RESPONSE_5210" => Some("INVALID_TRANSACTION."),
        "5400" | "RESPONSE_5400" => Some("EXPIRED_CARD."),
        "5500" | "RESPONSE_5500" => Some("INCORRECT_PIN_REENTER."),
        "5650" | "RESPONSE_5650" => Some("DECLINED_SCA_REQUIRED."),
        "5700" | "RESPONSE_5700" => {
            Some("TRANSACTION_NOT_PERMITTED. Outside of scope of accepted business.")
        }
        "5710" | "RESPONSE_5710" => Some("TX_ATTEMPTS_EXCEED_LIMIT."),
        "5800" | "RESPONSE_5800" => Some("REVERSAL_REJECTED."),
        "5900" | "RESPONSE_5900" => Some("INVALID_ISSUE."),
        "5910" | "RESPONSE_5910" => Some("ISSUER_NOT_AVAILABLE_NOT_RETRIABLE."),
        "5920" | "RESPONSE_5920" => Some("ISSUER_NOT_AVAILABLE_RETRIABLE."),
        "5930" | "RESPONSE_5930" => Some("CARD_NOT_ACTIVATED."),
        "5950" | "RESPONSE_5950" => Some(
            "DECLINED_DUE_TO_UPDATED_ACCOUNT. External decline as an updated card has been issued.",
        ),
        "6300" | "RESPONSE_6300" => Some("ACCOUNT_NOT_ON_FILE."),
        "7700" | "RESPONSE_7700" => Some("ERROR_3DS."),
        "7710" | "RESPONSE_7710" => Some("AUTHENTICATION_FAILED."),
        "7800" | "RESPONSE_7800" => Some("BIN_ERROR."),
        "7900" | "RESPONSE_7900" => Some("PIN_ERROR."),
        "8000" | "RESPONSE_8000" => Some("PROCESSOR_SYSTEM_ERROR."),
        "8010" | "RESPONSE_8010" => Some("HOST_KEY_ERROR."),
        "8020" | "RESPONSE_8020" => Some("CONFIGURATION_ERROR."),
        "8030" | "RESPONSE_8030" => Some("UNSUPPORTED_OPERATION."),
        "8100" | "RESPONSE_8100" => Some("FATAL_COMMUNICATION_ERROR."),
        "8110" | "RESPONSE_8110" => Some("RETRIABLE_COMMUNICATION_ERROR."),
        "8220" | "RESPONSE_8220" => Some("SYSTEM_UNAVAILABLE."),
        "9100" | "RESPONSE_9100" => Some("DECLINED_PLEASE_RETRY. Retry."),
        "9500" | "RESPONSE_9500" => {
            Some("SUSPECTED_FRAUD. Try using another card. Do not retry the same card.")
        }
        "9510" | "RESPONSE_9510" => Some("SECURITY_VIOLATION."),
        "9520" | "RESPONSE_9520" => {
            Some("LOST_OR_STOLEN. Try using another card. Do not retry the same card.")
        }
        "9540" | "RESPONSE_9540" => Some("REFUSED_CARD."),
        "9600" | "RESPONSE_9600" => Some("UNRECOGNIZED_RESPONSE_CODE."),
        "PCNR" | "RESPONSE_PCNR" => Some("CONTINGENCIES_NOT_RESOLVED."),
        "PCVV" | "RESPONSE_PCVV" => Some("CVV_FAILURE."),
        "PP06" | "RESPONSE_PP06" => Some("ACCOUNT_CLOSED. A previously open account is now closed"),
        "PPRN" | "RESPONSE_PPRN" => Some("REATTEMPT_NOT_PERMITTED."),
        "PPAD" | "RESPONSE_PPAD" => Some("BILLING_ADDRESS."),
        "PPAB" | "RESPONSE_PPAB" => Some("ACCOUNT_BLOCKED_BY_ISSUER."),
        "PPAE" | "RESPONSE_PPAE" => Some("AMEX_DISABLED."),
        "PPAG" | "RESPONSE_PPAG" => Some("ADULT_GAMING_UNSUPPORTED."),
        "PPAI" | "RESPONSE_PPAI" => Some("AMOUNT_INCOMPATIBLE."),
        "PPAR" | "RESPONSE_PPAR" => Some("AUTH_RESULT."),
        "PPAU" | "RESPONSE_PPAU" => Some("MCC_CODE."),
        "PPAV" | "RESPONSE_PPAV" => Some("ARC_AVS."),
        "PPAX" | "RESPONSE_PPAX" => Some("AMOUNT_EXCEEDED."),
        "PPBG" | "RESPONSE_PPBG" => Some("BAD_GAMING."),
        "PPC2" | "RESPONSE_PPC2" => Some("ARC_CVV."),
        "PPCE" | "RESPONSE_PPCE" => Some("CE_REGISTRATION_INCOMPLETE."),
        "PPCO" | "RESPONSE_PPCO" => Some("COUNTRY."),
        "PPCR" | "RESPONSE_PPCR" => Some("CREDIT_ERROR."),
        "PPCT" | "RESPONSE_PPCT" => Some("CARD_TYPE_UNSUPPORTED."),
        "PPCU" | "RESPONSE_PPCU" => Some("CURRENCY_USED_INVALID."),
        "PPD3" | "RESPONSE_PPD3" => Some("SECURE_ERROR_3DS."),
        "PPDC" | "RESPONSE_PPDC" => Some("DCC_UNSUPPORTED."),
        "PPDI" | "RESPONSE_PPDI" => Some("DINERS_REJECT."),
        "PPDV" | "RESPONSE_PPDV" => Some("AUTH_MESSAGE."),
        "PPDT" | "RESPONSE_PPDT" => Some("DECLINE_THRESHOLD_BREACH."),
        "PPEF" | "RESPONSE_PPEF" => Some("EXPIRED_FUNDING_INSTRUMENT."),
        "PPEL" | "RESPONSE_PPEL" => Some("EXCEEDS_FREQUENCY_LIMIT."),
        "PPER" | "RESPONSE_PPER" => Some("INTERNAL_SYSTEM_ERROR."),
        "PPEX" | "RESPONSE_PPEX" => Some("EXPIRY_DATE."),
        "PPFE" | "RESPONSE_PPFE" => Some("FUNDING_SOURCE_ALREADY_EXISTS."),
        "PPFI" | "RESPONSE_PPFI" => Some("INVALID_FUNDING_INSTRUMENT."),
        "PPFR" | "RESPONSE_PPFR" => Some("RESTRICTED_FUNDING_INSTRUMENT."),
        "PPFV" | "RESPONSE_PPFV" => Some("FIELD_VALIDATION_FAILED."),
        "PPGR" | "RESPONSE_PPGR" => Some("GAMING_REFUND_ERROR."),
        "PPH1" | "RESPONSE_PPH1" => Some("H1_ERROR."),
        "PPIF" | "RESPONSE_PPIF" => Some("IDEMPOTENCY_FAILURE."),
        "PPII" | "RESPONSE_PPII" => Some("INVALID_INPUT_FAILURE."),
        "PPIM" | "RESPONSE_PPIM" => Some("ID_MISMATCH."),
        "PPIT" | "RESPONSE_PPIT" => Some("INVALID_TRACE_ID."),
        "PPLR" | "RESPONSE_PPLR" => Some("LATE_REVERSAL."),
        "PPLS" | "RESPONSE_PPLS" => Some("LARGE_STATUS_CODE."),
        "PPMB" | "RESPONSE_PPMB" => Some("MISSING_BUSINESS_RULE_OR_DATA."),
        "PPMC" | "RESPONSE_PPMC" => Some("BLOCKED_Mastercard."),
        "PPMD" | "RESPONSE_PPMD" => Some("DEPRECATED The PPMD value has been deprecated."),
        "PPNC" | "RESPONSE_PPNC" => Some("NOT_SUPPORTED_NRC."),
        "PPNL" | "RESPONSE_PPNL" => Some("EXCEEDS_NETWORK_FREQUENCY_LIMIT."),
        "PPNM" | "RESPONSE_PPNM" => Some("NO_MID_FOUND."),
        "PPNT" | "RESPONSE_PPNT" => Some("NETWORK_ERROR."),
        "PPPH" | "RESPONSE_PPPH" => Some("NO_PHONE_FOR_DCC_TRANSACTION."),
        "PPPI" | "RESPONSE_PPPI" => Some("INVALID_PRODUCT."),
        "PPPM" | "RESPONSE_PPPM" => Some("INVALID_PAYMENT_METHOD."),
        "PPQC" | "RESPONSE_PPQC" => Some("QUASI_CASH_UNSUPPORTED."),
        "PPRE" | "RESPONSE_PPRE" => Some("UNSUPPORT_REFUND_ON_PENDING_BC."),
        "PPRF" | "RESPONSE_PPRF" => Some("INVALID_PARENT_TRANSACTION_STATUS."),
        "PPRR" | "RESPONSE_PPRR" => Some("MERCHANT_NOT_REGISTERED."),
        "PPS0" | "RESPONSE_PPS0" => Some("BANKAUTH_ROW_MISMATCH."),
        "PPS1" | "RESPONSE_PPS1" => Some("BANKAUTH_ROW_SETTLED."),
        "PPS2" | "RESPONSE_PPS2" => Some("BANKAUTH_ROW_VOIDED."),
        "PPS3" | "RESPONSE_PPS3" => Some("BANKAUTH_EXPIRED."),
        "PPS4" | "RESPONSE_PPS4" => Some("CURRENCY_MISMATCH."),
        "PPS5" | "RESPONSE_PPS5" => Some("CREDITCARD_MISMATCH."),
        "PPS6" | "RESPONSE_PPS6" => Some("AMOUNT_MISMATCH."),
        "PPSC" | "RESPONSE_PPSC" => Some("ARC_SCORE."),
        "PPSD" | "RESPONSE_PPSD" => Some("STATUS_DESCRIPTION."),
        "PPSE" | "RESPONSE_PPSE" => Some("AMEX_DENIED."),
        "PPTE" | "RESPONSE_PPTE" => Some("VERIFICATION_TOKEN_EXPIRED."),
        "PPTF" | "RESPONSE_PPTF" => Some("INVALID_TRACE_REFERENCE."),
        "PPTI" | "RESPONSE_PPTI" => Some("INVALID_TRANSACTION_ID."),
        "PPTR" | "RESPONSE_PPTR" => Some("VERIFICATION_TOKEN_REVOKED."),
        "PPTT" | "RESPONSE_PPTT" => Some("TRANSACTION_TYPE_UNSUPPORTED."),
        "PPTV" | "RESPONSE_PPTV" => Some("INVALID_VERIFICATION_TOKEN."),
        "PPUA" | "RESPONSE_PPUA" => Some("USER_NOT_AUTHORIZED."),
        "PPUC" | "RESPONSE_PPUC" => Some("CURRENCY_CODE_UNSUPPORTED."),
        "PPUE" | "RESPONSE_PPUE" => Some("UNSUPPORT_ENTITY."),
        "PPUI" | "RESPONSE_PPUI" => Some("UNSUPPORT_INSTALLMENT."),
        "PPUP" | "RESPONSE_PPUP" => Some("UNSUPPORT_POS_FLAG."),
        "PPUR" | "RESPONSE_PPUR" => Some("UNSUPPORTED_REVERSAL."),
        "PPVC" | "RESPONSE_PPVC" => Some("VALIDATE_CURRENCY."),
        "PPVE" | "RESPONSE_PPVE" => Some("VALIDATION_ERROR."),
        "PPVT" | "RESPONSE_PPVT" => Some("VIRTUAL_TERMINAL_UNSUPPORTED."),
        _ => None,
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalAccessTokenErrorResponse {
    pub error: String,
    pub error_description: String,
}

// ---- ClientAuthenticationToken flow types ----

/// PayPal client token request for SDK initialization.
/// PayPal's v1/identity/generate-token endpoint accepts an optional customer_id
/// and returns a client_token for the JS SDK. Passing customer_id scopes the
/// token to the customer and enables vault-related features.
#[derive(Debug, Serialize)]
pub struct PaypalClientAuthTokenRequest {
    /// Optional customer ID to scope the client token to a specific customer.
    /// When provided, enables vault-related features in the PayPal JS SDK.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaypalClientAuthTokenRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let customer_id = item
            .router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string());

        Ok(Self { customer_id })
    }
}

/// PayPal client token response from v1/identity/generate-token.
#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalClientAuthTokenResponse {
    pub client_token: String,
}

impl TryFrom<ResponseRouterData<PaypalClientAuthTokenResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalClientAuthTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::Paypal(Box::new(
            PaypalClientAuthenticationResponseDomain {
                connector: "paypal".to_string(),
                session_token: response.client_token.clone(),
                sdk_next_action: SdkNextAction {
                    next_action: domain_types::connector_types::NextActionCall::Confirm,
                },
                client_token: Some(response.client_token),
                transaction_info: Some(PaypalTransactionInfoDomain {
                    flow: PaypalFlowDomain::Checkout,
                    currency_code: item.router_data.request.currency,
                    total_price: item.router_data.request.amount,
                }),
            },
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
