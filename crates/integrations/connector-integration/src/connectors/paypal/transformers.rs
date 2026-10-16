use super::PaypalRouterData;
use crate::{types::ResponseRouterData, utils::ErrorCodeAndMessage};
use base64::Engine;
use cards;
use common_enums;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::{AmountConvertor, MinorUnit, Money, StringMajorUnit, StringMajorUnitForConnector},
    CustomResult, Method,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, PostAuthenticate,
        RepeatPayment, VerifyWebhookSource, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData, MandateReference,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsPostAuthenticateData,
        PaymentsResponseData, PaymentsSyncData,
        PaypalClientAuthenticationResponse as PaypalClientAuthenticationResponseDomain,
        PaypalFlow as PaypalFlowDomain, PaypalTransactionInfo as PaypalTransactionInfoDomain,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SdkNextAction, ServerAuthenticationTokenResponseData, SetupMandateRequestData,
        VerifyWebhookSourceFlowData,
    },
    errors::{ConnectorError, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        BankDebitData, BankRedirectData, BankTransferData, CardRedirectData, GiftCardData,
        GpayTokenizationData, PayLaterData, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber, VoucherData, WalletData,
    },
    router_data::{ConnectorSpecificConfig, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{RedirectForm, VerifyWebhookSourceResponseData, VerifyWebhookStatus},
    utils,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use url::Url;
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
trait GetRequestIncrementalAuthorization {
    fn expected_currency(&self) -> Option<common_enums::Currency> {
        None
    }
    fn uses_saved_payment_identity(&self) -> bool {
        false
    }
    fn capture_context(&self) -> Option<PaypalCaptureContext> {
        None
    }
    fn get_request_incremental_authorization(&self) -> Option<bool>;
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    GetRequestIncrementalAuthorization for PaymentsAuthorizeData<T>
{
    fn expected_currency(&self) -> Option<common_enums::Currency> {
        Some(self.currency)
    }
    fn capture_context(&self) -> Option<PaypalCaptureContext> {
        Some(PaypalCaptureContext::from_request(
            self.capture_method,
            self.merchant_order_id.clone(),
            self.billing_descriptor.as_ref(),
        ))
    }
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        self.request_incremental_authorization
    }
}

impl GetRequestIncrementalAuthorization for PaymentsSyncData {
    fn expected_currency(&self) -> Option<common_enums::Currency> {
        Some(self.currency)
    }
    fn uses_saved_payment_identity(&self) -> bool {
        true
    }
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        None
    }
}

impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > GetRequestIncrementalAuthorization for RepeatPaymentData<T>
{
    fn expected_currency(&self) -> Option<common_enums::Currency> {
        Some(self.currency)
    }
    fn capture_context(&self) -> Option<PaypalCaptureContext> {
        Some(PaypalCaptureContext::from_request(
            self.capture_method,
            self.merchant_order_id.clone(),
            self.billing_descriptor.as_ref(),
        ))
    }
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

/// Constants for PayPal connector.
pub mod constants {
    pub const DEFAULT_NOTIFICATION_LANGUAGE: &str = "en-US";
    pub const DEFAULT_PARTNER_ATTRIBUTION_ID: &str = "HyperSwitchPPCP_SP";
    pub const DEFAULT_LEGACY_PARTNER_ATTRIBUTION_ID: &str = "HyperSwitchlegacy_Ecom";
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
        paypal_order_parts(item).map(|(amount, _)| amount)
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
                item.router_data.request.amount.amount,
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
                    .as_ref()
                    .map(|money| money.amount)
                    .unwrap_or(MinorUnit::zero()),
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
                discount: None,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    discount: Option<OrderAmount>,
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct PurchaseUnitRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    reference_id: Option<String>, //reference for an item in purchase_units
    #[serde(skip_serializing_if = "Option::is_none")]
    invoice_id: Option<String>, //The API caller-provided external invoice number for this order. Appears in both the payer's transaction history and the emails that the payer receives.
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_id: Option<String>, //Used to reconcile client transactions with PayPal transactions.
    amount: OrderRequestAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    payee: Option<Payee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping: Option<ShippingAddress>,
    items: Vec<ItemDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    soft_descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supplementary_data: Option<PaypalPurchaseSupplementaryData>,
}

#[derive(Default, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Payee {
    merchant_id: Secret<String>,
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct ItemDetails {
    name: String,
    quantity: String,
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
                item.router_data.request.amount.amount,
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
            quantity: ORDER_QUANTITY.to_string(),
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
                item.router_data.request.amount.amount,
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
            quantity: ORDER_QUANTITY.to_string(),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    address_line_2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    admin_area_1: Option<Secret<String>>,
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
                    .get_optional_shipping_full_name(),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    number: Option<RawCardNumber<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<CardRequestAttributes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardVaultStruct {
    vault_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<VaultRequestAttributes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultRequestAttributes {
    customer: Option<CustomerRequestData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerRequestData {
    merchant_customer_id: Option<common_utils::id_type::CustomerId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalVaultStruct {
    vault_id: Secret<String>,
    // Buyer-return context for a vaulted PayPal (wallet) charge that requires
    // re-approval. Only valid on `payment_source.paypal`.
    #[serde(skip_serializing_if = "Option::is_none")]
    experience_context: Option<ContextStruct>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CardRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    CardRequestStruct(CardRequestStruct<T>),
    CardVaultStruct(CardVaultStruct),
    NetworkToken(PaypalNetworkTokenCard),
    Mit(Box<PaypalMitCard<T>>),
}
fn network_token_invalid(message: &str) -> Report<IntegrationError> {
    IntegrationError::InvalidDataFormat {
        field_name: "payment_method.network_token",
        context: domain_types::errors::IntegrationErrorContext {
            doc_url: Some("https://developer.paypal.com/expanded/third-party-tokens/".to_owned()),
            additional_context: Some(message.to_owned()),
            ..Default::default()
        },
    }
    .into()
}

// An empty generated message carries no ACS result. Inspect every actual
// AuthenticationData field rather than refusing Some(default).
fn meaningful_external_authentication(
    data: Option<&domain_types::router_request_types::AuthenticationData>,
) -> bool {
    let Some(data) = data else {
        return false;
    };
    let nonempty = |value: Option<&str>| value.is_some_and(|value| !value.trim().is_empty());
    data.trans_status.is_some()
        || nonempty(data.eci.as_deref())
        || data
            .cavv
            .as_ref()
            .is_some_and(|value| !value.peek().trim().is_empty())
        || nonempty(data.ucaf_collection_indicator.as_deref())
        || nonempty(data.threeds_server_transaction_id.as_deref())
        || data.message_version.is_some()
        || nonempty(data.ds_trans_id.as_deref())
        || nonempty(data.acs_transaction_id.as_deref())
        || nonempty(data.transaction_id.as_deref())
        || data
            .network_params
            .as_ref()
            .is_some_and(|params| params.cartes_bancaires.is_some())
        || data.exemption_indicator.is_some()
        || data.created_at.is_some()
        || nonempty(data.challenge_code.as_deref())
        || nonempty(data.challenge_cancel.as_deref())
        || nonempty(data.challenge_code_reason.as_deref())
        || data
            .message_extension
            .as_ref()
            .is_some_and(|value| match value.peek() {
                serde_json::Value::Null => false,
                serde_json::Value::Object(value) => !value.is_empty(),
                serde_json::Value::Array(value) => !value.is_empty(),
                serde_json::Value::String(value) => !value.trim().is_empty(),
                _ => true,
            })
        || data.authentication_type.is_some()
}
fn reject_external_authentication(
    data: Option<&domain_types::router_request_types::AuthenticationData>,
) -> Result<(), Report<IntegrationError>> {
    if meaningful_external_authentication(data) {
        return Err(IntegrationError::NotSupported {
            message: "PayPal Orders has no documented external ACS authentication result carrier".to_owned(),
            connector: "paypal",
            context: domain_types::errors::IntegrationErrorContext {
                doc_url: Some("https://developer.paypal.com/api/orders/v2/schema.json".to_owned()),
                additional_context: Some("Use native PayPal authentication or a genuine TSP token cryptogram/ECI; ACS CAVV/trans_status cannot be imported through a wallet or network token".to_owned()),
                ..Default::default()
            },
        }.into());
    }
    Ok(())
}

fn network_token_eci<T: PaymentMethodDataTypes>(
    token: &domain_types::payment_method_data::NetworkTokenData,
    request: &PaymentsAuthorizeData<T>,
    auth_type: common_enums::AuthenticationType,
) -> Result<String, Report<IntegrationError>> {
    if auth_type == common_enums::AuthenticationType::ThreeDs {
        return Err(network_token_invalid("Hosted native 3DS is not supported for this network-token carrier; use the genuine TSP token cryptogram and ECI without an external ACS result"));
    }
    let cryptogram = token.token_cryptogram.as_ref().ok_or_else(|| {
        network_token_invalid("CIT requires the provisioned one-use token cryptogram")
    })?;
    if !(28..=32).contains(&cryptogram.peek().len()) {
        return Err(network_token_invalid(
            "Token cryptogram length must be28 to32",
        ));
    }
    let raw = token
        .eci
        .as_deref()
        .ok_or_else(|| network_token_invalid("CIT requires network-token ECI"))?;
    let master = token.card_network.as_ref() == Some(&common_enums::CardNetwork::Mastercard);
    let network_known = token.card_network.is_some();
    let eci = match raw {
        "00" => "MASTERCARD_NON_3D_SECURE_TRANSACTION",
        "01" => "MASTERCARD_ATTEMPTED_AUTHENTICATION_TRANSACTION",
        "02" => "MASTERCARD_FULLY_AUTHENTICATED_TRANSACTION",
        "05" => "FULLY_AUTHENTICATED_TRANSACTION",
        "06" => "ATTEMPTED_AUTHENTICATION_TRANSACTION",
        "07" => "NON_3D_SECURE_TRANSACTION",
        "MASTERCARD_NON_3D_SECURE_TRANSACTION"
        | "MASTERCARD_ATTEMPTED_AUTHENTICATION_TRANSACTION"
        | "MASTERCARD_FULLY_AUTHENTICATED_TRANSACTION"
        | "FULLY_AUTHENTICATED_TRANSACTION"
        | "ATTEMPTED_AUTHENTICATION_TRANSACTION"
        | "NON_3D_SECURE_TRANSACTION" => raw,
        _ => {
            return Err(network_token_invalid(
                "Unsupported numeric/descriptive ECI; use a documented enum or network-specific ECI00/01/02/05/06/07",
            ))
        }
    };
    if !network_known || eci.starts_with("MASTERCARD_") != master {
        return Err(network_token_invalid(
            "ECI must be corroborated by the explicit card network",
        ));
    }
    // TSP ECI describes the token transaction, not an imported ACS outcome.
    reject_external_authentication(request.authentication_data.as_ref())?;
    Ok(eci.to_owned())
}

#[derive(Debug, Serialize)]
pub struct PaypalNetworkTokenCard {
    network_token: PaypalNetworkTokenCredential,
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<Address>,
}

#[derive(Debug, Serialize)]
pub struct PaypalNetworkTokenCredential {
    number: cards::NetworkToken,
    expiry: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci_flag: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CardRequestAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    vault: Option<PaypalVault>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification: Option<ThreeDsMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer: Option<CustomerRequestData>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decrypted_token: Option<ApplePayDecryptedToken>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct ApplePayDecryptedToken {
    pub tokenized_card: ApplePayTokenizedCard,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_data_type: Option<String>,
    pub payment_data: ApplePayPaymentDataRequest,
    pub transaction_amount: OrderAmount,
    // Optional original deviceManufacturerIdentifier from Apple's decrypted token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_manufacturer_id: Option<Secret<String>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_expiration: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserAction {
    #[serde(rename = "PAY_NOW")]
    PayNow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    PaypalVaultStruct(PaypalVaultStruct),
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
    // required for vaulting multiple payment tokens for a customer, as per Paypal's documentation. If false or not provided, only one payment token can be vaulted for a customer. If true, multiple payment tokens can be vaulted for a customer. and this will be required for returning customer flow.
    permit_multiple_payment_tokens: Option<bool>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaypalVaultResponse {
    id: Option<String>,
    status: String,
    customer: Option<CustomerId>,
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
    #[serde(rename = "apple_pay")]
    AppleSaved(PaypalAppleSaved),
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
            .convert(
                router_data.request.amount.amount,
                router_data.request.currency,
            )
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
    #[serde(skip_serializing_if = "Option::is_none")]
    customer: Option<CustomerRequestData>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    security_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    number: Option<cards::CardNumber>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalSetupMandatesResponse {
    id: String,
    customer: Option<Customer>,
    payment_source: PaypalSetupTokenSource,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalSetupTokenSource {
    card: Option<PaypalVaultCardResponse>,
    apple_pay: Option<PaypalVaultAppleEvidence>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalVaultCardResponse {
    verification_status: Option<String>,
    verification: Option<PaypalVaultVerification>,
    network_transaction_reference: Option<PaypalNetworkTransactionReference>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalVaultVerification {
    network_transaction_id: Option<String>,
    amount: Option<OrderAmount>,
    processor_response: Option<ProcessorResponse>,
}
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaypalSetupMandateRequest {
    Direct(Box<PaypalZeroMandateRequest>),
    AppleSetup(PaypalAppleSetupRequest),
    Exchange(PaypalTokenExchangeRequest),
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaypalSetupMandateResponse {
    Setup(PaypalAppleSetupResponse),
    PaymentToken(Box<PaypalSetupMandatesResponse>),
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
    fn try_from(
        mut item: ResponseRouterData<PaypalAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let scheme = item
            .router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .and_then(|v| v.peek().get("paypal_network_reference"))
            .cloned();
        item.router_data.resource_common_data.connector_feature_data =
            scheme.map(|value| serde_json::json!({"paypal_network_reference":value}).into());
        // RepeatPayment returns PaypalOrdersResponse variant (direct capture)
        let original_scheme = item
            .router_data
            .request
            .metadata
            .as_ref()
            .and_then(|value| value.peek().get("paypal_mit"))
            .and_then(|value| value.get("previous_network_transaction_reference"))
            .cloned()
            .or_else(|| {
                item.router_data
                    .resource_common_data
                    .connector_feature_data
                    .as_ref()
                    .and_then(|value| value.peek().get("paypal_network_reference"))
                    .and_then(|value| value.get("reference"))
                    .cloned()
            })
            .and_then(|value| {
                serde_json::from_value::<PaypalNetworkTransactionReference>(value).ok()
            })
            .filter(|reference| {
                item.router_data.request.get_network_mandate_id().as_ref() == Some(&reference.id)
            });
        let original_mandate = item.router_data.request.connector_mandate_id();
        let mut result = match item.response {
            PaypalAuthResponse::PaypalOrdersResponse(orders_response) => {
                Self::try_from(ResponseRouterData {
                    response: orders_response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            // A PayPal wallet MIT (payment_source.paypal.vault_id) may require buyer
            // re-approval, in which case PayPal returns a redirect response instead of a
            // direct capture. Mirror the Authorize redirect handler so we surface an
            // AuthenticationPending redirect instead of hard-failing (parity with the
            // reference, where MIT flows through the Authorize response handler).
            PaypalAuthResponse::PaypalRedirectResponse(redirect_response) => {
                let status = get_order_status(
                    redirect_response.status.clone(),
                    redirect_response.intent.clone(),
                );
                let link = get_redirect_url(redirect_response.links.clone())?;
                let connector_meta = merge_setup_metadata(
                    &item.router_data.resource_common_data,
                    &PaypalMeta {
                        capture_context: item.router_data.request.capture_context(),
                        authorize_id: None,
                        capture_id: None,
                        incremental_authorization_id: None,
                        psync_flow: redirect_response.intent.clone(),
                        next_action: None,
                        order_id: Some(redirect_response.id.clone()),
                        reference_id: Some(
                            item.router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                        ),
                        continuation: None,
                        authentication_result: None,
                    },
                )
                .change_context(
                    crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
                )?;
                let purchase_units = redirect_response.purchase_units.first();
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            redirect_response.id.clone(),
                        ),
                        redirection_data: link
                            .map(|url| Box::new(RedirectForm::from((url, Method::Get)))),
                        mandate_reference: None,
                        connector_metadata: Some(connector_meta),
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: Some(
                            purchase_units
                                .map_or(redirect_response.id, |item| item.invoice_id.clone()),
                        ),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
            // A card MIT can surface a 3DS challenge; mirror the Authorize 3DS handler so the
            // buyer can complete authentication instead of hard-failing. RepeatPaymentData has
            // no `complete_authorize_url`, so the merchant `router_return_url` is used as the
            // post-challenge return target.
            PaypalAuthResponse::PaypalThreeDsResponse(threeds_response) => {
                let connector_meta = merge_setup_metadata(
                    &item.router_data.resource_common_data,
                    &PaypalMeta {
                        capture_context: item.router_data.request.capture_context(),
                        authorize_id: None,
                        capture_id: None,
                        incremental_authorization_id: None,
                        psync_flow: PaypalPaymentIntent::Authenticate,
                        next_action: None,
                        order_id: Some(threeds_response.id.clone()),
                        reference_id: Some(
                            item.router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                        ),
                        continuation: None,
                        authentication_result: None,
                    },
                )
                .change_context(
                    crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
                )?;
                let status = get_order_status(
                    threeds_response.status.clone(),
                    PaypalPaymentIntent::Authenticate,
                );
                let link = get_redirect_url(threeds_response.links.clone())?;
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(threeds_response.id),
                        redirection_data: Some(Box::new(paypal_threeds_link(
                            (link, item.router_data.request.router_return_url.clone()),
                            item.http_code,
                        )?)),
                        mandate_reference: None,
                        connector_metadata: Some(connector_meta),
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
        }?;
        if let (
            Some(id),
            Ok(PaymentsResponseData::TransactionResponse {
                mandate_reference, ..
            }),
        ) = (original_mandate, &mut result.response)
        {
            let reference = mandate_reference.get_or_insert_with(|| {
                Box::new(MandateReference {
                    connector_mandate_id: None,
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })
            });
            if reference.connector_mandate_id.is_none() {
                reference.connector_mandate_id = Some(id);
            }
        }
        if let (
            Some(reference),
            Ok(PaymentsResponseData::TransactionResponse {
                connector_metadata: Some(metadata),
                ..
            }),
        ) = (original_scheme, &mut result.response)
        {
            if let Some(object) = metadata.as_object_mut() {
                object
                    .entry("paypal_network_reference")
                    .or_insert_with(|| serde_json::json!({"version":1,"reference":reference}));
            }
        }
        Ok(result)
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
// Encrypted Apple Pay tokens use the separate PSP vault setup and approval path.
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
    if let Some(continuation) =
        paypal_authorize_meta(router_data)?.and_then(|meta| meta.continuation)
    {
        if continuation.stage == PaypalContinuationStage::PaymentTokenCreated
            && continuation.operation == PaypalContinuationOperation::Authorize
        {
            let vault_id = continuation
                .payment_token_id
                .filter(|id| !id.peek().is_empty())
                .ok_or_else(|| {
                    network_token_invalid(
                        "Apple Orders continuation requires the exchanged saved payment token",
                    )
                })?;
            return Ok(PaymentSourceItem::ApplePay(ApplePayRequest {
                id: apple_pay_data.transaction_identifier.clone(),
                name: router_data
                    .resource_common_data
                    .get_optional_payment_billing_full_name(),
                decrypted_token: None,
                vault_id: Some(vault_id),
            }));
        }
    }
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
        vault_id: None,
        decrypted_token: Some(ApplePayDecryptedToken {
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
            device_manufacturer_id: apple_pay_decrypted_data
                .device_manufacturer_identifier
                .clone(),
        }),
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
            address_line_2: address.line2.clone(),
            admin_area_1: address.state.clone(),
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
        BankRedirectData::Eps { .. } => Ok(PaymentSourceItem::Eps(RedirectRequest {
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
        BankRedirectData::Ideal { .. } => Ok(PaymentSourceItem::IDeal(RedirectRequest {
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
        BankRedirectData::Sofort { .. } => Ok(PaymentSourceItem::Sofort(RedirectRequest {
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
        | BankRedirectData::Przelewy24 { .. } => Err(IntegrationError::NotSupported {
            message: utils::get_unimplemented_payment_method_error_message("Paypal"),
            connector: "Paypal",
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some(
                    "This payment method has no supported PayPal Orders payment_source mapping"
                        .to_owned(),
                ),
                doc_url: Some(
                    "https://developer.paypal.com/docs/api/orders/v2/#orders_create".to_owned(),
                ),
                ..Default::default()
            },
        }
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
        | BankRedirectData::OpenBanking { .. }
        | BankRedirectData::Netbanking { .. } => Err(IntegrationError::NotSupported {
            message: utils::get_unimplemented_payment_method_error_message("Paypal"),
            connector: "Paypal",
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some(
                    "This payment method has no supported PayPal Orders payment_source mapping"
                        .to_owned(),
                ),
                doc_url: Some(
                    "https://developer.paypal.com/docs/api/orders/v2/#orders_create".to_owned(),
                ),
                ..Default::default()
            },
        })?,
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

        let (amount, item_details) = paypal_order_parts(&item)?;

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

        if connector_request_reference_id.is_empty()
            || connector_request_reference_id.chars().count() > 256
        {
            return Err(network_token_invalid(
                "PayPal purchase reference must contain1 to256 characters",
            ));
        }
        if item
            .router_data
            .request
            .merchant_order_id
            .as_ref()
            .is_some_and(|id| id.chars().count() > 255)
        {
            return Err(network_token_invalid(
                "PayPal custom_id must contain at most255 characters",
            ));
        }
        let invoice_id = (connector_request_reference_id.chars().count() <= 127)
            .then(|| connector_request_reference_id.clone());
        let purchase_units = vec![PurchaseUnitRequest {
            reference_id: Some(connector_request_reference_id.clone()),
            custom_id: item.router_data.request.merchant_order_id.clone(),
            invoice_id,
            amount,
            payee,
            shipping: if shipping_address.address.is_some() {
                Some(shipping_address)
            } else {
                None
            },
            items: item_details,
            soft_descriptor: paypal_descriptor(&item.router_data.request)?,
            description: item.router_data.resource_common_data.description.clone(),
            supplementary_data: paypal_supplementary(&item)?,
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
                                        // only required for paypal
                                        permit_multiple_payment_tokens: Some(false),
                                    }),

                                    common_enums::FutureUsage::OnSession => None,
                                },
                                None => None,
                            },
                            verification,
                            customer: item
                                .router_data
                                .resource_common_data
                                .customer_id
                                .clone()
                                .map(|customer_id| CustomerRequestData {
                                    merchant_customer_id: Some(customer_id),
                                }),
                        }),
                    },
                )));

                Ok(Self {
                    intent,
                    purchase_units,
                    payment_source,
                })
            }
            PaymentMethodData::CardWithNoCvc(ref card) => {
                let source = CardRequestStruct {
                    billing_address: get_address_info(item.router_data.resource_common_data.get_optional_payment_billing()),
                    expiry: Some(card.get_expiry_date_as_yyyymm("-")?),
                    name: card.card_holder_name.clone().or_else(|| item.router_data.resource_common_data.get_optional_payment_billing_full_name()),
                    number: Some(RawCardNumber(T::inner_from_card_number(card.card_number.clone()))),
                    security_code: None,
                    attributes: Some(CardRequestAttributes {
                        vault: if item.router_data.request.setup_future_usage == Some(common_enums::FutureUsage::OffSession) {
                            Some(PaypalVault { store_in_vault: StoreInVault::OnSuccess, usage_type: UsageType::Merchant, permit_multiple_payment_tokens: Some(false) })
                        } else { None },
                        verification: if item.router_data.resource_common_data.auth_type == common_enums::AuthenticationType::ThreeDs {
                            Some(ThreeDsMethod { method: ThreeDsType::ScaAlways })
                        } else { None },
                        customer: item.router_data.resource_common_data.customer_id.clone().map(|customer_id| CustomerRequestData { merchant_customer_id: Some(customer_id) }),
                    }),
                };
                Ok(Self { intent, purchase_units, payment_source: Some(PaymentSourceItem::Card(CardRequest::CardRequestStruct(source))) })
            }
            PaymentMethodData::NetworkToken(ref token) => {
                let eci = network_token_eci(token, &item.router_data.request, item.router_data.resource_common_data.auth_type)?;
                let month = token.token_exp_month.peek().parse::<u8>()
                    .map_err(|_| network_token_invalid("Token expiry month must be numeric"))?;
                if !(1..=12).contains(&month) { return Err(network_token_invalid("Token expiry month must be1 to12")); }
                let expiry = Secret::new(format!("{}-{month:02}", token.get_expiry_year_4_digit().peek()));
                Ok(Self { intent, purchase_units, payment_source: Some(PaymentSourceItem::Card(CardRequest::NetworkToken(PaypalNetworkTokenCard {
                    network_token: PaypalNetworkTokenCredential {
                        number: token.token_number.clone(),
                        expiry,
                        cryptogram: token.token_cryptogram.clone(),
                        eci_flag: Some(eci),
                    },
                    billing_address: get_address_info(item.router_data.resource_common_data.get_optional_payment_billing()),
                }))) })
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
                                                permit_multiple_payment_tokens: Some(true),
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
                                                permit_multiple_payment_tokens: Some(true),
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
                        validate_google_signed_fields(decrypted_data)?;
                        let authentication_method = decrypted_data.auth_method.unwrap_or_else(|| {
                            if decrypted_data.cryptogram.is_some() { common_enums::GooglePayAuthMethod::Cryptogram }
                            else { common_enums::GooglePayAuthMethod::PanOnly }
                        });
                        match authentication_method {
                            common_enums::GooglePayAuthMethod::Cryptogram if decrypted_data.cryptogram.as_ref().is_none_or(|crypto| crypto.peek().is_empty()) => {
                                return Err(network_token_invalid("Explicit Google CRYPTOGRAM_3DS requires the provisioned cryptogram"));
                            }
                            common_enums::GooglePayAuthMethod::PanOnly if decrypted_data.cryptogram.is_some() => {
                                return Err(network_token_invalid("Explicit Google PAN_ONLY cannot carry a cryptogram"));
                            }
                            common_enums::GooglePayAuthMethod::PanOnly | common_enums::GooglePayAuthMethod::Cryptogram => {}
                        }
                        let payment_source = Some(PaymentSourceItem::GooglePay(GooglePayRequest {
                            decrypted_token: GooglePayDecryptedToken {
                                message_id: decrypted_data.message_id.clone(),
                                message_expiration: decrypted_data.message_expiration.clone(),
                                payment_method: GooglePayPaymentMethod::Card,
                                authentication_method,
                                cryptogram: decrypted_data.cryptogram.clone(),
                                eci_indicator: if decrypted_data.cryptogram.is_some() { decrypted_data.eci_indicator.clone() } else { None },
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
                    // Orders v2 google_pay_request fields are name,email_address,phone_number,
                    // card,decrypted_token,assurance_details,experience_context; no encrypted token.
                    // https://developer.paypal.com/api/orders/v2/schema.json
                    GpayTokenizationData::Encrypted(_) => Err(IntegrationError::NotSupported {
                        message: "PayPal REST does not accept an encrypted Google Pay token".to_owned(),
                        connector: "paypal",
                        context: domain_types::errors::IntegrationErrorContext {
                            doc_url: Some("https://developer.paypal.com/api/orders/v2/schema.json".to_owned()),
                            additional_context: Some("Use a decrypted Google Pay card credential; client confirmOrder is not a server REST field".to_owned()),
                            ..Default::default()
                        },
                    }.into()),
                },
                WalletData::ApplePay(apple_pay_data) => {
                    let amount_value = item
                        .connector
                        .amount_converter
                        .convert(
                            item.router_data.request.amount.amount,
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
                | WalletData::GrabpayRedirect {}
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
                | WalletData::EaseBuzzRedirect(_)
                | WalletData::PaymayaRedirect(_)
                | WalletData::PayhereRedirect {}
                | WalletData::QwikcilverWalletDirect(_)
                | WalletData::Skrill(_)
                | WalletData::Neteller(_) => {
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
            | CardRedirectData::CardRedirect {}
            | CardRedirectData::Webpay {} => {
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
            | PayLaterData::TamaraRedirect {}
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
            | BankTransferData::LocalBankTransfer { .. } => Err(IntegrationError::NotSupported {
                message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                connector: "Paypal",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "This payment method has no supported PayPal Orders payment_source mapping"
                            .to_owned(),
                    ),
                    doc_url: Some(
                        "https://developer.paypal.com/docs/api/orders/v2/#orders_create".to_owned(),
                    ),
                    ..Default::default()
                },
            }
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
            | VoucherData::PayEasy(_) => Err(IntegrationError::NotSupported {
                message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                connector: "Paypal",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "This payment method has no supported PayPal Orders payment_source mapping"
                            .to_owned(),
                    ),
                    doc_url: Some(
                        "https://developer.paypal.com/docs/api/orders/v2/#orders_create".to_owned(),
                    ),
                    ..Default::default()
                },
            }
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
                MerchantAuthenticationFlowData,
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
                MerchantAuthenticationFlowData,
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
    for RouterDataV2<F, MerchantAuthenticationFlowData, T, ServerAuthenticationTokenResponseData>
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PaypalNetworkTransactionReference {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    acquirer_reference_number: Option<String>,
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
    #[serde(other)]
    Unknown,
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
        PaypalOrderStatus::Created
        | PaypalOrderStatus::Saved
        | PaypalOrderStatus::Pending
        | PaypalOrderStatus::Unknown => common_enums::AttemptStatus::Pending,
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
    network_transaction_reference: Option<PaypalNetworkTransactionReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorResponse {
    avs_code: Option<String>,
    cvv_code: Option<String>,
    response_code: Option<String>,
    payment_advice_code: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PaymentsCollection {
    authorizations: Option<Vec<PaymentsCollectionItem>>,
    // Missing is vendor-observed absence; explicit null is not the documented
    // array type and must never qualify as a zero capture collection.
    #[serde(default, deserialize_with = "deserialize_capture_collection")]
    captures: Option<Vec<PaymentsCollectionItem>>,
}

fn deserialize_capture_collection<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<PaymentsCollectionItem>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<PaymentsCollectionItem>::deserialize(deserializer).map(Some)
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseUnitItem {
    pub amount: Option<OrderAmount>,
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
pub struct PaypalPostAuthenticateResponse {
    id: String,
    status: String,
    payment_source: Option<PaypalPostAuthSources>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaypalPostAuthSources {
    card: Option<AuthResult>,
    apple_pay: Option<PaypalPostWalletEvidence>,
    google_pay: Option<PaypalPostWalletEvidence>,
    paypal: Option<PaypalPostWalletEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaypalPostWalletEvidence {}

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
    #[serde(other)]
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
    #[serde(other)]
    Unknown,
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
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalOrdersResponse {
    // Internal transport provenance, never accepted from provider JSON. Only
    // PSync on our fixed unfiltered Orders GET can set this after target checks.
    #[serde(skip)]
    full_owned_get: bool,
    id: String,
    intent: PaypalPaymentIntent,
    // PayPal can omit the top-level order `status` in some Orders v2 responses (observed in PSync
    // for an order whose only capture was `DECLINED`). This field is kept optional so the response
    // still deserializes when it is absent. The attempt status is not derived from this field
    // anyway: for orders it is taken from the capture/authorization status inside `purchase_units`
    // (see `TryFrom<ResponseRouterData<PaypalOrdersResponse, ..>>`).
    status: Option<PaypalOrderStatus>,
    purchase_units: Vec<PurchaseUnitItem>,
    payment_source: Option<PaymentSourceItemResponse>,
    payer: Option<Payer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payer {
    payer_id: Option<String>,
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
    payer: Option<Payer>,
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
    PaypalPaymentsSyncResponse(PaypalPaymentsSyncResponse),
    PaypalThreeDsSyncResponse(PaypalThreeDsSyncResponse),
    PaypalRedirectSyncResponse(PaypalRedirectResponse),
    PaypalOrderStateSyncResponse(PaypalOrderStateSyncResponse),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextActionCall {
    CompleteAuthorize,
}

/// Keep raw typed status vocabulary until the validated request target selects Orders or Payments.
/// Optional amount is financial evidence, never a response-type discriminator.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalSyncStatus {
    Created,
    Saved,
    Approved,
    PayerActionRequired,
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
    PartiallyRefunded,
    #[default]
    #[serde(other)]
    Unknown,
}
impl PaypalSyncStatus {
    fn order_status(&self) -> PaypalOrderStatus {
        match self {
            Self::Created => PaypalOrderStatus::Created,
            Self::Saved => PaypalOrderStatus::Saved,
            Self::Approved => PaypalOrderStatus::Approved,
            Self::PayerActionRequired => PaypalOrderStatus::PayerActionRequired,
            Self::Completed => PaypalOrderStatus::Completed,
            Self::Voided => PaypalOrderStatus::Voided,
            Self::Pending => PaypalOrderStatus::Pending,
            Self::Unknown => PaypalOrderStatus::Unknown,
            Self::Captured
            | Self::Declined
            | Self::Failed
            | Self::Denied
            | Self::Expired
            | Self::PartiallyCaptured
            | Self::Refunded
            | Self::PartiallyRefunded => PaypalOrderStatus::Unknown,
        }
    }
    fn payment_status(&self) -> PaypalPaymentStatus {
        match self {
            Self::Created => PaypalPaymentStatus::Created,
            Self::Captured => PaypalPaymentStatus::Captured,
            Self::Completed => PaypalPaymentStatus::Completed,
            Self::Declined => PaypalPaymentStatus::Declined,
            Self::Voided => PaypalPaymentStatus::Voided,
            Self::Failed => PaypalPaymentStatus::Failed,
            Self::Pending => PaypalPaymentStatus::Pending,
            Self::Denied => PaypalPaymentStatus::Denied,
            Self::Expired => PaypalPaymentStatus::Expired,
            Self::PartiallyCaptured => PaypalPaymentStatus::PartiallyCaptured,
            Self::Refunded => PaypalPaymentStatus::Refunded,
            Self::PartiallyRefunded => PaypalPaymentStatus::PartiallyRefunded,
            Self::Unknown => PaypalPaymentStatus::Unknown,
            Self::Saved | Self::Approved | Self::PayerActionRequired => {
                PaypalPaymentStatus::Unknown
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalPaymentsSyncResponse {
    id: String,
    #[serde(default)]
    status: PaypalSyncStatus,
    amount: Option<OrderAmount>,
    expiration_time: Option<String>,
    final_capture: Option<bool>,
    status_details: Option<PaypalCaptureStatusDetails>,
    invoice_id: Option<String>,
    supplementary_data: Option<PaypalSupplementaryData>,
    payer: Option<Payer>,
    processor_response: Option<ProcessorResponse>,
    network_transaction_reference: Option<PaypalNetworkTransactionReference>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalOrderStateSyncResponse {
    id: String,
    status: PaypalOrderStatus,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PaypalContinuationOperation {
    Authorize,
    SetupRecurring,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PaypalContinuationStage {
    SetupApprovalRequired,
    SetupApproved,
    PaymentTokenCreated,
    OrderApprovalRequired,
    OrderApproved,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalContinuation {
    pub operation: PaypalContinuationOperation,
    pub stage: PaypalContinuationStage,
    pub intent: PaypalPaymentIntent,
    pub setup_token_id: Option<Secret<String>>,
    pub payment_token_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_context: Option<PaypalCaptureContext>,
    pub authorize_id: Option<String>,
    pub capture_id: Option<String>,
    pub incremental_authorization_id: Option<String>,
    pub psync_flow: PaypalPaymentIntent,
    pub next_action: Option<NextActionCall>,
    pub order_id: Option<String>,
    #[serde(default)]
    pub reference_id: Option<String>,
    #[serde(default)]
    pub continuation: Option<PaypalContinuation>,
    #[serde(default)]
    pub authentication_result: Option<PaypalThreeDsParams>,
}

impl<F, Req> TryFrom<ResponseRouterData<PaypalOrdersResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
where
    Req: GetRequestIncrementalAuthorization,
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaypalOrdersResponse, Self>) -> Result<Self, Self::Error> {
        let previous: Option<PaypalMeta> = item
            .router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .and_then(|value| serde_json::from_value(value.clone().expose()).ok())
            .filter(|metadata: &PaypalMeta| {
                item.router_data.request.uses_saved_payment_identity()
                    || metadata
                        .order_id
                        .as_ref()
                        .is_none_or(|id| id == &item.response.id)
            });
        let target = previous
            .as_ref()
            .and_then(|meta| meta.reference_id.as_ref())
            .or_else(|| {
                (!item.router_data.request.uses_saved_payment_identity()).then_some(
                    &item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id,
                )
            })
            .filter(|reference| !reference.is_empty());
        let purchase_units = if let Some(target) = target {
            let mut matches = item
                .response
                .purchase_units
                .iter()
                .filter(|unit| unit.reference_id.as_ref() == Some(target));
            let matched = matches.next();
            if matches.next().is_some() {
                None
            } else {
                matched
            }
        } else {
            match item.response.purchase_units.as_slice() {
                [unit] => Some(unit),
                _ => None,
            }
        }
        .ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("PayPal purchase-unit identity missing or ambiguous".to_owned()),
            ))
        })?;

        let payment_intent = if item.router_data.request.uses_saved_payment_identity()
            && purchase_units
                .payments
                .captures
                .as_ref()
                .is_some_and(|captures| !captures.is_empty())
        {
            PaypalPaymentIntent::Capture
        } else {
            item.response.intent.clone()
        };
        let resources = match payment_intent {
            PaypalPaymentIntent::Capture => purchase_units.payments.captures.as_ref(),
            PaypalPaymentIntent::Authorize => purchase_units.payments.authorizations.as_ref(),
            PaypalPaymentIntent::Authenticate => None,
        }
        .ok_or_else(|| {
            crate::utils::response_handling_fail_for_connector(item.http_code, "paypal")
        })?;

        let requested_id = previous.as_ref().and_then(|meta| match payment_intent {
            PaypalPaymentIntent::Capture => meta.capture_id.as_ref(),
            PaypalPaymentIntent::Authorize => meta.authorize_id.as_ref(),
            PaypalPaymentIntent::Authenticate => None,
        });
        let payment_collection_item = if let Some(requested) = requested_id {
            let mut matches = resources
                .iter()
                .filter(|resource| &resource.id == requested);
            let matched = matches.next();
            if matches.next().is_some() {
                None
            } else {
                matched
            }
        } else {
            match resources.as_slice() {
                [resource] => Some(resource),
                // Only a display/processor representative: the complete Orders
                // ledger below determines money/status. Never save this arbitrary
                // member as a refund target when multiple captures were discovered.
                [resource, ..]
                    if item.router_data.request.uses_saved_payment_identity()
                        && payment_intent == PaypalPaymentIntent::Capture =>
                {
                    Some(resource)
                }
                _ => None,
            }
        }
        .ok_or_else(|| {
            crate::utils::response_handling_fail_for_connector(item.http_code, "paypal")
        })?;
        if item
            .router_data
            .request
            .expected_currency()
            .is_some_and(|currency| currency != payment_collection_item.amount.currency_code)
        {
            return Err(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "paypal",
            )
            .into());
        }
        let id = payment_collection_item.id.clone();
        let mut capture_context = previous
            .as_ref()
            .and_then(|meta| meta.capture_context.clone())
            .or_else(|| item.router_data.request.capture_context());
        if payment_intent == PaypalPaymentIntent::Authorize {
            let context = capture_context.get_or_insert_with(PaypalCaptureContext::default);
            context.authorized_amount = Some(payment_collection_item.amount.clone());
            if previous
                .as_ref()
                .and_then(|meta| meta.authorize_id.as_ref())
                .is_none()
            {
                context.capture_amounts = Some(std::collections::BTreeMap::new());
                context.capture_history_complete = Some(true);
            }
        } else if matches!(
            payment_collection_item.status,
            PaypalPaymentStatus::Completed
                | PaypalPaymentStatus::Captured
                | PaypalPaymentStatus::Refunded
                | PaypalPaymentStatus::PartiallyRefunded
        ) {
            if let Some(context) = capture_context.as_mut() {
                // Only a fresh payment execution establishes a complete new ledger.
                // A legacy sync with unknown history must never become an empty history.
                if !item.router_data.request.uses_saved_payment_identity()
                    && previous
                        .as_ref()
                        .is_none_or(|meta| meta.authorize_id.is_none() && meta.capture_id.is_none())
                {
                    context.capture_amounts = Some(std::collections::BTreeMap::new());
                    context.capture_history_complete = Some(true);
                }
                record_capture_amount(
                    context,
                    &payment_collection_item.id,
                    &payment_collection_item.amount,
                )
                .change_context(
                    crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
                )?;
            }
        }
        // C25 is a narrow vendor-observed qualification, not a general JSON
        // omission rule. A prior capture fact contradicts an empty aggregate.
        let qualified_zero = item.response.full_owned_get
            && item.response.purchase_units.len() == 1
            && previous.as_ref().is_some_and(|meta| {
                meta.order_id.as_ref() == Some(&item.response.id)
                    && meta.reference_id.is_some()
                    && meta.reference_id == purchase_units.reference_id
                    && meta.capture_id.is_none()
                    && meta.capture_context.as_ref().is_some_and(|context| {
                        context
                            .capture_amounts
                            .as_ref()
                            .is_none_or(|map| map.is_empty())
                            && context.authorized_amount.as_ref().is_some_and(|amount| {
                                purchase_units.amount.as_ref().is_some_and(|unit_amount| {
                                    amount.currency_code == unit_amount.currency_code
                                        && amount.value == unit_amount.value
                                        && item.router_data.request.expected_currency()
                                            == Some(amount.currency_code)
                                })
                            })
                    })
                    && purchase_units.payments.authorizations.as_ref().is_some_and(
                        |authorizations| {
                            matches!(authorizations.as_slice(), [authorization]
                            if meta.authorize_id.as_ref() == Some(&authorization.id)
                                && authorization.status == PaypalPaymentStatus::Voided
                                && purchase_units.amount.as_ref().is_some_and(|amount| {
                                    amount.currency_code == authorization.amount.currency_code
                                        && amount.value == authorization.amount.value
                                }))
                        },
                    )
            })
            && item
                .router_data
                .resource_common_data
                .amount_captured
                .as_ref()
                .is_none_or(|money| money.amount.get_amount_as_i64() == 0)
            && purchase_units
                .payments
                .captures
                .as_ref()
                .is_none_or(|captures| captures.is_empty());
        let mut observed_authorization_id = None;
        if item.router_data.request.uses_saved_payment_identity() {
            if let Some(authorizations) = purchase_units.payments.authorizations.as_ref() {
                let authorization = match previous
                    .as_ref()
                    .and_then(|meta| meta.authorize_id.as_ref())
                {
                    Some(id) => {
                        let mut matches = authorizations
                            .iter()
                            .filter(|authorization| &authorization.id == id);
                        let one = matches.next();
                        if matches.next().is_none() {
                            one
                        } else {
                            None
                        }
                    }
                    None => match authorizations.as_slice() {
                        [authorization] => Some(authorization),
                        _ => None,
                    },
                };
                if !authorizations.is_empty() && authorization.is_none() {
                    return Err(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "paypal",
                    )
                    .into());
                }
                if let Some(authorization) = authorization {
                    if !valid_sync_id(&authorization.id)
                        || item
                            .router_data
                            .request
                            .expected_currency()
                            .is_some_and(|currency| currency != authorization.amount.currency_code)
                    {
                        return Err(crate::utils::response_handling_fail_for_connector(
                            item.http_code,
                            "paypal",
                        )
                        .into());
                    }
                    observed_authorization_id = Some(authorization.id.clone());
                    capture_minor(&authorization.amount).change_context(
                        crate::utils::response_handling_fail_for_connector(
                            item.http_code,
                            "paypal",
                        ),
                    )?;
                    capture_context
                        .get_or_insert_with(PaypalCaptureContext::default)
                        .authorized_amount = Some(authorization.amount.clone());
                }
            }
            if let Some(captures) = purchase_units
                .payments
                .captures
                .as_ref()
                .filter(|captures| !captures.is_empty() || qualified_zero)
            {
                let context = capture_context.get_or_insert_with(PaypalCaptureContext::default);
                let previous_captures = context.capture_amounts.clone();
                context.capture_amounts = Some(std::collections::BTreeMap::new());
                context.capture_history_complete = Some(true);
                let mut observed_ids = std::collections::BTreeSet::new();
                for capture in captures {
                    if !valid_sync_id(&capture.id)
                        || !observed_ids.insert(&capture.id)
                        || item
                            .router_data
                            .request
                            .expected_currency()
                            .is_some_and(|currency| currency != capture.amount.currency_code)
                    {
                        return Err(crate::utils::response_handling_fail_for_connector(
                            item.http_code,
                            "paypal",
                        )
                        .into());
                    }
                    capture_minor(&capture.amount).change_context(
                        crate::utils::response_handling_fail_for_connector(
                            item.http_code,
                            "paypal",
                        ),
                    )?;
                    if matches!(
                        capture.status,
                        PaypalPaymentStatus::Completed
                            | PaypalPaymentStatus::Captured
                            | PaypalPaymentStatus::Refunded
                            | PaypalPaymentStatus::PartiallyRefunded
                    ) {
                        record_capture_amount(context, &capture.id, &capture.amount)
                            .change_context(crate::utils::response_handling_fail_for_connector(
                                item.http_code,
                                "paypal",
                            ))?;
                    }
                }
                // A delayed/incomplete order cannot erase already observed money.
                // Refunded resources still carry their original gross capture value.
                if previous_captures.as_ref().is_some_and(|previous| {
                    previous.iter().any(|(id, amount)| {
                        context
                            .capture_amounts
                            .as_ref()
                            .and_then(|current| current.get(id))
                            != Some(amount)
                    })
                }) {
                    return Err(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "paypal",
                    )
                    .into());
                }
            }
        }
        let mut current_hold_open = false;
        if item.router_data.request.uses_saved_payment_identity() {
            if purchase_units
                .payments
                .captures
                .as_ref()
                .is_none_or(|captures| captures.is_empty())
            {
                if let Some(context) = capture_context.as_mut() {
                    context.capture_history_complete = Some(qualified_zero);
                    if qualified_zero {
                        context.capture_amounts = Some(std::collections::BTreeMap::new());
                    }
                }
            }
            if let Some(authorizations) = purchase_units.payments.authorizations.as_ref() {
                for authorization in authorizations {
                    if previous
                        .as_ref()
                        .and_then(|metadata| metadata.authorize_id.as_ref())
                        .is_some_and(|id| id != &authorization.id)
                    {
                        continue;
                    }
                    if let Some(context) = capture_context.as_mut() {
                        let expired =
                            authorization_expired(authorization.expiration_time.as_deref())
                                .change_context(
                                    crate::utils::response_handling_fail_for_connector(
                                        item.http_code,
                                        "paypal",
                                    ),
                                )?;
                        if matches!(authorization.status, PaypalPaymentStatus::Voided) || expired {
                            context.authorization_closed = Some(true);
                        }
                        current_hold_open |= !expired
                            && matches!(
                                authorization.status,
                                PaypalPaymentStatus::Created
                                    | PaypalPaymentStatus::PartiallyCaptured
                            );
                    }
                }
            }
            if purchase_units
                .payments
                .captures
                .as_ref()
                .is_some_and(|captures| {
                    captures.iter().any(|capture| {
                        capture.final_capture == Some(true)
                            && matches!(
                                capture.status,
                                PaypalPaymentStatus::Completed
                                    | PaypalPaymentStatus::Captured
                                    | PaypalPaymentStatus::Refunded
                                    | PaypalPaymentStatus::PartiallyRefunded
                            )
                    })
                })
            {
                if let Some(context) = capture_context.as_mut() {
                    context.authorization_closed = Some(true);
                }
            }
        }
        let (connector_meta, order_id) = match payment_intent.clone() {
            PaypalPaymentIntent::Capture => (
                merge_setup_metadata(
                    &item.router_data.resource_common_data,
                    &PaypalMeta {
                        capture_context: capture_context.clone(),
                        authorize_id: previous
                            .as_ref()
                            .and_then(|meta| meta.authorize_id.clone())
                            .or(observed_authorization_id),
                        capture_id: if item.router_data.request.uses_saved_payment_identity()
                            && resources.len() > 1
                            && requested_id.is_none()
                        {
                            None
                        } else {
                            Some(id)
                        },
                        incremental_authorization_id: previous
                            .as_ref()
                            .and_then(|meta| meta.incremental_authorization_id.clone()),
                        psync_flow: payment_intent.clone(),
                        next_action: None,
                        order_id: Some(item.response.id.clone()),
                        reference_id: purchase_units.reference_id.clone().or_else(|| {
                            previous.as_ref().and_then(|meta| meta.reference_id.clone())
                        }),
                        continuation: previous
                            .as_ref()
                            .and_then(|meta| meta.continuation.clone())
                            .map(|mut continuation| {
                                continuation.stage = PaypalContinuationStage::Completed;
                                continuation
                            }),
                        authentication_result: previous
                            .as_ref()
                            .and_then(|meta| meta.authentication_result.clone()),
                    },
                )
                .change_context(
                    crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
                )?,
                ResponseId::ConnectorTransactionId(item.response.id.clone()),
            ),

            PaypalPaymentIntent::Authorize => (
                merge_setup_metadata(
                    &item.router_data.resource_common_data,
                    &PaypalMeta {
                        capture_context: capture_context.clone(),
                        authorize_id: Some(id),
                        capture_id: previous.as_ref().and_then(|meta| meta.capture_id.clone()),
                        incremental_authorization_id: Some(payment_collection_item.id.clone()),
                        psync_flow: payment_intent.clone(),
                        next_action: None,
                        order_id: Some(item.response.id.clone()),
                        reference_id: purchase_units.reference_id.clone().or_else(|| {
                            previous.as_ref().and_then(|meta| meta.reference_id.clone())
                        }),
                        continuation: previous
                            .as_ref()
                            .and_then(|meta| meta.continuation.clone())
                            .map(|mut continuation| {
                                continuation.stage = PaypalContinuationStage::Completed;
                                continuation
                            }),
                        authentication_result: previous
                            .as_ref()
                            .and_then(|meta| meta.authentication_result.clone()),
                    },
                )
                .change_context(
                    crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
                )?,
                ResponseId::ConnectorTransactionId(item.response.id.clone()),
            ),

            PaypalPaymentIntent::Authenticate => Err(
                crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
            )?,
        };
        let mut connector_meta = connector_meta;
        if let Some(reference) = payment_collection_item
            .network_transaction_reference
            .as_ref()
            .filter(|reference| !reference.id.is_empty())
        {
            if let Some(object) = connector_meta.as_object_mut() {
                object.insert(
                    "paypal_network_reference".to_owned(),
                    serde_json::json!({"version":1,"reference":reference}),
                );
            }
        }
        let processor = payment_collection_item.processor_response.as_ref();
        let connector_response = processor.map(|processor| {
            domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
                domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                    authentication_data: None,
                    payment_checks: Some(serde_json::json!({"avs_code": processor.avs_code, "cvv_code": processor.cvv_code})),
                    card_network: None, domestic_network: None, auth_code: None,
                }
            )
        });
        let network_txn_id = payment_collection_item
            .network_transaction_reference
            .as_ref()
            .map(|reference| reference.id.clone());
        let mut captured = if payment_intent == PaypalPaymentIntent::Capture
            && matches!(
                payment_collection_item.status,
                PaypalPaymentStatus::Completed
                    | PaypalPaymentStatus::Captured
                    | PaypalPaymentStatus::PartiallyCaptured
                    | PaypalPaymentStatus::Refunded
                    | PaypalPaymentStatus::PartiallyRefunded
            ) {
            if item
                .router_data
                .resource_common_data
                .amount
                .as_ref()
                .is_some_and(|money| money.currency != payment_collection_item.amount.currency_code)
            {
                return Err(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "paypal",
                )
                .into());
            }
            let amount = StringMajorUnitForConnector
                .convert_back(
                    payment_collection_item.amount.value.clone(),
                    payment_collection_item.amount.currency_code,
                )
                .change_context(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "paypal",
                ))?;
            Some(Money {
                amount,
                currency: payment_collection_item.amount.currency_code,
            })
        } else {
            None
        };
        // Derive the attempt status from the capture/authorization status rather than the
        // top-level order `status`, which PayPal may omit (e.g. a declined capture). For example a
        // `DECLINED` capture maps to `AttemptStatus::Failure` and is handled by the branch below.
        let mut status = paypal_resource_status(
            &payment_collection_item.status,
            payment_intent == PaypalPaymentIntent::Capture,
        );

        if item.router_data.request.uses_saved_payment_identity() {
            if let Some(context) = capture_context.as_ref() {
                status = financial_sync_status(context, status, current_hold_open).change_context(
                    crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
                )?;
                // Missing/pending authorization information is not evidence that
                // an outstanding ManualMultiple hold was closed. Keep actual
                // captured money while the closure outcome remains uncertain.
                if matches!(
                    context.capture_method,
                    Some(
                        common_enums::CaptureMethod::ManualMultiple
                            | common_enums::CaptureMethod::Manual
                    )
                ) && context.authorization_closed != Some(true)
                    && !current_hold_open
                    && status == common_enums::AttemptStatus::PartialCharged
                {
                    status = common_enums::AttemptStatus::Pending;
                }
                if let Some(money) = captured_ledger_money(context).change_context(
                    crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
                )? {
                    captured = Some(money);
                }
            }
        } else {
            if payment_intent == PaypalPaymentIntent::Capture
                && captured.is_some()
                && capture_context
                    .as_ref()
                    .map(capture_remaining)
                    .transpose()
                    .change_context(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "paypal",
                    ))?
                    .flatten()
                    .is_some_and(|remaining| remaining > 0)
            {
                status = match payment_collection_item.final_capture {
                    Some(true) => common_enums::AttemptStatus::PartialCharged,
                    Some(false) => common_enums::AttemptStatus::PartialChargedAndChargeable,
                    None => status,
                };
            }
        }
        if item.router_data.request.uses_saved_payment_identity() && captured.is_none() {
            captured = item
                .router_data
                .resource_common_data
                .amount_captured
                .clone();
        }
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
                        code: error_code
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                        message: error_message
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                        reason: error_message,
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(status)),
                        connector_transaction_id: Some(item.response.id.clone()),
                        network_decline_code: error_code.clone(),
                        network_advice_code: processor
                            .and_then(|value| value.payment_advice_code.clone()),
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                    ..item.router_data
                })
            }
            false => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    connector_response,
                    amount_captured: captured,
                    sender_payment_instrument_id: item
                        .response
                        .payer
                        .as_ref()
                        .and_then(|payer| payer.payer_id.clone()),
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: order_id,
                    redirection_data: None,
                    mandate_reference: Some(Box::new(MandateReference {
                        connector_mandate_id: match item.response.payment_source.clone() {
                            Some(paypal_source) => match paypal_source {
                                PaymentSourceItemResponse::Paypal(paypal_source) => {
                                    paypal_source.attributes.and_then(|attr| attr.vault.id)
                                }
                                PaymentSourceItemResponse::Card(card) => {
                                    card.attributes.and_then(|attr| attr.vault.id)
                                }
                                PaymentSourceItemResponse::Eps(_)
                                | PaymentSourceItemResponse::Ideal(_)
                                | PaymentSourceItemResponse::GooglePay(_)
                                | PaymentSourceItemResponse::ApplePay(_) => None,
                            },
                            None => None,
                        },
                        payment_method_id: None,
                        // Surface the PayPal vault customer id (returning-customer flow),
                        // mirroring the reference. Only PayPal sources carry a vault customer;
                        // card/eps/ideal/wallet sources have none here.
                        connector_mandate_request_reference_id: match item
                            .response
                            .payment_source
                            .clone()
                        {
                            Some(PaymentSourceItemResponse::Paypal(paypal_source)) => paypal_source
                                .attributes
                                .and_then(|attr| attr.vault.customer.map(|cus| cus.id)),
                            Some(PaymentSourceItemResponse::Card(_))
                            | Some(PaymentSourceItemResponse::Eps(_))
                            | Some(PaymentSourceItemResponse::Ideal(_))
                            | Some(PaymentSourceItemResponse::GooglePay(_))
                            | Some(PaymentSourceItemResponse::ApplePay(_))
                            | None => None,
                        },
                        mandate_metadata: None,
                    })),
                    status_code: item.http_code,
                    connector_metadata: Some(connector_meta),
                    network_txn_id,
                    network_txn_link_id: None,
                    connector_response_reference_id: purchase_units
                        .invoice_id
                        .clone()
                        .or(Some(item.response.id)),
                    incremental_authorization_allowed: item
                        .router_data
                        .request
                        .get_request_incremental_authorization(),
                    splits: None,
                    payment_account_reference: None,
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
        let metadata = sync_metadata(&item.router_data).change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
        )?;
        match item.response {
            PaypalSyncResponse::PaypalOrdersSyncResponse(mut response) => {
                validate_sync_order_identity(
                    &item.router_data,
                    &metadata,
                    &response.id,
                    item.http_code,
                )?;
                // get_url for PSync builds exactly the saved target path, with
                // no fields/filter/pagination query. The target check above
                // excludes direct authorization/capture responses.
                response.full_owned_get = true;
                Self::try_from(ResponseRouterData {
                    response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })
            }
            PaypalSyncResponse::PaypalPaymentsSyncResponse(response) => {
                if matches!(
                    sync_target(&item.router_data, &metadata).change_context(
                        crate::utils::response_handling_fail_for_connector(
                            item.http_code,
                            "paypal"
                        )
                    )?,
                    PaypalSyncTarget::Order(_)
                ) {
                    sync_order_state(
                        item.router_data,
                        metadata,
                        response.id,
                        response.status.order_status(),
                        item.http_code,
                    )
                } else {
                    Self::try_from(ResponseRouterData {
                        response,
                        router_data: item.router_data,
                        http_code: item.http_code,
                    })
                }
            }
            PaypalSyncResponse::PaypalRedirectSyncResponse(response) => sync_order_state(
                item.router_data,
                metadata,
                response.id,
                response.status,
                item.http_code,
            ),
            PaypalSyncResponse::PaypalThreeDsSyncResponse(response) => sync_order_state(
                item.router_data,
                metadata,
                response.id,
                response.status,
                item.http_code,
            ),
            PaypalSyncResponse::PaypalOrderStateSyncResponse(response) => sync_order_state(
                item.router_data,
                metadata,
                response.id,
                response.status,
                item.http_code,
            ),
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
        let connector_meta = merge_setup_metadata(
            &item.router_data.resource_common_data,
            &PaypalMeta {
                capture_context: saved_capture_context(&item.router_data.resource_common_data),
                authorize_id: None,
                capture_id: None,
                incremental_authorization_id: None,
                psync_flow: item.response.intent,
                next_action,
                order_id: None,
                reference_id: None,
                continuation: None,
                authentication_result: None,
            },
        )
        .change_context(crate::utils::response_handling_fail_for_connector(
            item.http_code,
            "paypal",
        ))?;
        let purchase_units = item.response.purchase_units.first();
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                sender_payment_instrument_id: item
                    .response
                    .payer
                    .as_ref()
                    .and_then(|payer| payer.payer_id.clone()),
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
                network_txn_link_id: None,
                connector_response_reference_id: Some(
                    purchase_units.map_or(item.response.id, |item| item.invoice_id.clone()),
                ),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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
        let reference_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let connector_meta = merge_setup_metadata(
            &item.router_data.resource_common_data,
            &PaypalMeta {
                capture_context: saved_capture_context(&item.router_data.resource_common_data)
                    .or_else(|| item.router_data.request.capture_context()),
                authorize_id: None,
                capture_id: None,
                incremental_authorization_id: None,
                psync_flow: if item.router_data.request.is_auto_capture() {
                    PaypalPaymentIntent::Capture
                } else {
                    PaypalPaymentIntent::Authorize
                },
                next_action: None,
                order_id: Some(item.response.id.clone()),
                reference_id: Some(
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
                continuation: None,
                authentication_result: None,
            },
        )
        .change_context(crate::utils::response_handling_fail_for_connector(
            item.http_code,
            "paypal",
        ))?;

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
                network_txn_link_id: None,
                connector_response_reference_id: Some(reference_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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

        let connector_meta = merge_setup_metadata(
            &item.router_data.resource_common_data,
            &PaypalMeta {
                capture_context: saved_capture_context(&item.router_data.resource_common_data)
                    .or_else(|| item.router_data.request.capture_context()),
                authorize_id: None,
                capture_id: None,
                incremental_authorization_id: None,
                psync_flow: item.response.intent,
                next_action: None,
                order_id: Some(item.response.id.clone()),
                reference_id: Some(
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
                continuation: None,
                authentication_result: None,
            },
        )
        .change_context(crate::utils::response_handling_fail_for_connector(
            item.http_code,
            "paypal",
        ))?;
        let purchase_units = item.response.purchase_units.first();
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                sender_payment_instrument_id: item
                    .response
                    .payer
                    .as_ref()
                    .and_then(|payer| payer.payer_id.clone()),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: link.map(|url| Box::new(RedirectForm::from((url, Method::Get)))),
                mandate_reference: None,
                connector_metadata: Some(connector_meta),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(
                    purchase_units.map_or(item.response.id, |item| item.invoice_id.clone()),
                ),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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

impl TryFrom<ResponseRouterData<PaypalPaymentsSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalPaymentsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let fail = || crate::utils::response_handling_fail_for_connector(item.http_code, "paypal");
        let mut metadata = sync_metadata(&item.router_data).change_context(fail())?;
        let target = sync_target(&item.router_data, &metadata).change_context(fail())?;
        let is_capture = match &target {
            PaypalSyncTarget::Capture(id) if id == &item.response.id => true,
            PaypalSyncTarget::Authorization(id) if id == &item.response.id => false,
            PaypalSyncTarget::Order(_)
            | PaypalSyncTarget::SetupToken(_)
            | PaypalSyncTarget::PaymentToken(_)
            | PaypalSyncTarget::Capture(_)
            | PaypalSyncTarget::Authorization(_) => return Err(fail().into()),
        };
        let actual_order = item
            .response
            .supplementary_data
            .as_ref()
            .map(|data| &data.related_ids.order_id)
            .filter(|id| !id.is_empty());
        if metadata
            .order_id
            .as_ref()
            .zip(actual_order)
            .is_some_and(|(known, actual)| known != actual)
        {
            return Err(fail().into());
        }
        let supplied = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(fail())?;
        if supplied != item.response.id
            && metadata.order_id.as_ref() != Some(&supplied)
            && actual_order != Some(&supplied)
            && metadata.capture_id.as_ref() != Some(&supplied)
            && metadata.authorize_id.as_ref() != Some(&supplied)
        {
            return Err(fail().into());
        }
        if let Some(order) = actual_order {
            metadata.order_id = Some(order.clone());
        }
        let resource_status = item.response.status.payment_status();
        let mut status = paypal_resource_status(&resource_status, is_capture);
        if item
            .response
            .amount
            .as_ref()
            .is_some_and(|amount| amount.currency_code != item.router_data.request.currency)
        {
            return Err(fail().into());
        }
        let collected = is_capture
            && matches!(
                resource_status,
                PaypalPaymentStatus::Completed
                    | PaypalPaymentStatus::Captured
                    | PaypalPaymentStatus::Refunded
                    | PaypalPaymentStatus::PartiallyRefunded
                    | PaypalPaymentStatus::PartiallyCaptured
            );
        let mut captured = if collected {
            item.response
                .amount
                .as_ref()
                .map(|amount| -> Result<Money, Report<ConnectorError>> {
                    Ok(Money {
                        amount: StringMajorUnitForConnector
                            .convert_back(amount.value.clone(), amount.currency_code)
                            .change_context(fail())?,
                        currency: amount.currency_code,
                    })
                })
                .transpose()?
        } else {
            None
        };
        if is_capture {
            metadata.capture_id = Some(item.response.id.clone());
            metadata.psync_flow = PaypalPaymentIntent::Capture;
            if let (Some(context), Some(amount)) = (
                metadata.capture_context.as_mut(),
                item.response.amount.as_ref(),
            ) {
                if collected {
                    record_capture_amount(context, &item.response.id, amount)
                        .change_context(fail())?;
                }
            }
        } else {
            metadata.authorize_id = Some(item.response.id.clone());
            metadata.psync_flow = if metadata.capture_id.is_some() {
                PaypalPaymentIntent::Capture
            } else {
                PaypalPaymentIntent::Authorize
            };
            if let Some(amount) = item.response.amount.as_ref() {
                capture_minor(amount).change_context(fail())?;
                metadata
                    .capture_context
                    .get_or_insert_with(PaypalCaptureContext::default)
                    .authorized_amount = Some(amount.clone());
            }
        }
        let expired = !is_capture
            && authorization_expired(item.response.expiration_time.as_deref())
                .change_context(fail())?;
        let current_open = !is_capture
            && !expired
            && matches!(
                resource_status,
                PaypalPaymentStatus::Created | PaypalPaymentStatus::PartiallyCaptured
            );
        // Payments GET is one resource, not a complete current Orders ledger.
        // Neither a bare void nor a collected resource without actual Money can
        // preserve a prior complete-history certificate.
        if !is_capture
            || matches!(resource_status, PaypalPaymentStatus::Voided)
            || (collected && item.response.amount.is_none())
            || metadata.capture_context.as_ref().is_some_and(|context| {
                context.capture_method == Some(common_enums::CaptureMethod::ManualMultiple)
            })
        {
            metadata
                .capture_context
                .get_or_insert_with(PaypalCaptureContext::default)
                .capture_history_complete = Some(false);
        }
        if let Some(context) = metadata.capture_context.as_mut() {
            if matches!(resource_status, PaypalPaymentStatus::Voided)
                || expired
                || (collected && item.response.final_capture == Some(true))
            {
                context.authorization_closed = Some(true);
            }
            status = financial_sync_status(context, status, current_open).change_context(fail())?;
            if !is_capture
                && current_open
                && matches!(resource_status, PaypalPaymentStatus::PartiallyCaptured)
                && context.authorization_closed != Some(true)
            {
                // The actual authorization reports an open partial hold. This
                // proves the state, not an exact aggregate or remaining amount.
                status = common_enums::AttemptStatus::PartialChargedAndChargeable;
            } else if (collected && item.response.amount.is_none())
                || (context.capture_history_complete != Some(true)
                    && (matches!(
                        status,
                        common_enums::AttemptStatus::Charged
                            | common_enums::AttemptStatus::PartialCharged
                            | common_enums::AttemptStatus::PartialChargedAndChargeable
                            | common_enums::AttemptStatus::Voided
                    ) || (!is_capture && utils::is_payment_failure(status))))
            {
                status = common_enums::AttemptStatus::Pending;
            }
            if let Some(money) = captured_ledger_money(context).change_context(fail())? {
                captured = Some(money);
            }
        } else if matches!(resource_status, PaypalPaymentStatus::Voided) {
            // Authorization status alone does not establish zero collected money.
            status = common_enums::AttemptStatus::Pending;
        }
        captured = captured.or_else(|| {
            item.router_data
                .resource_common_data
                .amount_captured
                .clone()
        });
        let processor = item.response.processor_response.as_ref();
        let checks = processor.map(|value| {
            domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
                domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                    authentication_data: None,
                    payment_checks: Some(
                        serde_json::json!({"avs_code":value.avs_code,"cvv_code":value.cvv_code}),
                    ),
                    card_network: None,
                    domestic_network: None,
                    auth_code: None,
                },
            )
        });
        let response = if utils::is_payment_failure(status) {
            let code = processor.and_then(|value| value.response_code.clone());
            let message = code
                .as_deref()
                .and_then(get_paypal_error_message)
                .map(str::to_owned)
                .or_else(|| {
                    item.response
                        .status_details
                        .as_ref()
                        .and_then(|details| details.reason.clone())
                });
            Err(domain_types::router_data::ErrorResponse {
                code: code.clone().unwrap_or_else(|| NO_ERROR_CODE.to_owned()),
                message: message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_owned()),
                reason: message,
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: metadata
                    .order_id
                    .clone()
                    .or(Some(item.response.id.clone())),
                network_decline_code: code,
                network_advice_code: processor.and_then(|value| value.payment_advice_code.clone()),
                ..Default::default()
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    metadata
                        .order_id
                        .clone()
                        .unwrap_or_else(|| item.response.id.clone()),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(
                    merge_setup_metadata(&item.router_data.resource_common_data, &metadata)
                        .change_context(fail())?,
                ),
                network_txn_id: item
                    .response
                    .network_transaction_reference
                    .as_ref()
                    .map(|value| value.id.clone()),
                network_txn_link_id: None,
                connector_response_reference_id: item
                    .response
                    .invoice_id
                    .or_else(|| metadata.reference_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: captured,
                connector_response: checks,
                sender_payment_instrument_id: item
                    .response
                    .payer
                    .as_ref()
                    .and_then(|payer| payer.payer_id.clone()),
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
pub struct PaypalPaymentsCaptureRequest {
    amount: OrderAmount,
    final_capture: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    invoice_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    soft_descriptor: Option<String>,
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
        let metadata = capture_metadata(&item.router_data)?;
        let amount = paypal_money_amount(
            &item.router_data.request.amount_to_capture,
            item.router_data.request.currency,
        )?;
        let requested = item
            .router_data
            .request
            .amount_to_capture
            .amount
            .get_amount_as_i64();
        if requested <= 0 {
            return Err(sync_invalid("Capture amount must be positive"));
        }
        let context = metadata.capture_context.as_ref();
        let remaining = context.map(capture_remaining).transpose()?.flatten();
        if remaining.is_some_and(|remaining| requested > remaining) {
            return Err(sync_invalid(
                "Capture amount exceeds actual remaining authorized amount",
            ));
        }
        let saved_mode = context.and_then(|context| context.capture_method);
        if item
            .router_data
            .request
            .capture_method
            .zip(saved_mode)
            .is_some_and(|(requested, saved)| {
                // An ordinary PayPal authorization can be captured in multiple
                // operations. Never downgrade a saved multiple-capture operation
                // to Manual and accidentally close its remaining hold.
                requested != saved
                    && !(saved == common_enums::CaptureMethod::Manual
                        && requested == common_enums::CaptureMethod::ManualMultiple)
            })
        {
            return Err(sync_invalid(
                "Capture method conflicts with original authorization",
            ));
        }
        let mode = item.router_data.request.capture_method.or(saved_mode);
        if mode.is_some_and(|mode| {
            !matches!(
                mode,
                common_enums::CaptureMethod::Manual | common_enums::CaptureMethod::ManualMultiple
            )
        }) {
            return Err(sync_invalid(
                "Capture requires manual or manual_multiple capture method",
            ));
        }
        let multiple = item.router_data.request.multiple_capture_data.is_some()
            || mode == Some(common_enums::CaptureMethod::ManualMultiple);
        let final_capture = !multiple || remaining == Some(requested);
        // Optional operation invoice is omitted: the original order invoice remains in metadata.
        let invoice_id: Option<String> = None;
        let soft_descriptor = context.and_then(|context| context.soft_descriptor.clone());
        if invoice_id
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.chars().count() > 127)
        {
            return Err(sync_invalid(
                "Capture invoice_id must contain1 to127 characters",
            ));
        }
        if soft_descriptor
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.chars().count() > 22)
        {
            return Err(sync_invalid(
                "Capture soft_descriptor must contain1 to22 characters",
            ));
        }
        Ok(Self {
            amount,
            final_capture,
            invoice_id,
            soft_descriptor,
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
    PartiallyRefunded,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalCaptureResponse {
    id: Option<String>,
    status: Option<PaypalPaymentStatus>,
    amount: Option<OrderAmount>,
    invoice_id: Option<String>,
    final_capture: Option<bool>,
    processor_response: Option<ProcessorResponse>,
    network_transaction_reference: Option<PaypalNetworkTransactionReference>,
    status_details: Option<PaypalCaptureStatusDetails>,
    payment_source: Option<PaymentSourceItemResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalCaptureStatusDetails {
    reason: Option<String>,
}

impl From<PaypalPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: PaypalPaymentStatus) -> Self {
        match item {
            PaypalPaymentStatus::Created => Self::Authorized,
            PaypalPaymentStatus::Completed
            | PaypalPaymentStatus::Captured
            | PaypalPaymentStatus::Refunded
            | PaypalPaymentStatus::PartiallyRefunded => Self::Charged,
            PaypalPaymentStatus::Declined => Self::Failure,
            PaypalPaymentStatus::Failed => Self::CaptureFailed,
            PaypalPaymentStatus::Pending | PaypalPaymentStatus::Unknown => Self::Pending,
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
        let fail = || crate::utils::response_handling_fail_for_connector(item.http_code, "paypal");
        let mut metadata = capture_metadata(&item.router_data).change_context(fail())?;
        if item.router_data.request.capture_method
            == Some(common_enums::CaptureMethod::ManualMultiple)
            || item.router_data.request.multiple_capture_data.is_some()
        {
            metadata
                .capture_context
                .get_or_insert_with(PaypalCaptureContext::default)
                .capture_method = Some(common_enums::CaptureMethod::ManualMultiple);
        }
        if item
            .response
            .id
            .as_ref()
            .is_some_and(|id| !valid_sync_id(id))
        {
            return Err(fail().into());
        }
        let actual_id = item.response.id.as_ref();
        let state = item
            .response
            .status
            .as_ref()
            .unwrap_or(&PaypalPaymentStatus::Unknown);
        let mut status = match state {
            PaypalPaymentStatus::Completed
            | PaypalPaymentStatus::Captured
            | PaypalPaymentStatus::Refunded
            | PaypalPaymentStatus::PartiallyRefunded
                if actual_id.is_some() =>
            {
                common_enums::AttemptStatus::Charged
            }
            PaypalPaymentStatus::Declined
            | PaypalPaymentStatus::Denied
            | PaypalPaymentStatus::Expired
            | PaypalPaymentStatus::Failed => common_enums::AttemptStatus::CaptureFailed,
            PaypalPaymentStatus::PartiallyCaptured if actual_id.is_some() => {
                match item.response.final_capture {
                    Some(true) => common_enums::AttemptStatus::PartialCharged,
                    Some(false) => common_enums::AttemptStatus::PartialChargedAndChargeable,
                    None => common_enums::AttemptStatus::Pending,
                }
            }
            PaypalPaymentStatus::Created
            | PaypalPaymentStatus::Voided
            | PaypalPaymentStatus::Pending
            | PaypalPaymentStatus::Unknown
            | PaypalPaymentStatus::PartiallyCaptured
            | PaypalPaymentStatus::Completed
            | PaypalPaymentStatus::Captured
            | PaypalPaymentStatus::Refunded
            | PaypalPaymentStatus::PartiallyRefunded => common_enums::AttemptStatus::Pending,
        };
        let collected = matches!(
            status,
            common_enums::AttemptStatus::Charged
                | common_enums::AttemptStatus::PartialCharged
                | common_enums::AttemptStatus::PartialChargedAndChargeable
        );
        let amount_captured = if collected {
            item.response
                .amount
                .as_ref()
                .map(|amount| {
                    if amount.currency_code != item.router_data.request.currency {
                        return Err(Report::new(fail()));
                    }
                    Ok(Money {
                        amount: StringMajorUnitForConnector
                            .convert_back(amount.value.clone(), amount.currency_code)
                            .change_context(fail())?,
                        currency: amount.currency_code,
                    })
                })
                .transpose()?
        } else {
            None
        };
        if let Some(id) = actual_id {
            metadata.capture_id = Some(id.clone());
            if collected {
                if item.response.amount.is_none() {
                    metadata
                        .capture_context
                        .get_or_insert_with(PaypalCaptureContext::default)
                        .capture_history_complete = Some(false);
                }
                if item.response.final_capture == Some(true) {
                    metadata
                        .capture_context
                        .get_or_insert_with(PaypalCaptureContext::default)
                        .authorization_closed = Some(true);
                }
                if let (Some(context), Some(amount)) = (
                    metadata.capture_context.as_mut(),
                    item.response.amount.as_ref(),
                ) {
                    record_capture_amount(context, id, amount).change_context(fail())?;
                    if matches!(
                        state,
                        PaypalPaymentStatus::Completed | PaypalPaymentStatus::Captured
                    ) && item.response.final_capture == Some(false)
                        && context.capture_history_complete == Some(true)
                        && context.authorized_amount.is_some()
                        && context.authorization_closed != Some(true)
                    {
                        // Preserve the explicit provider nonfinal-capture witness for
                        // HS's authoritative ledger guard; absence is never openness.
                        context.authorization_closed = Some(false);
                    }
                    if item.response.final_capture == Some(true) {
                        context.authorization_closed = Some(true);
                    }
                    status = financial_sync_status(
                        context,
                        status,
                        item.response.final_capture == Some(false),
                    )
                    .change_context(fail())?;
                }
            }
        }
        if collected
            && metadata.capture_context.as_ref().is_some_and(|context| {
                context.capture_method == Some(common_enums::CaptureMethod::ManualMultiple)
                    && context.capture_history_complete != Some(true)
            })
        {
            // The operation has an actual successful capture ID, but its whole
            // payment total is unresolved. Orders sync must establish that total.
            status = common_enums::AttemptStatus::Pending;
        }
        metadata.psync_flow = PaypalPaymentIntent::Capture;
        let processor = item.response.processor_response.as_ref();
        let connector_response = processor.map(|processor| domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card { authentication_data: None, payment_checks: Some(serde_json::json!({"avs_code":processor.avs_code,"cvv_code":processor.cvv_code})), card_network:None,domestic_network:None,auth_code:None }));
        let response = if status == common_enums::AttemptStatus::CaptureFailed {
            let code = processor.and_then(|processor| processor.response_code.clone());
            let message = code
                .as_deref()
                .and_then(get_paypal_error_message)
                .map(str::to_owned)
                .or_else(|| {
                    item.response
                        .status_details
                        .as_ref()
                        .and_then(|details| details.reason.clone())
                });
            Err(domain_types::router_data::ErrorResponse {
                code: code.clone().unwrap_or_else(|| NO_ERROR_CODE.to_owned()),
                message: message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_owned()),
                reason: message,
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: metadata.order_id.clone(),
                network_decline_code: code,
                network_advice_code: processor
                    .and_then(|processor| processor.payment_advice_code.clone()),
                ..Default::default()
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item.router_data.request.connector_transaction_id.clone(),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(
                    merge_setup_metadata(&item.router_data.resource_common_data, &metadata)
                        .change_context(fail())?,
                ),
                network_txn_id: item
                    .response
                    .network_transaction_reference
                    .as_ref()
                    .map(|reference| reference.id.clone()),
                network_txn_link_id: None,
                connector_response_reference_id: item
                    .response
                    .invoice_id
                    .or_else(|| metadata.reference_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                connector_response,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalCancelStatus {
    Voided,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct PaypalVoidRequest {}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PaypalVoidRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: PaypalRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        void_metadata(&item.router_data)?;
        Ok(Self {})
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PaypalPaymentsCancelResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<PaypalCancelStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<OrderAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invoice_id: Option<String>,
}
impl TryFrom<ResponseRouterData<PaypalPaymentsCancelResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalPaymentsCancelResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let fail = || crate::utils::response_handling_fail_for_connector(item.http_code, "paypal");
        let mut metadata = void_metadata(&item.router_data).change_context(fail())?;
        if item
            .response
            .id
            .as_ref()
            .is_some_and(|id| metadata.authorize_id.as_ref() != Some(id))
        {
            return Err(fail().into());
        }
        let closed =
            item.http_code == 204 || item.response.status == Some(PaypalCancelStatus::Voided);
        let status = common_enums::AttemptStatus::Pending;
        let captured = item
            .router_data
            .resource_common_data
            .amount_captured
            .clone();
        if closed {
            let context = metadata
                .capture_context
                .get_or_insert_with(PaypalCaptureContext::default);
            context.authorization_closed = Some(true);
            // A void acknowledges closure of the hold, not the current capture
            // collection. A capture may have completed since the saved ledger.
            // Preserve observed money and reconcile the owned Order before
            // reporting a final aggregate or an uncaptured void.
            context.capture_history_complete = Some(false);
        }
        let merged = merge_setup_metadata(&item.router_data.resource_common_data, &metadata)
            .change_context(fail())?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: captured,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(merged),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item
                    .response
                    .invoice_id
                    .or_else(|| metadata.reference_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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
        setup_mandate_metadata(&item.router_data)?;
        let payment_source = match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ccard) => ZeroMandateSourceItem::Card(CardMandateRequest {
                security_code: (!ccard.card_cvc.peek().is_empty()).then_some(ccard.card_cvc.clone()),
                billing_address: get_address_info(
                    item.router_data.resource_common_data.get_optional_billing(),
                ),
                expiry: Some(ccard.get_expiry_date_as_yyyymm("-")),
                name: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_full_name(),
                // This vault request has no external-vault injector wiring. Preserve
                // raw PAN holders and refuse opaque aliases through the shared trait.
                number: Some(ccard.card_number.try_card_number("paypal")?),
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
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("Paypal"),
                    connector: "Paypal",
                    context: domain_types::errors::IntegrationErrorContext {additional_context:Some("This payment method has no direct PayPal zero-value vault request carrier".to_owned()),doc_url:Some("https://developer.paypal.com/api/payment-tokens/v3/payment-tokens-create/".to_owned()),..Default::default()},
                }))?
            }
        };

        Ok(Self {
            payment_source,
            customer: item
                .router_data
                .resource_common_data
                .customer_id
                .clone()
                .map(|id| CustomerRequestData {
                    merchant_customer_id: Some(id),
                }),
        })
    }
}

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
        mut item: ResponseRouterData<PaypalPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let fail = || crate::utils::response_handling_fail_for_connector(item.http_code, "paypal");
        let path = post_authenticate_path(&item.router_data).change_context(fail())?;
        let mut meta: PaypalMeta = serde_json::from_value(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .ok_or_else(fail)?
                .clone()
                .expose(),
        )
        .change_context(fail())?;
        if item.response.status != "APPROVED" || !valid_sync_id(&item.response.id) {
            return Err(error_stack::report!(fail()).attach_printable("PayPal original order/setup must be APPROVED before completion; completed payments require synchronization"));
        }
        if path.starts_with("v3/vault/") {
            let continuation = meta.continuation.as_mut().ok_or_else(fail)?;
            if continuation
                .setup_token_id
                .as_ref()
                .is_none_or(|id| id.peek() != &item.response.id)
            {
                return Err(
                    error_stack::report!(fail()).attach_printable("PayPal setup identity mismatch")
                );
            }
            continuation.stage = PaypalContinuationStage::SetupApproved;
        } else {
            if meta.order_id.as_ref() != Some(&item.response.id) {
                return Err(
                    error_stack::report!(fail()).attach_printable("PayPal order identity mismatch")
                );
            }
            let sources = item.response.payment_source.as_ref().ok_or_else(fail)?;
            if [
                sources.card.is_some(),
                sources.apple_pay.is_some(),
                sources.google_pay.is_some(),
                sources.paypal.is_some(),
            ]
            .into_iter()
            .filter(|present| *present)
            .count()
                != 1
            {
                return Err(error_stack::report!(fail())
                    .attach_printable("Ambiguous or absent PayPal payment source"));
            }
            let wallet = match item.router_data.request.payment_method_data.as_ref() {
                Some(PaymentMethodData::Wallet(WalletData::ApplePay(_)))
                | Some(PaymentMethodData::PaymentMethodToken(
                    domain_types::payment_method_data::PaymentMethodToken {
                        token_payment_method_type:
                            Some(domain_types::payment_method_data::TokenPaymentMethod::ApplePay),
                        ..
                    },
                )) => Some(sources.apple_pay.is_some()),
                Some(PaymentMethodData::Wallet(WalletData::GooglePay(_)))
                | Some(PaymentMethodData::PaymentMethodToken(
                    domain_types::payment_method_data::PaymentMethodToken {
                        token_payment_method_type:
                            Some(domain_types::payment_method_data::TokenPaymentMethod::GooglePay),
                        ..
                    },
                )) => Some(sources.google_pay.is_some()),
                Some(PaymentMethodData::Wallet(
                    WalletData::PaypalRedirect(_) | WalletData::PaypalSdk(_),
                )) => Some(sources.paypal.is_some()),
                _ => None,
            };
            if wallet == Some(false) {
                return Err(error_stack::report!(fail())
                    .attach_printable("Approved PayPal source differs from the original wallet"));
            }
            if let Some(card) = sources.card.as_ref() {
                if !accepted_native_authentication(&card.authentication_result) {
                    return Err(error_stack::report!(fail()).attach_printable("Rejected PayPal native authentication tuple; no monetary completion permitted"));
                }
                meta.authentication_result = Some(card.authentication_result.clone());
            } else if wallet == Some(true) {
                meta.authentication_result = None;
            } else {
                return Err(error_stack::report!(fail()).attach_printable("Missing native card authentication result; non-liability fallback is forbidden"));
            }
            let intent = meta.psync_flow.clone();
            if intent == PaypalPaymentIntent::Authenticate {
                return Err(error_stack::report!(fail())
                    .attach_printable("Original monetary intent missing"));
            }
            let expected = if matches!(
                item.router_data.request.capture_method,
                Some(
                    common_enums::CaptureMethod::Manual
                        | common_enums::CaptureMethod::ManualMultiple
                )
            ) {
                PaypalPaymentIntent::Authorize
            } else {
                PaypalPaymentIntent::Capture
            };
            if intent != expected {
                return Err(error_stack::report!(fail())
                    .attach_printable("Original capture intent changed during authentication"));
            }
            meta.continuation = Some(PaypalContinuation {
                operation: PaypalContinuationOperation::Authorize,
                stage: PaypalContinuationStage::OrderApproved,
                intent,
                setup_token_id: None,
                payment_token_id: None,
            });
        }
        let value = merge_setup_metadata(&item.router_data.resource_common_data, &meta)
            .change_context(fail())?;
        item.router_data.resource_common_data.connector_feature_data = Some(value.clone().into());
        item.router_data.resource_common_data.status =
            common_enums::AttemptStatus::AuthenticationSuccessful;
        item.router_data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: Some(value),
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: meta.reference_id,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        });
        Ok(item.router_data)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum RefundStatus {
    Completed,
    Failed,
    Cancelled,
    Pending,
    #[default]
    #[serde(other)]
    Unknown,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Completed => Self::Success,
            RefundStatus::Failed | RefundStatus::Cancelled => Self::Failure,
            RefundStatus::Pending | RefundStatus::Unknown => Self::Pending,
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OrderErrorDetails {
    pub issue: String,
    pub description: String,
    pub value: Option<Secret<String>>,
    pub field: Option<String>,
    pub location: Option<String>,
}

/// Only known schema pointers enter human diagnostics, never submitted field values.
pub fn order_error_context(detail: &OrderErrorDetails) -> String {
    const FIELDS: &[&str] = &[
        "payment_source",
        "card",
        "vault_id",
        "billing_address",
        "number",
        "expiry",
        "security_code",
        "name",
        "address_line_1",
        "address_line_2",
        "postal_code",
        "country_code",
        "admin_area_1",
        "admin_area_2",
        "stored_credential",
        "payment_initiator",
        "payment_type",
        "usage",
        "previous_transaction_reference",
        "previous_network_transaction_reference",
        "id",
        "network",
        "date",
        "purchase_units",
        "reference_id",
        "custom_id",
        "invoice_id",
        "amount",
        "value",
        "currency_code",
        "breakdown",
        "item_total",
        "tax_total",
        "shipping",
        "handling",
        "insurance",
        "shipping_discount",
        "discount",
        "items",
        "unit_amount",
        "quantity",
        "tax",
        "description",
        "soft_descriptor",
        "attributes",
        "verification",
        "method",
        "experience_context",
        "return_url",
        "cancel_url",
        "intent",
        "final_capture",
        "apple_pay",
        "google_pay",
        "decrypted_token",
        "token",
        "network_token",
    ];
    let issue = if !detail.issue.is_empty()
        && detail.issue.len() <= 64
        && detail
            .issue
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        detail.issue.as_str()
    } else {
        "UNRECOGNIZED_ISSUE"
    };
    let mut context = format!("issue - {issue}");
    if let Some(field) = detail.field.as_deref().filter(|field| {
        field.starts_with('/')
            && field.len() <= 256
            && field.split('/').skip(1).all(|segment| {
                FIELDS.contains(&segment)
                    || (!segment.is_empty()
                        && segment.len() <= 4
                        && segment.bytes().all(|b| b.is_ascii_digit()))
            })
    }) {
        context.push_str(&format!(", field - {field}"));
    }
    if let Some(location) = detail
        .location
        .as_deref()
        .filter(|v| matches!(*v, "body" | "path" | "query" | "header"))
    {
        context.push_str(&format!(", location - {location}"));
    }
    context.push(';');
    context
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_to_payer: Option<String>,
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
        refund_metadata(&item.router_data)?;
        let request = &item.router_data.request;
        if request.refund_amount.currency != request.currency
            || request.payment_amount.currency != request.currency
            || request.refund_amount.amount.get_amount_as_i64() <= 0
        {
            return Err(refund_invalid(
                "refund_amount",
                "Refund requires a positive amount in the original payment currency",
            ));
        }
        let value = item
            .connector
            .amount_converter
            .convert(request.refund_amount.amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Refund amount cannot be represented in PayPal currency units".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;
        let custom_id = Some(refund_text(&request.refund_id, "merchant_refund_id", 127)?);
        // The original order invoice is not a distinct merchant refund invoice.
        let invoice_id = None;
        let note_to_payer = request
            .reason
            .as_ref()
            .map(|value| refund_text(value, "reason", 255))
            .transpose()?;
        Ok(Self {
            custom_id,
            invoice_id,
            note_to_payer,
            amount: OrderAmount {
                currency_code: item.router_data.request.currency,
                value,
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefundResponse {
    id: Option<String>,
    #[serde(default)]
    status: RefundStatus,
    amount: Option<OrderAmount>,
    status_details: Option<PaypalCaptureStatusDetails>,
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
        let fail = || crate::utils::response_handling_fail_for_connector(item.http_code, "paypal");
        let id = item.response.id.filter(|id| valid_sync_id(id));
        let observed = item
            .response
            .amount
            .as_ref()
            .map(|amount| {
                capture_minor(amount).map(|minor| Money {
                    amount: MinorUnit::new(minor),
                    currency: amount.currency_code,
                })
            })
            .transpose();
        let invalid_amount = observed.is_err();
        let amount = observed.ok().flatten();
        let mismatch = invalid_amount
            || amount.as_ref().is_some_and(|actual| {
                actual.currency != item.router_data.request.currency
                    || actual.amount != item.router_data.request.refund_amount.amount
            });
        let raw_status = serde_json::to_value(&item.response.status)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned));
        let reason = item
            .response
            .status_details
            .and_then(|details| details.reason);
        let mut status = common_enums::RefundStatus::from(item.response.status);
        let resource_failed = status == common_enums::RefundStatus::Failure;
        if mismatch {
            status = common_enums::RefundStatus::ManualReview;
        }
        if id.is_none() {
            status = common_enums::RefundStatus::Pending;
        }
        let response = if resource_failed || id.is_none() {
            let code = reason.clone().unwrap_or_else(|| {
                if id.is_none() {
                    "REFUND_RESOURCE_ID_MISSING".to_owned()
                } else {
                    "REFUND_FAILED".to_owned()
                }
            });
            Err(domain_types::router_data::ErrorResponse {
                code: code.clone(),
                message: reason.clone().unwrap_or_else(|| code.clone()),
                reason: reason.clone(),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Refund(status)),
                connector_transaction_id: Some(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                ..Default::default()
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: id.clone().ok_or_else(fail)?,
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        };
        Ok(Self {
            resource_common_data: RefundFlowData {
                status,
                response_refund_amount: amount,
                response_connector_refund_id: id,
                raw_connector_status: Some(domain_types::connector_types::RawConnectorStatus {
                    code: raw_status,
                    message: None,
                    reason,
                }),
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefundSyncResponse {
    id: Option<String>,
    #[serde(default)]
    status: Option<RefundStatus>,
    amount: Option<OrderAmount>,
    status_details: Option<PaypalCaptureStatusDetails>,
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
        let fail = || crate::utils::response_handling_fail_for_connector(item.http_code, "paypal");
        let expected = rsync_refund_id(&item.router_data).change_context(fail())?;
        let actual = item.response.id.as_ref().filter(|id| valid_sync_id(id));
        let conflicting_id = actual.is_some_and(|id| id != &expected);
        let missing_id = actual.is_none();
        let observed = item
            .response
            .amount
            .as_ref()
            .map(|amount| {
                capture_minor(amount).map(|minor| Money {
                    amount: MinorUnit::new(minor),
                    currency: amount.currency_code,
                })
            })
            .transpose();
        let invalid_amount = observed.is_err();
        let amount = if conflicting_id || missing_id {
            None
        } else {
            observed.ok().flatten()
        };
        // Optional caller refund_money is not the only currency/gross proof:
        // corroborate a response against the actual selected capture ledger too.
        // rsync_refund_id above already validated metadata agreement/shape.
        let known_capture = item
            .router_data
            .request
            .connector_feature_data
            .as_ref()
            .or(item
                .router_data
                .resource_common_data
                .connector_feature_data
                .as_ref())
            .and_then(|value| serde_json::from_value::<PaypalMeta>(value.clone().expose()).ok())
            .and_then(|metadata| {
                let captures = metadata
                    .capture_context
                    .as_ref()?
                    .capture_amounts
                    .as_ref()?;
                let supplied = &item.router_data.request.connector_transaction_id;
                captures
                    .get(supplied)
                    .or_else(|| {
                        (metadata.order_id.as_ref() == Some(supplied))
                            .then(|| metadata.capture_id.as_ref().and_then(|id| captures.get(id)))
                            .flatten()
                    })
                    .cloned()
            });
        let known_capture = known_capture
            .as_ref()
            .map(|amount| {
                capture_minor(amount).map(|minor| Money {
                    amount: MinorUnit::new(minor),
                    currency: amount.currency_code,
                })
            })
            .transpose()
            .change_context(fail())?;
        let mismatch = invalid_amount
            || amount
                .as_ref()
                .is_some_and(|actual| actual.amount.get_amount_as_i64() <= 0)
            || amount
                .as_ref()
                .zip(known_capture.as_ref())
                .is_some_and(|(actual, capture)| {
                    actual.currency != capture.currency
                        || actual.amount.get_amount_as_i64() > capture.amount.get_amount_as_i64()
                })
            || amount
                .as_ref()
                .zip(item.router_data.request.refund_money.as_ref())
                .is_some_and(|(actual, requested)| actual != requested);
        let observed_status = item.response.status.unwrap_or_default();
        let raw_status = serde_json::to_value(&observed_status)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned));
        let reason = item
            .response
            .status_details
            .and_then(|details| details.reason);
        let mut status = common_enums::RefundStatus::from(observed_status);
        let resource_failed = status == common_enums::RefundStatus::Failure;
        if mismatch || conflicting_id {
            status = common_enums::RefundStatus::ManualReview;
        }
        if missing_id {
            status = common_enums::RefundStatus::Pending;
        }
        let response = if resource_failed || conflicting_id || missing_id {
            let (code, message) = if conflicting_id {
                ("REFUND_RESOURCE_ID_MISMATCH".to_owned(),"Refund GET returned a different resource; retain the known refund ID and reconcile".to_owned())
            } else if missing_id {
                (
                    "REFUND_RESOURCE_ID_MISSING".to_owned(),
                    "Refund GET omitted a corroborating resource ID; retry sync".to_owned(),
                )
            } else {
                let code = reason.clone().unwrap_or_else(|| "REFUND_FAILED".to_owned());
                (code.clone(), code)
            };
            Err(domain_types::router_data::ErrorResponse {
                code,
                message,
                reason: reason.clone(),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Refund(status)),
                connector_transaction_id: Some(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                ..Default::default()
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: expected.clone(),
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        };
        Ok(Self {
            resource_common_data: RefundFlowData {
                status,
                response_refund_amount: amount,
                response_connector_refund_id: Some(expected),
                raw_connector_status: Some(domain_types::connector_types::RawConnectorStatus {
                    code: raw_status,
                    message: None,
                    reason,
                }),
                ..item.router_data.resource_common_data
            },
            response,
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
        let req = &item.router_data.request;
        if req.capture_method == Some(common_enums::CaptureMethod::Scheduled) {
            return Err(IntegrationError::CaptureMethodNotSupported {
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some("PayPal recurring Orders do not support scheduled capture".to_owned()),
                    suggested_action: Some("Use automatic capture or manual authorization followed by an explicit Capture call".to_owned()),
                    doc_url: Some("https://developer.paypal.com/docs/api/orders/v2/".to_owned()),
                },
            }
            .into());
        }
        reject_external_authentication(req.authentication_data.as_ref())?;
        let intent = if req.is_auto_capture() {
            PaypalPaymentIntent::Capture
        } else {
            PaypalPaymentIntent::Authorize
        };
        let paypal_auth = PaypalAuthType::try_from(&item.router_data.connector_config)?;
        let (amount, item_details) = paypal_repeat_order_parts(&item)?;
        let reference = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        if reference.is_empty() || reference.chars().count() > 127 {
            return Err(paypal_purchase_invalid(
                "MIT invoice reference must contain1 to127 characters",
            ));
        }
        let descriptor = paypal_descriptor_value(req.billing_descriptor.as_ref());
        if descriptor
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.chars().count() > 22)
        {
            return Err(paypal_purchase_invalid(
                "MIT descriptor must contain1 to22 characters",
            ));
        }
        let shipping = ShippingAddress::from(&item);
        let purchase_units = vec![PurchaseUnitRequest {
            reference_id: Some(reference.clone()),
            custom_id: req.merchant_order_id.clone(),
            invoice_id: Some(reference),
            amount,
            payee: get_payee(&paypal_auth),
            shipping: shipping.address.is_some().then_some(shipping),
            items: item_details,
            soft_descriptor: descriptor,
            description: item.router_data.resource_common_data.description.clone(),
            supplementary_data: paypal_repeat_supplementary(&item)?,
        }];
        if req.amount.amount.get_amount_as_i64() <= 0 || req.amount.currency != req.currency {
            return Err(paypal_purchase_invalid(
                "MIT requires positive same-currency amount",
            ));
        }
        if req
            .merchant_order_id
            .as_ref()
            .is_some_and(|id| id.chars().count() > 255)
        {
            return Err(paypal_purchase_invalid(
                "MIT custom_id must contain at most255 characters",
            ));
        }
        let payment_source = Some(paypal_mit_source(&item.router_data)?);

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

#[derive(Debug, Serialize)]
pub struct PaypalWebhooksBody {
    pub event_type: PaypalWebhookEventType,
    pub resource: PaypalResource,
}

// Event type selects the resource schema. An untagged {id} refund arm would
// otherwise swallow disputes/orders and turn a capture ID into a refund ID.
impl<'de> Deserialize<'de> for PaypalWebhooksBody {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Envelope {
            event_type: PaypalWebhookEventType,
            resource: serde_json::Value,
            resource_type: Option<String>,
        }
        let envelope = Envelope::deserialize(deserializer)?;
        use PaypalWebhookEventType as E;
        let resource = match envelope.event_type {
            E::PaymentCaptureCompleted
            | E::PaymentCapturePending
            | E::PaymentCaptureDeclined
            | E::PaymentCaptureDenied
            | E::PaymentCaptureReversed
            | E::PaymentAuthorizationCreated
            | E::PaymentAuthorizationVoided => PaypalResource::PaypalCardWebhooks(Box::new(
                serde_json::from_value(envelope.resource).map_err(serde::de::Error::custom)?,
            )),
            E::CheckoutOrderCompleted | E::CheckoutOrderProcessed | E::CheckoutOrderApproved => {
                PaypalResource::PaypalRedirectsWebhooks(Box::new(
                    serde_json::from_value(envelope.resource).map_err(serde::de::Error::custom)?,
                ))
            }
            E::PaymentCaptureRefunded | E::PaymentRefundPending | E::PaymentRefundFailed => {
                if envelope.resource_type.as_deref() != Some("refund") {
                    PaypalResource::Unknown
                } else {
                    PaypalResource::PaypalRefundWebhooks(Box::new(
                        serde_json::from_value(envelope.resource)
                            .map_err(serde::de::Error::custom)?,
                    ))
                }
            }
            E::CustomerDisputeCreated | E::CustomerDisputeResolved | E::CustomerDisputedUpdated => {
                PaypalResource::PaypalDisputeWebhooks(Box::new(
                    serde_json::from_value(envelope.resource).map_err(serde::de::Error::custom)?,
                ))
            }
            _ => PaypalResource::Unknown,
        };
        Ok(Self {
            event_type: envelope.event_type,
            resource,
        })
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalWebooksEventType {
    pub event_type: PaypalWebhookEventType,
}

#[derive(Clone, Copy, Deserialize, Debug, strum::Display, Serialize, PartialEq)]
pub enum PaypalWebhookEventType {
    #[serde(rename = "PAYMENT.CAPTURE.COMPLETED")]
    PaymentCaptureCompleted,
    #[serde(rename = "PAYMENT.CAPTURE.PENDING")]
    PaymentCapturePending,
    #[serde(rename = "PAYMENT.CAPTURE.DECLINED")]
    PaymentCaptureDeclined,
    #[serde(rename = "PAYMENT.CAPTURE.DENIED")]
    PaymentCaptureDenied,
    #[serde(rename = "PAYMENT.CAPTURE.REFUNDED")]
    PaymentCaptureRefunded,
    #[serde(rename = "PAYMENT.CAPTURE.REVERSED")]
    PaymentCaptureReversed,
    #[serde(rename = "PAYMENT.REFUND.PENDING")]
    PaymentRefundPending,
    #[serde(rename = "PAYMENT.REFUND.FAILED")]
    PaymentRefundFailed,
    #[serde(rename = "PAYMENT.AUTHORIZATION.CREATED")]
    PaymentAuthorizationCreated,
    #[serde(rename = "PAYMENT.AUTHORIZATION.VOIDED")]
    PaymentAuthorizationVoided,
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

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaypalResource {
    PaypalCardWebhooks(Box<PaypalCardWebhooks>),
    PaypalRedirectsWebhooks(Box<PaypalRedirectsWebhooks>),
    PaypalRefundWebhooks(Box<PaypalRefundWebhooks>),
    PaypalDisputeWebhooks(Box<PaypalDisputeWebhooks>),
    Unknown,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalRefundWebhooks {
    pub id: String,
    pub status: Option<String>,
    pub amount: Option<OrderAmount>,
    pub status_details: Option<PaypalWebhookStatusDetails>,
}
#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalWebhookStatusDetails {
    pub reason: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalCardWebhooks {
    pub id: String,
    pub status: Option<String>,
    pub supplementary_data: Option<PaypalSupplementaryData>,
    pub amount: Option<OrderAmount>,
    pub invoice_id: Option<String>,
    pub status_details: Option<PaypalWebhookStatusDetails>,
}
#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalRedirectsWebhooks {
    #[serde(default)]
    pub purchase_units: Vec<PurchaseUnitItem>,
    pub id: String,
}
impl PaypalRedirectsWebhooks {
    pub fn webhook_observation(
        &self,
    ) -> Result<
        (common_enums::AttemptStatus, Option<Money>),
        Report<domain_types::errors::WebhookError>,
    > {
        use domain_types::errors::WebhookError;
        let mut captures = std::collections::BTreeMap::new();
        let mut has_authorization = false;
        for unit in &self.purchase_units {
            if let Some(authorizations) = &unit.payments.authorizations {
                has_authorization |= authorizations.iter().any(|item| {
                    !item.id.is_empty() && matches!(item.status, PaypalPaymentStatus::Created)
                });
            }
            if let Some(items) = &unit.payments.captures {
                for item in items {
                    if item.id.is_empty() || !matches!(item.status, PaypalPaymentStatus::Completed)
                    {
                        return Ok((common_enums::AttemptStatus::Pending, None));
                    }
                    if let Some(previous) = captures.insert(item.id.clone(), item.amount.clone()) {
                        if previous != item.amount {
                            return Err(Report::new(WebhookError::WebhookProcessingFailed)
                                .attach_printable(
                                    "Conflicting actual capture amounts in PayPal order webhook",
                                ));
                        }
                    }
                }
            }
        }
        let mut currency = None;
        let mut total = 0_i64;
        for amount in captures.values() {
            if currency.is_some_and(|value| value != amount.currency_code) {
                return Err(Report::new(WebhookError::WebhookAmountConversionFailed {
                    reason: "PayPal webhook capture currencies disagree".to_owned(),
                }));
            }
            currency = Some(amount.currency_code);
            let minor = utils::convert_back_amount_to_minor_units_for_webhook(
                &StringMajorUnitForConnector,
                amount.value.clone(),
                amount.currency_code,
            )?;
            if minor.get_amount_as_i64() < 0 {
                return Err(Report::new(WebhookError::WebhookAmountConversionFailed {
                    reason: "Negative PayPal webhook capture amount".to_owned(),
                }));
            }
            total = total
                .checked_add(minor.get_amount_as_i64())
                .filter(|value| *value >= 0)
                .ok_or_else(|| {
                    Report::new(WebhookError::WebhookAmountConversionFailed {
                        reason: "PayPal webhook capture total overflow or negative amount"
                            .to_owned(),
                    })
                })?;
        }
        if let Some(currency) = currency {
            return Ok((
                common_enums::AttemptStatus::Charged,
                Some(Money {
                    amount: MinorUnit::new(total),
                    currency,
                }),
            ));
        }
        Ok((
            if has_authorization {
                common_enums::AttemptStatus::Authorized
            } else {
                common_enums::AttemptStatus::Pending
            },
            None,
        ))
    }
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
    pub invoice_number: Option<String>,
}
#[derive(Clone, Deserialize, Debug, strum::Display, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisputeLifeCycleStage {
    Inquiry,
    Chargeback,
    PreArbitration,
    Arbitration,
}
#[derive(Deserialize, Debug, strum::Display, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisputeStatus {
    Open,
    WaitingForBuyerResponse,
    WaitingForSellerResponse,
    UnderReview,
    Resolved,
    #[serde(other)]
    Other,
}
#[derive(Deserialize, Debug, Serialize)]
pub struct DisputeOutcome {
    pub outcome_code: OutcomeCode,
}
#[derive(Clone, Copy, Deserialize, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutcomeCode {
    ResolvedBuyerFavour,
    ResolvedSellerFavour,
    ResolvedWithPayout,
    CanceledByBuyer,
    ACCEPTED,
    DENIED,
    NONE,
    #[serde(other)]
    Unknown,
}

// ----------------------------------------------------------------------------
// Webhook Source Verification
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct PaypalSourceVerificationRequest {
    pub transmission_id: String,
    pub transmission_time: String,
    pub cert_url: String,
    pub transmission_sig: Secret<String>,
    pub auth_algo: String,
    pub webhook_id: Secret<String>,
    pub webhook_event: Secret<String>,
}

impl PaypalSourceVerificationRequest {
    /// The remote postback verifier requires the exact original event bytes.
    /// Serialize only envelope metadata; never serialize the parsed event value.
    pub fn to_wire_body(&self) -> Result<Vec<u8>, Report<IntegrationError>> {
        #[derive(Serialize)]
        struct Envelope<'a> {
            transmission_id: &'a str,
            transmission_time: &'a str,
            cert_url: &'a str,
            transmission_sig: &'a Secret<String>,
            auth_algo: &'a str,
            webhook_id: &'a Secret<String>,
        }
        let encoding_error = || {
            IntegrationError::RequestEncodingFailed {
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some("Unable to encode the PayPal webhook verification envelope".to_owned()),
                suggested_action: Some("Preserve the original UTF8 JSON event and transmission metadata without rewriting the signed event".to_owned()),
                doc_url: Some("https://developer.paypal.com/api/rest/webhooks/rest/".to_owned()),
            },
        }
        };
        let envelope = Envelope {
            transmission_id: &self.transmission_id,
            transmission_time: &self.transmission_time,
            cert_url: &self.cert_url,
            transmission_sig: &self.transmission_sig,
            auth_algo: &self.auth_algo,
            webhook_id: &self.webhook_id,
        };
        let mut wire = serde_json::to_vec(&envelope).change_context(encoding_error())?;
        if wire.pop() != Some(b'}') {
            return Err(encoding_error().into());
        }
        wire.reserve(self.webhook_event.peek().len() + 18);
        wire.extend_from_slice(b",\"webhook_event\":");
        // Include leading/trailing whitespace, key order, escapes and numeric spelling.
        wire.extend_from_slice(self.webhook_event.peek().as_bytes());
        wire.push(b'}');
        Ok(wire)
    }
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
    pub access_token: Secret<String>,
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
        let context = || {
            domain_types::errors::IntegrationErrorContext {
            suggested_action:Some("Supply the configured webhook subscription ID, all five original PayPal transmission headers, original JSON event, and trusted HTTPS PayPal certificate URL".to_owned()),
            doc_url:Some("https://developer.paypal.com/api/webhooks/v1/verify-webhook-signature-post".to_owned()),
            ..Default::default()
        }
        };
        let invalid = || IntegrationError::InvalidDataFormat {
            field_name: "webhook source verification",
            context: context(),
        };
        let original_event = std::str::from_utf8(&req.webhook_body)
            .change_context(invalid())
            .attach_printable("PayPal webhook body must retain valid original UTF8 bytes")?;
        // Validation is separate from the outbound byte representation.
        let webhook_event: serde_json::Value = serde_json::from_str(original_event)
            .change_context(invalid())
            .attach_printable("PayPal webhook body must be a JSON event object")?;
        if !webhook_event.is_object()
            || webhook_event
                .get("event_type")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|v| v.trim().is_empty())
        {
            return Err(Report::new(invalid())
                .attach_printable("PayPal webhook requires an event_type in a JSON object"));
        }
        // HTTP names are case-insensitive; reject ambiguous duplicates rather than
        // allowing HashMap iteration order to choose an authenticated value.
        let headers = webhook_headers_lowercase(&req.webhook_headers);
        if headers.len() != req.webhook_headers.len() {
            return Err(Report::new(invalid())
                .attach_printable("Duplicate case-insensitive PayPal transmission header"));
        }
        let required = |name: &'static str| -> Result<String, Report<IntegrationError>> {
            headers
                .get(name)
                .filter(|value| !value.trim().is_empty() && !value.contains(['\r', '\n']))
                .cloned()
                .ok_or_else(|| {
                    Report::new(IntegrationError::MissingRequiredField {
                        field_name: name,
                        context: context(),
                    })
                })
        };
        let cert_url = required(webhook_headers::PAYPAL_CERT_URL)?;
        let cert = Url::parse(&cert_url)
            .change_context(invalid())
            .attach_printable(
                "PayPal certificate URL must be a trusted HTTPS notifications certificate",
            )?;
        if cert.scheme() != "https"
            || !matches!(
                cert.host_str(),
                Some(
                    "api.paypal.com"
                        | "api.sandbox.paypal.com"
                        | "api-m.paypal.com"
                        | "api-m.sandbox.paypal.com"
                )
            )
            || !cert.username().is_empty()
            || cert.password().is_some()
            || cert.port().is_some()
            || cert.query().is_some()
            || cert.fragment().is_some()
            || !cert
                .path()
                .strip_prefix("/v1/notifications/certs/")
                .is_some_and(|id| {
                    !id.is_empty() && id.bytes().all(|v| v.is_ascii_alphanumeric() || v == b'-')
                })
        {
            return Err(Report::new(invalid()).attach_printable(
                "PayPal certificate URL must use an allowlisted HTTPS certificate endpoint",
            ));
        }
        let webhook_id =
            String::from_utf8(req.merchant_secret.secret.clone()).change_context(invalid())?;
        if webhook_id.trim().is_empty() {
            return Err(Report::new(invalid())
                .attach_printable("A nonempty PayPal webhook subscription ID is required"));
        }
        Ok(Self {
            transmission_id: required(webhook_headers::PAYPAL_TRANSMISSION_ID)?,
            transmission_time: required(webhook_headers::PAYPAL_TRANSMISSION_TIME)?,
            cert_url,
            transmission_sig: Secret::new(required(webhook_headers::PAYPAL_TRANSMISSION_SIG)?),
            auth_algo: required(webhook_headers::PAYPAL_AUTH_ALGO)?,
            webhook_id: Secret::new(webhook_id),
            webhook_event: Secret::new(original_event.to_owned()),
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
                MerchantAuthenticationFlowData,
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
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let customer_id = item
            .router_data
            .request
            .customer
            .as_ref()
            .and_then(|c| c.customer_id.as_ref())
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
        MerchantAuthenticationFlowData,
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
                    total_price: item.router_data.request.amount.amount,
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

// Vault v3 carries encrypted Apple Pay; Orders v2 carries the resulting saved Apple source.
// https://developer.paypal.com/api/payment-tokens/v3/definitions/apple_pay_request/
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaypalAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Orders(Box<PaypalPaymentsRequest<T>>),
    AppleSetup(PaypalAppleSetupRequest),
    Exchange(PaypalTokenExchangeRequest),
    Complete(PaypalEmptyRequest),
}

#[derive(Debug, Serialize)]
pub struct PaypalEmptyRequest {}

#[derive(Debug, Serialize)]
pub struct PaypalAppleSetupRequest {
    payment_source: PaypalAppleSetupSource,
}
#[derive(Debug, Serialize)]
pub struct PaypalAppleSetupSource {
    apple_pay: PaypalEncryptedAppleToken,
}
#[derive(Debug, Serialize)]
pub struct PaypalEncryptedAppleToken {
    token: Secret<String>,
}
#[derive(Debug, Serialize)]
pub struct PaypalTokenExchangeRequest {
    payment_source: PaypalExchangeSource,
}
#[derive(Debug, Serialize)]
pub struct PaypalExchangeSource {
    token: PaypalSetupTokenReference,
}
#[derive(Debug, Serialize)]
pub struct PaypalSetupTokenReference {
    id: Secret<String>,
    #[serde(rename = "type")]
    token_type: &'static str,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PaypalAuthorizeResponse {
    Orders(PaypalAuthResponse),
    AppleSetup(PaypalAppleSetupResponse),
    AppleToken(PaypalAppleVaultTokenResponse),
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalAppleSetupResponse {
    id: Secret<String>,
    status: PaypalAppleSetupStatus,
    #[serde(default)]
    links: Vec<PaypalLinks>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaypalAppleSetupStatus {
    Created,
    PayerActionRequired,
    Approved,
    #[serde(other)]
    Unknown,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalAppleVaultTokenResponse {
    id: Secret<String>,
    customer: Option<Customer>,
    payment_source: PaypalAppleVaultTokenSource,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalAppleVaultTokenSource {
    apple_pay: PaypalVaultAppleEvidence,
}
// Require the actual Apple source object, but do not retain untyped sensitive card data in logs.
#[derive(Debug, Deserialize, Serialize)]
pub struct PaypalVaultAppleEvidence {}

pub fn paypal_authorize_meta<T: PaymentMethodDataTypes>(
    data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<Option<PaypalMeta>, Report<IntegrationError>> {
    data.resource_common_data
        .connector_feature_data
        .as_ref()
        .map(|secret| {
            let value = secret.clone().expose();
            if value.get("psync_flow").is_none() && value.get("continuation").is_none() {
                return Ok(None);
            }
            serde_json::from_value(value).map(Some).map_err(|_| {
                network_token_invalid(
                    "PayPal connector metadata is malformed; retain original continuation metadata",
                )
            })
        })
        .transpose()
        .map(Option::flatten)
}

pub fn is_encrypted_apple<T: PaymentMethodDataTypes>(
    data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> bool {
    matches!(&data.request.payment_method_data,
        PaymentMethodData::Wallet(WalletData::ApplePay(apple)) if apple.payment_data.get_encrypted_apple_pay_payment_data_optional().is_some())
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
    > for PaypalAuthorizeRequest<T>
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
        let data = &item.router_data;
        if data.request.capture_method == Some(common_enums::CaptureMethod::Scheduled) {
            return Err(IntegrationError::CaptureMethodNotSupported {
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "PayPal does not support scheduled capture".to_owned(),
                    ),
                    ..Default::default()
                },
            }
            .into());
        }
        // Apply before ALL instruments and native continuation metadata.
        // Caller-controlled order_approved metadata cannot authorize ACS import.
        reject_external_authentication(data.request.authentication_data.as_ref())?;
        if matches!(
            data.request.payment_method_data,
            PaymentMethodData::CardDetailsForNetworkTransactionId(_)
                | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        ) {
            return Err(IntegrationError::NotSupported {
                message: "Network MIT requires RecurringPaymentService/Charge with the original network mandate reference".to_owned(),
                connector: "paypal",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some("Authorize credential-only variants contain no original NTID".to_owned()), ..Default::default()
                }
            }.into());
        }
        let meta = paypal_authorize_meta(data)?;
        if let Some(continuation) = meta.as_ref().and_then(|meta| meta.continuation.as_ref()) {
            match continuation.stage {
                PaypalContinuationStage::SetupApproved => {
                    let id = continuation.setup_token_id.clone().filter(|id| !id.peek().is_empty())
                        .ok_or_else(|| network_token_invalid("Approved Apple setup continuation requires its original setup token id"))?;
                    return Ok(Self::Exchange(PaypalTokenExchangeRequest {
                        payment_source: PaypalExchangeSource {
                            token: PaypalSetupTokenReference {
                                id,
                                token_type: "SETUP_TOKEN",
                            },
                        },
                    }));
                }
                PaypalContinuationStage::PaymentTokenCreated => {
                    if continuation.operation != PaypalContinuationOperation::Authorize {
                        return Err(network_token_invalid(
                            "Setup completion must stop before creating a monetary Orders request",
                        ));
                    }
                    if continuation.intent
                        != if data.request.is_auto_capture() {
                            PaypalPaymentIntent::Capture
                        } else {
                            PaypalPaymentIntent::Authorize
                        }
                    {
                        return Err(network_token_invalid(
                            "Continuation intent differs from the original payment operation",
                        ));
                    }
                }
                PaypalContinuationStage::OrderApproved => {
                    let expected_intent = if data.request.is_auto_capture() {
                        PaypalPaymentIntent::Capture
                    } else {
                        PaypalPaymentIntent::Authorize
                    };
                    if continuation.intent != expected_intent {
                        return Err(network_token_invalid(
                            "Approved order completion must preserve the original capture intent",
                        ));
                    }
                    // PostAuthenticate corroborates a wallet's original APPROVED order and
                    // matching source. Wallet sources do not carry a card authentication tuple.
                    // Untyped tokens remain card continuations and require the complete tuple.
                    let approved_wallet = matches!(
                        &data.request.payment_method_data,
                        PaymentMethodData::Wallet(
                            WalletData::ApplePay(_)
                                | WalletData::GooglePay(_)
                                | WalletData::PaypalRedirect(_)
                                | WalletData::PaypalSdk(_)
                        ) | PaymentMethodData::PaymentMethodToken(
                            domain_types::payment_method_data::PaymentMethodToken {
                                token_payment_method_type: Some(
                                    domain_types::payment_method_data::TokenPaymentMethod::ApplePay
                                        | domain_types::payment_method_data::TokenPaymentMethod::GooglePay
                                ),
                                ..
                            }
                        )
                    );
                    if continuation.operation != PaypalContinuationOperation::Authorize
                        || (!approved_wallet
                            && meta
                                .as_ref()
                                .and_then(|meta| meta.authentication_result.as_ref())
                                .is_none_or(|result| !accepted_native_authentication(result)))
                        || data.resource_common_data.connector_order_id.is_none()
                    {
                        return Err(network_token_invalid("Order completion requires its original order and corroborated PostAuthenticate outcome"));
                    }
                    return Ok(Self::Complete(PaypalEmptyRequest {}));
                }
                PaypalContinuationStage::OrderApprovalRequired => {
                    return Err(network_token_invalid(
                        "Pending order approval must pass PostAuthenticate before charging",
                    ))
                }
                PaypalContinuationStage::SetupApprovalRequired => {
                    return Err(network_token_invalid(
                        "Resume requires authoritative approval from PostAuthenticate",
                    ))
                }
                PaypalContinuationStage::Completed => return Err(network_token_invalid(
                    "Payment continuation already completed; synchronize rather than charge again",
                )),
            }
        } else if let PaymentMethodData::Wallet(WalletData::ApplePay(apple)) =
            &data.request.payment_method_data
        {
            if let Some(token) = apple
                .payment_data
                .get_encrypted_apple_pay_payment_data_optional()
            {
                if token.is_empty() || token.len() > 10000 {
                    return Err(network_token_invalid(
                        "Encrypted Apple Pay token must have1 to10000 characters",
                    ));
                }
                return Ok(Self::AppleSetup(PaypalAppleSetupRequest {
                    payment_source: PaypalAppleSetupSource {
                        apple_pay: PaypalEncryptedAppleToken {
                            token: Secret::new(token.clone()),
                        },
                    },
                }));
            }
        }
        if matches!(
            data.request.payment_method_data,
            PaymentMethodData::Wallet(WalletData::PaypalSdk(_))
        ) || data.resource_common_data.connector_order_id.is_some()
        {
            return Ok(Self::Complete(PaypalEmptyRequest {}));
        }
        Ok(Self::Orders(Box::new(PaypalPaymentsRequest::try_from(
            item,
        )?)))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaypalAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            PaypalAuthorizeResponse::Orders(response) => {
                let previous: Option<PaypalMeta> = item
                    .router_data
                    .resource_common_data
                    .connector_feature_data
                    .as_ref()
                    .and_then(|secret| serde_json::from_value(secret.clone().expose()).ok());
                let expects_setup = is_encrypted_apple(&item.router_data)
                    && previous
                        .as_ref()
                        .and_then(|meta| meta.continuation.as_ref())
                        .is_none();
                if expects_setup {
                    let PaypalAuthResponse::PaypalThreeDsResponse(setup) = response else {
                        return Err(crate::utils::response_handling_fail_for_connector(
                            item.http_code,
                            "paypal",
                        )
                        .into());
                    };
                    let status = match setup.status {
                        PaypalOrderStatus::Approved => PaypalAppleSetupStatus::Approved,
                        PaypalOrderStatus::PayerActionRequired => {
                            PaypalAppleSetupStatus::PayerActionRequired
                        }
                        PaypalOrderStatus::Created => PaypalAppleSetupStatus::Created,
                        PaypalOrderStatus::Pending
                        | PaypalOrderStatus::Completed
                        | PaypalOrderStatus::Voided
                        | PaypalOrderStatus::Saved
                        | PaypalOrderStatus::Unknown => PaypalAppleSetupStatus::Unknown,
                    };
                    return Self::try_from(ResponseRouterData {
                        response: PaypalAuthorizeResponse::AppleSetup(PaypalAppleSetupResponse {
                            id: Secret::new(setup.id),
                            status,
                            links: setup.links,
                        }),
                        router_data: item.router_data,
                        http_code: item.http_code,
                    });
                }
                let mut result = Self::try_from(ResponseRouterData {
                    response,
                    router_data: item.router_data,
                    http_code: item.http_code,
                })?;
                if let Some(mut continuation) = previous.and_then(|meta| meta.continuation) {
                    if let Ok(PaymentsResponseData::TransactionResponse {
                        connector_metadata: Some(metadata),
                        redirection_data,
                        ..
                    }) = &mut result.response
                    {
                        continuation.stage = if redirection_data.is_some() {
                            PaypalContinuationStage::OrderApprovalRequired
                        } else {
                            PaypalContinuationStage::Completed
                        };
                        metadata["continuation"] = serde_json::to_value(continuation)
                            .change_context(crate::utils::response_handling_fail_for_connector(
                                item.http_code,
                                "paypal",
                            ))?;
                        result.resource_common_data.connector_feature_data =
                            Some(Secret::new(metadata.clone()));
                    }
                }
                Ok(result)
            }
            PaypalAuthorizeResponse::AppleSetup(response) => {
                if !is_encrypted_apple(&item.router_data) || response.id.peek().is_empty() {
                    return Err(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "paypal",
                    )
                    .into());
                }
                let (stage, status, redirect) = match response.status {
                    PaypalAppleSetupStatus::Approved => (
                        PaypalContinuationStage::SetupApproved,
                        common_enums::AttemptStatus::Pending,
                        None,
                    ),
                    PaypalAppleSetupStatus::PayerActionRequired => {
                        let link = get_setup_redirect_url(response.links)?.ok_or_else(|| {
                            crate::utils::response_handling_fail_for_connector(
                                item.http_code,
                                "paypal",
                            )
                        })?;
                        (
                            PaypalContinuationStage::SetupApprovalRequired,
                            common_enums::AttemptStatus::AuthenticationPending,
                            Some(Box::new(RedirectForm::Form {
                                endpoint: link.to_string(),
                                method: Method::Get,
                                form_fields: std::collections::HashMap::new(),
                            })),
                        )
                    }
                    PaypalAppleSetupStatus::Created | PaypalAppleSetupStatus::Unknown => {
                        return Err(ConnectorError::response_handling_failed_with_context(item.http_code,
                            Some("Apple setup token was not approved and has no actionable payer redirect".to_owned())).into());
                    }
                };
                let continuation = PaypalContinuation {
                    operation: PaypalContinuationOperation::Authorize,
                    stage,
                    intent: if item.router_data.request.is_auto_capture() {
                        PaypalPaymentIntent::Capture
                    } else {
                        PaypalPaymentIntent::Authorize
                    },
                    setup_token_id: Some(response.id.clone()),
                    payment_token_id: None,
                };
                let meta = PaypalMeta {
                    capture_context: saved_capture_context(&item.router_data.resource_common_data)
                        .or_else(|| item.router_data.request.capture_context()),
                    authorize_id: None,
                    capture_id: None,
                    incremental_authorization_id: None,
                    psync_flow: continuation.intent.clone(),
                    next_action: None,
                    order_id: None,
                    reference_id: Some(
                        item.router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    ),
                    continuation: Some(continuation),
                    authentication_result: None,
                };
                let meta = merge_setup_metadata(&item.router_data.resource_common_data, &meta)
                    .change_context(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "paypal",
                    ))?;
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_feature_data: Some(Secret::new(meta.clone())),
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(response.id.expose()),
                        redirection_data: redirect,
                        mandate_reference: None,
                        connector_metadata: Some(meta),
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
            PaypalAuthorizeResponse::AppleToken(response) => {
                let mut meta: PaypalMeta = item
                    .router_data
                    .resource_common_data
                    .connector_feature_data
                    .as_ref()
                    .and_then(|secret| serde_json::from_value(secret.clone().expose()).ok())
                    .ok_or_else(|| {
                        crate::utils::response_handling_fail_for_connector(item.http_code, "paypal")
                    })?;
                let continuation = meta.continuation.as_mut().ok_or_else(|| {
                    crate::utils::response_handling_fail_for_connector(item.http_code, "paypal")
                })?;
                if continuation.stage != PaypalContinuationStage::SetupApproved
                    || response.id.peek().is_empty()
                {
                    return Err(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "paypal",
                    )
                    .into());
                }
                continuation.payment_token_id = Some(response.id.clone());
                let setup_complete =
                    continuation.operation == PaypalContinuationOperation::SetupRecurring;
                if setup_complete && item.router_data.request.amount.amount != MinorUnit::zero() {
                    return Err(ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("Noncharging setup continuation must preserve zero amount".to_owned()),
                    )
                    .into());
                }
                continuation.stage = if setup_complete {
                    PaypalContinuationStage::Completed
                } else {
                    PaypalContinuationStage::PaymentTokenCreated
                };
                let mandate_reference = if setup_complete {
                    Some(Box::new(MandateReference {
                        connector_mandate_id: Some(response.id.clone().expose()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    }))
                } else {
                    None
                };
                let metadata = merge_setup_metadata(&item.router_data.resource_common_data, &meta)
                    .change_context(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "paypal",
                    ))?;
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: if setup_complete {
                            common_enums::AttemptStatus::Charged
                        } else {
                            common_enums::AttemptStatus::Pending
                        },
                        connector_customer: response
                            .customer
                            .map(|customer| customer.id)
                            .filter(|id| !id.trim().is_empty())
                            .or(item.router_data.resource_common_data.connector_customer),
                        amount_captured: if setup_complete {
                            None
                        } else {
                            item.router_data.resource_common_data.amount_captured
                        },
                        connector_feature_data: Some(Secret::new(metadata.clone())),
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(response.id.expose()),
                        redirection_data: None,
                        mandate_reference,
                        connector_metadata: Some(metadata),
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct PaypalPurchaseSupplementaryData {
    card: PaypalSupplementaryCard,
}
#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct PaypalSupplementaryCard {
    level_2: PaypalLevelTwo,
    level_3: PaypalLevelThree,
}
#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct PaypalLevelTwo {
    #[serde(skip_serializing_if = "Option::is_none")]
    invoice_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax_total: Option<OrderAmount>,
}
#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct PaypalLevelThree {
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_amount: Option<OrderAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duty_amount: Option<OrderAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_amount: Option<OrderAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address: Option<Address>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    line_items: Vec<PaypalLevelThreeItem>,
}
#[derive(Default, Debug, Serialize, Eq, PartialEq)]
pub struct PaypalLevelThreeItem {
    name: String,
    quantity: String,
    unit_amount: OrderAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax: Option<OrderAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_amount: Option<OrderAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_amount: Option<OrderAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit_of_measure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commodity_code: Option<String>,
}

fn paypal_purchase_invalid(message: &str) -> Report<IntegrationError> {
    IntegrationError::InvalidDataFormat {
        field_name: "order_details",
        context: domain_types::errors::IntegrationErrorContext {
            doc_url: Some("https://developer.paypal.com/docs/api/orders/v2/".to_owned()),
            additional_context: Some(message.to_owned()),
            ..Default::default()
        },
    }
    .into()
}

fn paypal_descriptor_value(
    descriptor: Option<&domain_types::connector_types::BillingDescriptor>,
) -> Option<String> {
    descriptor.and_then(|descriptor| {
        let name = descriptor
            .statement_descriptor
            .clone()
            .or_else(|| descriptor.name.as_ref().map(|name| name.peek().clone()));
        match (name, descriptor.statement_descriptor_suffix.as_ref()) {
            (Some(name), Some(suffix)) => Some(format!("{name} {suffix}")),
            (Some(name), None) => Some(name),
            (None, Some(suffix)) => Some(suffix.clone()),
            (None, None) => None,
        }
    })
}

fn paypal_descriptor<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<Option<String>, Report<IntegrationError>> {
    let descriptor = paypal_descriptor_value(request.billing_descriptor.as_ref());
    if descriptor
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.chars().count() > 22)
    {
        return Err(paypal_purchase_invalid(
            "PayPal soft descriptor must contain1 to22 characters; no truncation is performed",
        ));
    }
    Ok(descriptor)
}

fn paypal_amount(
    value: MinorUnit,
    currency: common_enums::Currency,
) -> Result<OrderAmount, Report<IntegrationError>> {
    if value.get_amount_as_i64() < 0 {
        return Err(paypal_purchase_invalid(
            "PayPal money values cannot be negative",
        ));
    }
    let value = StringMajorUnitForConnector
        .convert(value, currency)
        .change_context(IntegrationError::AmountConversionFailed {
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some(
                    "Convert checked PayPal money using its currency exponent".to_owned(),
                ),
                ..Default::default()
            },
        })?;
    Ok(OrderAmount {
        currency_code: currency,
        value,
    })
}

fn paypal_money_amount(
    money: &Money,
    currency: common_enums::Currency,
) -> Result<OrderAmount, Report<IntegrationError>> {
    if money.currency != currency {
        return Err(paypal_purchase_invalid(
            "All purchase and supplementary money must use the request currency",
        ));
    }
    paypal_amount(money.amount, currency)
}

fn paypal_order_parts<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &PaypalRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Result<(OrderRequestAmount, Vec<ItemDetails>), Report<IntegrationError>> {
    let data = &item.router_data;
    let l23 = data.resource_common_data.l2_l3_data.as_deref();
    let supplementary_tax = l23
        .and_then(|value| value.tax_info.as_ref())
        .and_then(|tax| tax.order_tax_amount.as_ref());
    let supplementary_shipping = l23
        .and_then(|value| value.order_info.as_ref())
        .and_then(|order| order.shipping_cost.as_ref());
    let tax = data.request.order_tax_amount.as_ref().or(supplementary_tax);
    let shipping = data
        .request
        .shipping_cost
        .as_ref()
        .or(supplementary_shipping);
    for (primary, supplementary) in [
        (data.request.order_tax_amount.as_ref(), supplementary_tax),
        (data.request.shipping_cost.as_ref(), supplementary_shipping),
    ] {
        if let (Some(primary), Some(supplementary)) = (primary, supplementary) {
            if primary != supplementary {
                return Err(paypal_purchase_invalid(
                    "Primary and supplementary purchase amounts disagree",
                ));
            }
        }
    }
    let discount = l23
        .and_then(|value| value.order_info.as_ref())
        .and_then(|order| order.discount_amount.as_ref());
    let minor = |money: Option<&Money>| -> Result<i64, Report<IntegrationError>> {
        money
            .map(|money| {
                paypal_money_amount(money, data.request.currency)
                    .map(|_| money.amount.get_amount_as_i64())
            })
            .transpose()
            .map(|amount| amount.unwrap_or(0))
    };
    let tax_minor = minor(tax)?;
    let shipping_minor = minor(shipping)?;
    let discount_minor = minor(discount)?;
    paypal_money_amount(&data.request.amount, data.request.currency)?;
    let total = data.request.amount.amount.get_amount_as_i64();
    let details = data
        .resource_common_data
        .order_details
        .as_ref()
        .filter(|details| !details.is_empty())
        .or_else(|| {
            l23.and_then(|value| value.order_info.as_ref())
                .and_then(|order| order.order_details.as_ref())
        });
    let (item_total, items) = if let Some(details) = details {
        if details.is_empty() || details.len() > 100 {
            return Err(paypal_purchase_invalid("PayPal item count must be1 to100"));
        }
        let mut sum = 0_i64;
        let mut tax_sum = 0_i64;
        let mut items = Vec::with_capacity(details.len());
        for detail in details {
            if detail.quantity == 0
                || detail.product_name.is_empty()
                || detail.product_name.chars().count() > 127
            {
                return Err(paypal_purchase_invalid(
                    "Each PayPal item needs a positive quantity and name of1 to127 characters",
                ));
            }
            let line = detail
                .amount
                .get_amount_as_i64()
                .checked_mul(i64::from(detail.quantity))
                .ok_or_else(|| {
                    paypal_purchase_invalid("PayPal item amount multiplication overflow")
                })?;
            if line < 0 {
                return Err(paypal_purchase_invalid(
                    "PayPal item amount cannot be negative",
                ));
            }
            sum = sum
                .checked_add(line)
                .ok_or_else(|| paypal_purchase_invalid("PayPal item sum overflow"))?;
            let line_tax = detail
                .total_tax_amount
                .map(MinorUnit::get_amount_as_i64)
                .unwrap_or(0);
            if line_tax < 0 || line_tax % i64::from(detail.quantity) != 0 {
                return Err(paypal_purchase_invalid(
                    "Item tax must be representable per unit without rounding",
                ));
            }
            tax_sum = tax_sum
                .checked_add(line_tax)
                .ok_or_else(|| paypal_purchase_invalid("PayPal item tax overflow"))?;
            items.push(ItemDetails {
                name: detail.product_name.clone(),
                quantity: detail.quantity.to_string(),
                unit_amount: paypal_amount(detail.amount, data.request.currency)?,
                tax: if line_tax == 0 {
                    None
                } else {
                    Some(paypal_amount(
                        MinorUnit::new(line_tax / i64::from(detail.quantity)),
                        data.request.currency,
                    )?)
                },
            });
        }
        if tax_sum != tax_minor {
            return Err(paypal_purchase_invalid(
                "Item taxes do not reconcile with purchase tax_total",
            ));
        }
        (sum, items)
    } else {
        let item_total = total
            .checked_sub(tax_minor)
            .and_then(|value| value.checked_sub(shipping_minor))
            .and_then(|value| value.checked_add(discount_minor))
            .filter(|value| *value >= 0)
            .ok_or_else(|| {
                paypal_purchase_invalid(
                    "Tax/shipping/discount do not reconcile with the total purchase amount",
                )
            })?;
        (
            item_total,
            vec![ItemDetails {
                name: "Purchase".to_owned(),
                quantity: "1".to_owned(),
                unit_amount: paypal_amount(MinorUnit::new(item_total), data.request.currency)?,
                tax: if tax_minor == 0 {
                    None
                } else {
                    Some(paypal_amount(
                        MinorUnit::new(tax_minor),
                        data.request.currency,
                    )?)
                },
            }],
        )
    };
    let reconciled = item_total
        .checked_add(tax_minor)
        .and_then(|value| value.checked_add(shipping_minor))
        .and_then(|value| value.checked_sub(discount_minor))
        .ok_or_else(|| paypal_purchase_invalid("Purchase amount arithmetic overflow"))?;
    if reconciled != total {
        return Err(paypal_purchase_invalid(
            "PayPal items+tax+shipping-discount must equal the original request total",
        ));
    }
    Ok((
        OrderRequestAmount {
            currency_code: data.request.currency,
            value: paypal_money_amount(&data.request.amount, data.request.currency)?.value,
            breakdown: AmountBreakdown {
                item_total: paypal_amount(MinorUnit::new(item_total), data.request.currency)?,
                tax_total: tax
                    .map(|money| paypal_money_amount(money, data.request.currency))
                    .transpose()?,
                shipping: shipping
                    .map(|money| paypal_money_amount(money, data.request.currency))
                    .transpose()?,
                discount: discount
                    .map(|money| paypal_money_amount(money, data.request.currency))
                    .transpose()?,
            },
        },
        items,
    ))
}

fn paypal_supplementary<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &PaypalRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Result<Option<PaypalPurchaseSupplementaryData>, Report<IntegrationError>> {
    let data = &item.router_data;
    let Some(l23) = data.resource_common_data.l2_l3_data.as_deref() else {
        return Ok(None);
    };
    let order = l23.order_info.as_ref();
    let tax = l23.tax_info.as_ref();
    if order
        .and_then(|order| order.merchant_order_reference_id.as_ref())
        .is_some_and(|id| id.chars().count() > 127)
    {
        return Err(paypal_purchase_invalid(
            "Level2 invoice_id must contain at most127 characters",
        ));
    }
    let currency = data.request.currency;
    let convert = |money: Option<&Money>| {
        money
            .map(|money| paypal_money_amount(money, currency))
            .transpose()
    };
    let details = order
        .and_then(|order| order.order_details.as_ref())
        .or(data.resource_common_data.order_details.as_ref());
    let line_items = details
        .map(|details| {
            details
                .iter()
                .map(|detail| {
                    Ok(PaypalLevelThreeItem {
                        name: detail.product_name.clone(),
                        quantity: detail.quantity.to_string(),
                        unit_amount: paypal_amount(detail.amount, currency)?,
                        tax: detail
                            .total_tax_amount
                            .map(|amount| paypal_amount(amount, currency))
                            .transpose()?,
                        discount_amount: detail
                            .unit_discount_amount
                            .map(|amount| paypal_amount(amount, currency))
                            .transpose()?,
                        total_amount: detail
                            .total_amount
                            .map(|amount| paypal_amount(amount, currency))
                            .transpose()?,
                        description: detail.description.clone(),
                        unit_of_measure: detail.unit_of_measure.clone(),
                        commodity_code: detail.commodity_code.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Report<IntegrationError>>>()
        })
        .transpose()?;
    // Absent optional Level3 lines stay absent on the wire (skip_serializing_if).
    let line_items = line_items.into_iter().flatten().collect();
    Ok(Some(PaypalPurchaseSupplementaryData {
        card: PaypalSupplementaryCard {
            level_2: PaypalLevelTwo {
                invoice_id: order
                    .and_then(|order| order.merchant_order_reference_id.clone())
                    .or_else(|| {
                        (data
                            .resource_common_data
                            .connector_request_reference_id
                            .chars()
                            .count()
                            <= 127)
                            .then(|| {
                                data.resource_common_data
                                    .connector_request_reference_id
                                    .clone()
                            })
                    }),
                tax_total: convert(
                    tax.and_then(|tax| tax.order_tax_amount.as_ref())
                        .or(data.request.order_tax_amount.as_ref()),
                )?,
            },
            level_3: PaypalLevelThree {
                shipping_amount: convert(
                    order
                        .and_then(|order| order.shipping_cost.as_ref())
                        .or(data.request.shipping_cost.as_ref()),
                )?,
                duty_amount: convert(order.and_then(|order| order.duty_amount.as_ref()))?,
                discount_amount: convert(order.and_then(|order| order.discount_amount.as_ref()))?,
                shipping_address: get_address_info(
                    data.resource_common_data.get_optional_shipping(),
                ),
                line_items,
            },
        },
    }))
}

fn paypal_repeat_order_parts<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &PaypalRouterData<
        RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        T,
    >,
) -> Result<(OrderRequestAmount, Vec<ItemDetails>), Report<IntegrationError>> {
    let data = &item.router_data;
    let l23 = data.resource_common_data.l2_l3_data.as_deref();
    let supplementary_tax = l23
        .and_then(|value| value.tax_info.as_ref())
        .and_then(|tax| tax.order_tax_amount.as_ref());
    let supplementary_shipping = l23
        .and_then(|value| value.order_info.as_ref())
        .and_then(|order| order.shipping_cost.as_ref());
    let tax = None::<&Money>.or(supplementary_tax);
    let shipping = data
        .request
        .shipping_cost
        .as_ref()
        .or(supplementary_shipping);
    for (primary, supplementary) in [
        (None::<&Money>, supplementary_tax),
        (data.request.shipping_cost.as_ref(), supplementary_shipping),
    ] {
        if let (Some(primary), Some(supplementary)) = (primary, supplementary) {
            if primary != supplementary {
                return Err(paypal_purchase_invalid(
                    "Primary and supplementary purchase amounts disagree",
                ));
            }
        }
    }
    let discount = l23
        .and_then(|value| value.order_info.as_ref())
        .and_then(|order| order.discount_amount.as_ref());
    let minor = |money: Option<&Money>| -> Result<i64, Report<IntegrationError>> {
        money
            .map(|money| {
                paypal_money_amount(money, data.request.currency)
                    .map(|_| money.amount.get_amount_as_i64())
            })
            .transpose()
            .map(|amount| amount.unwrap_or(0))
    };
    let tax_minor = minor(tax)?;
    let shipping_minor = minor(shipping)?;
    let discount_minor = minor(discount)?;
    paypal_money_amount(&data.request.amount, data.request.currency)?;
    let total = data.request.amount.amount.get_amount_as_i64();
    let details = data
        .resource_common_data
        .order_details
        .as_ref()
        .filter(|details| !details.is_empty())
        .or_else(|| {
            l23.and_then(|value| value.order_info.as_ref())
                .and_then(|order| order.order_details.as_ref())
        });
    let (item_total, items) = if let Some(details) = details {
        if details.is_empty() || details.len() > 100 {
            return Err(paypal_purchase_invalid("PayPal item count must be1 to100"));
        }
        let mut sum = 0_i64;
        let mut tax_sum = 0_i64;
        let mut items = Vec::with_capacity(details.len());
        for detail in details {
            if detail.quantity == 0
                || detail.product_name.is_empty()
                || detail.product_name.chars().count() > 127
            {
                return Err(paypal_purchase_invalid(
                    "Each PayPal item needs a positive quantity and name of1 to127 characters",
                ));
            }
            let line = detail
                .amount
                .get_amount_as_i64()
                .checked_mul(i64::from(detail.quantity))
                .ok_or_else(|| {
                    paypal_purchase_invalid("PayPal item amount multiplication overflow")
                })?;
            if line < 0 {
                return Err(paypal_purchase_invalid(
                    "PayPal item amount cannot be negative",
                ));
            }
            sum = sum
                .checked_add(line)
                .ok_or_else(|| paypal_purchase_invalid("PayPal item sum overflow"))?;
            let line_tax = detail
                .total_tax_amount
                .map(MinorUnit::get_amount_as_i64)
                .unwrap_or(0);
            if line_tax < 0 || line_tax % i64::from(detail.quantity) != 0 {
                return Err(paypal_purchase_invalid(
                    "Item tax must be representable per unit without rounding",
                ));
            }
            tax_sum = tax_sum
                .checked_add(line_tax)
                .ok_or_else(|| paypal_purchase_invalid("PayPal item tax overflow"))?;
            items.push(ItemDetails {
                name: detail.product_name.clone(),
                quantity: detail.quantity.to_string(),
                unit_amount: paypal_amount(detail.amount, data.request.currency)?,
                tax: if line_tax == 0 {
                    None
                } else {
                    Some(paypal_amount(
                        MinorUnit::new(line_tax / i64::from(detail.quantity)),
                        data.request.currency,
                    )?)
                },
            });
        }
        if tax_sum != tax_minor {
            return Err(paypal_purchase_invalid(
                "Item taxes do not reconcile with purchase tax_total",
            ));
        }
        (sum, items)
    } else {
        let item_total = total
            .checked_sub(tax_minor)
            .and_then(|value| value.checked_sub(shipping_minor))
            .and_then(|value| value.checked_add(discount_minor))
            .filter(|value| *value >= 0)
            .ok_or_else(|| {
                paypal_purchase_invalid(
                    "Tax/shipping/discount do not reconcile with the total purchase amount",
                )
            })?;
        (
            item_total,
            vec![ItemDetails {
                name: "Purchase".to_owned(),
                quantity: "1".to_owned(),
                unit_amount: paypal_amount(MinorUnit::new(item_total), data.request.currency)?,
                tax: if tax_minor == 0 {
                    None
                } else {
                    Some(paypal_amount(
                        MinorUnit::new(tax_minor),
                        data.request.currency,
                    )?)
                },
            }],
        )
    };
    let reconciled = item_total
        .checked_add(tax_minor)
        .and_then(|value| value.checked_add(shipping_minor))
        .and_then(|value| value.checked_sub(discount_minor))
        .ok_or_else(|| paypal_purchase_invalid("Purchase amount arithmetic overflow"))?;
    if reconciled != total {
        return Err(paypal_purchase_invalid(
            "PayPal items+tax+shipping-discount must equal the original request total",
        ));
    }
    Ok((
        OrderRequestAmount {
            currency_code: data.request.currency,
            value: paypal_money_amount(&data.request.amount, data.request.currency)?.value,
            breakdown: AmountBreakdown {
                item_total: paypal_amount(MinorUnit::new(item_total), data.request.currency)?,
                tax_total: tax
                    .map(|money| paypal_money_amount(money, data.request.currency))
                    .transpose()?,
                shipping: shipping
                    .map(|money| paypal_money_amount(money, data.request.currency))
                    .transpose()?,
                discount: discount
                    .map(|money| paypal_money_amount(money, data.request.currency))
                    .transpose()?,
            },
        },
        items,
    ))
}

fn paypal_repeat_supplementary<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &PaypalRouterData<
        RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        T,
    >,
) -> Result<Option<PaypalPurchaseSupplementaryData>, Report<IntegrationError>> {
    let data = &item.router_data;
    let Some(l23) = data.resource_common_data.l2_l3_data.as_deref() else {
        return Ok(None);
    };
    let order = l23.order_info.as_ref();
    let tax = l23.tax_info.as_ref();
    if order
        .and_then(|order| order.merchant_order_reference_id.as_ref())
        .is_some_and(|id| id.chars().count() > 127)
    {
        return Err(paypal_purchase_invalid(
            "Level2 invoice_id must contain at most127 characters",
        ));
    }
    let currency = data.request.currency;
    let convert = |money: Option<&Money>| {
        money
            .map(|money| paypal_money_amount(money, currency))
            .transpose()
    };
    let details = order
        .and_then(|order| order.order_details.as_ref())
        .or(data.resource_common_data.order_details.as_ref());
    let line_items = details
        .map(|details| {
            details
                .iter()
                .map(|detail| {
                    Ok(PaypalLevelThreeItem {
                        name: detail.product_name.clone(),
                        quantity: detail.quantity.to_string(),
                        unit_amount: paypal_amount(detail.amount, currency)?,
                        tax: detail
                            .total_tax_amount
                            .map(|amount| paypal_amount(amount, currency))
                            .transpose()?,
                        discount_amount: detail
                            .unit_discount_amount
                            .map(|amount| paypal_amount(amount, currency))
                            .transpose()?,
                        total_amount: detail
                            .total_amount
                            .map(|amount| paypal_amount(amount, currency))
                            .transpose()?,
                        description: detail.description.clone(),
                        unit_of_measure: detail.unit_of_measure.clone(),
                        commodity_code: detail.commodity_code.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Report<IntegrationError>>>()
        })
        .transpose()?;
    // Absent optional Level3 lines stay absent on the wire (skip_serializing_if).
    let line_items = line_items.into_iter().flatten().collect();
    Ok(Some(PaypalPurchaseSupplementaryData {
        card: PaypalSupplementaryCard {
            level_2: PaypalLevelTwo {
                invoice_id: order
                    .and_then(|order| order.merchant_order_reference_id.clone())
                    .or_else(|| {
                        (data
                            .resource_common_data
                            .connector_request_reference_id
                            .chars()
                            .count()
                            <= 127)
                            .then(|| {
                                data.resource_common_data
                                    .connector_request_reference_id
                                    .clone()
                            })
                    }),
                tax_total: convert(
                    tax.and_then(|tax| tax.order_tax_amount.as_ref())
                        .or(None::<&Money>),
                )?,
            },
            level_3: PaypalLevelThree {
                shipping_amount: convert(
                    order
                        .and_then(|order| order.shipping_cost.as_ref())
                        .or(data.request.shipping_cost.as_ref()),
                )?,
                duty_amount: convert(order.and_then(|order| order.duty_amount.as_ref()))?,
                discount_amount: convert(order.and_then(|order| order.discount_amount.as_ref()))?,
                shipping_address: get_address_info(
                    data.resource_common_data.get_optional_shipping(),
                ),
                line_items,
            },
        },
    }))
}

fn validate_google_signed_fields(
    data: &domain_types::payment_method_data::GooglePayDecryptedData,
) -> Result<(), Report<IntegrationError>> {
    if let Some(expiration) = data.message_expiration.as_ref() {
        if expiration.len() != 13 || !expiration.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(network_token_invalid(
                "Google message_expiration must be13 ASCII digits in UTC milliseconds",
            ));
        }
        let expiration = expiration
            .parse::<i64>()
            .map_err(|_| network_token_invalid("Google message_expiration is invalid"))?;
        if expiration <= common_utils::date_time::now_unix_millis() {
            return Err(network_token_invalid(
                "The original signed Google Pay message has expired",
            ));
        }
    }
    if data
        .message_id
        .as_ref()
        .is_some_and(|id| id.is_empty() || id.chars().count() > 250)
    {
        return Err(network_token_invalid(
            "Google message_id must have1 to250 characters when supplied",
        ));
    }
    Ok(())
}

/// Each permitted native result is corroborated by enrollment, authentication and liability.
pub fn accepted_native_authentication(result: &PaypalThreeDsParams) -> bool {
    matches!(
        (
            &result.three_d_secure.enrollment_status,
            &result.three_d_secure.authentication_status,
            &result.liability_shift
        ),
        (
            Some(EnrollmentStatus::Ready),
            Some(AuthenticationStatus::Success | AuthenticationStatus::Attempted),
            LiabilityShift::Possible
        ) | (
            Some(
                EnrollmentStatus::NotReady
                    | EnrollmentStatus::Unavailable
                    | EnrollmentStatus::Bypassed
            ),
            None,
            LiabilityShift::No
        )
    )
}

fn sync_invalid(message: &str) -> Report<IntegrationError> {
    IntegrationError::InvalidDataFormat {
        field_name: "connector_feature_data",
        context: domain_types::errors::IntegrationErrorContext {
            additional_context: Some(message.to_owned()),
            suggested_action: Some(
                "Retain the original PayPal order and resource metadata from the payment response"
                    .to_owned(),
            ),
            doc_url: Some("https://developer.paypal.com/docs/api/payments/v2/".to_owned()),
        },
    }
    .into()
}

pub fn sync_metadata(
    data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
) -> Result<PaypalMeta, Report<IntegrationError>> {
    let request = data
        .request
        .connector_feature_data
        .as_ref()
        .map(|value| value.peek());
    let common = data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .map(|value| value.peek());
    if request.zip(common).is_some_and(|(a, b)| a != b) {
        return Err(sync_invalid("Request and payment metadata disagree"));
    }
    let value = request
        .or(common)
        .ok_or_else(|| sync_invalid("PSync requires the original resource metadata"))?;
    serde_json::from_value(value.clone())
        .map_err(|_| sync_invalid("PayPal sync metadata is malformed or has an unsupported intent"))
}

// Vault GET resources are parsed only after the saved continuation chooses the target.
// They deliberately do not participate in the untagged Orders response union.
#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalVaultSyncCustomer {
    pub id: Secret<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalSetupTokenSyncResponse {
    pub id: Secret<String>,
    pub status: String,
    pub customer: Option<PaypalVaultSyncCustomer>,
    #[serde(default)]
    pub links: Vec<PaypalLinks>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PaypalPaymentTokenSyncResponse {
    pub id: Secret<String>,
    pub customer: Option<PaypalVaultSyncCustomer>,
}
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaypalVaultSyncResponse {
    Setup(PaypalSetupTokenSyncResponse),
    Payment(PaypalPaymentTokenSyncResponse),
}
impl TryFrom<ResponseRouterData<PaypalVaultSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalVaultSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let fail = || crate::utils::response_handling_fail_for_connector(item.http_code, "paypal");
        let mut meta = sync_metadata(&item.router_data).change_context(fail())?;
        let target = sync_target(&item.router_data, &meta).change_context(fail())?;
        let mut redirect = None;
        let mut status = common_enums::AttemptStatus::Pending;
        let (id, customer) = match (target, item.response) {
            (PaypalSyncTarget::SetupToken(expected), PaypalVaultSyncResponse::Setup(response))
                if response.id.peek() == &expected =>
            {
                match response.status.as_str() {
                    "CREATED" | "VAULTED" | "TOKENIZED" => {}
                    "APPROVED" => {
                        meta.continuation.as_mut().ok_or_else(fail)?.stage =
                            PaypalContinuationStage::SetupApproved;
                    }
                    "PAYER_ACTION_REQUIRED" => {
                        if let Some(link) = get_setup_redirect_url(response.links)? {
                            status = common_enums::AttemptStatus::AuthenticationPending;
                            redirect = Some(Box::new(RedirectForm::Form {
                                endpoint: link.to_string(),
                                method: Method::Get,
                                form_fields: std::collections::HashMap::new(),
                            }));
                        }
                    }
                    _ => return Err(fail().into()),
                }
                (response.id, response.customer)
            }
            (
                PaypalSyncTarget::PaymentToken(expected),
                PaypalVaultSyncResponse::Payment(response),
            ) if response.id.peek() == &expected => (response.id, response.customer),
            _ => return Err(fail().into()),
        };
        if let Some(customer) = customer {
            if customer.id.peek().trim().is_empty()
                || item
                    .router_data
                    .resource_common_data
                    .connector_customer
                    .as_ref()
                    .filter(|saved| !saved.trim().is_empty())
                    .is_some_and(|saved| saved != customer.id.peek())
            {
                return Err(fail().into());
            }
        }
        let value = merge_setup_metadata(&item.router_data.resource_common_data, &meta)
            .change_context(fail())?;
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured: None,
                connector_feature_data: Some(Secret::new(value.clone())),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(id.expose()),
                redirection_data: redirect,
                mandate_reference: None,
                connector_metadata: Some(value),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            ..item.router_data
        })
    }
}

pub enum PaypalSyncTarget {
    Order(String),
    Authorization(String),
    Capture(String),
    SetupToken(String),
    PaymentToken(String),
}
impl PaypalSyncTarget {
    pub fn path(&self) -> String {
        match self {
            Self::Order(id) => format!("v2/checkout/orders/{id}"),
            Self::Authorization(id) => format!("v2/payments/authorizations/{id}"),
            Self::Capture(id) => format!("v2/payments/captures/{id}"),
            Self::SetupToken(id) => format!("v3/vault/setup-tokens/{id}"),
            Self::PaymentToken(id) => format!("v3/vault/payment-tokens/{id}"),
        }
    }
}
fn valid_sync_id(id: &str) -> bool {
    !id.is_empty()
        && id.trim() == id
        && !id.bytes().any(|value| {
            value.is_ascii_control()
                || value.is_ascii_whitespace()
                || matches!(value, b'/' | b'?' | b'#' | b'%')
        })
}
pub fn sync_target(
    data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    metadata: &PaypalMeta,
) -> Result<PaypalSyncTarget, Report<IntegrationError>> {
    if metadata.continuation.as_ref().is_some_and(|continuation| {
        continuation.operation == PaypalContinuationOperation::SetupRecurring
    }) {
        return Err(sync_invalid("Noncharging vault setup is not an Orders payment resource; reuse the completed mandate instead"));
    }
    let supplied = data
        .request
        .connector_transaction_id
        .get_connector_transaction_id()
        .change_context(IntegrationError::MissingConnectorTransactionID {
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some(
                    "PSync requires the original payment or saved resource id".to_owned(),
                ),
                ..Default::default()
            },
        })?;
    if let Some(continuation) = metadata.continuation.as_ref().filter(|value| {
        matches!(
            value.stage,
            PaypalContinuationStage::SetupApprovalRequired
                | PaypalContinuationStage::SetupApproved
                | PaypalContinuationStage::PaymentTokenCreated
        )
    }) {
        let financial_context = metadata.capture_context.as_ref().is_some_and(|context| {
            context.authorization_closed == Some(true)
                || context
                    .capture_amounts
                    .as_ref()
                    .is_some_and(|amounts| !amounts.is_empty())
        });
        if continuation.operation != PaypalContinuationOperation::Authorize
            || metadata.order_id.is_some()
            || metadata.authorize_id.is_some()
            || metadata.capture_id.is_some()
            || metadata.incremental_authorization_id.is_some()
            || metadata.authentication_result.is_some()
            || financial_context
            || data
                .resource_common_data
                .amount_captured
                .as_ref()
                .is_some_and(|money| money.amount.get_amount_as_i64() != 0)
        {
            return Err(sync_invalid(
                "Vault continuation conflicts with financial resource history",
            ));
        }
        let (id, payment) = match continuation.stage {
            PaypalContinuationStage::SetupApprovalRequired
            | PaypalContinuationStage::SetupApproved => {
                if continuation.payment_token_id.is_some() {
                    return Err(sync_invalid(
                        "Setup continuation conflicts with a payment token",
                    ));
                }
                (continuation.setup_token_id.as_ref(), false)
            }
            PaypalContinuationStage::PaymentTokenCreated => {
                (continuation.payment_token_id.as_ref(), true)
            }
            _ => unreachable!(),
        };
        let id = id
            .map(|value| value.peek())
            .filter(|id| valid_sync_id(id) && id.is_ascii() && !id.contains('\\'))
            .ok_or_else(|| {
                sync_invalid("Vault continuation requires its exact safe saved token id")
            })?;
        if supplied != *id {
            return Err(sync_invalid(
                "Supplied token id conflicts with saved vault continuation",
            ));
        }
        return Ok(if payment {
            PaypalSyncTarget::PaymentToken(id.clone())
        } else {
            PaypalSyncTarget::SetupToken(id.clone())
        });
    }
    if !valid_sync_id(&supplied)
        || [
            metadata.order_id.as_ref(),
            metadata.authorize_id.as_ref(),
            metadata.capture_id.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|id| !valid_sync_id(id))
    {
        return Err(sync_invalid(
            "PayPal resource identifiers must be nonempty, unescaped path segments",
        ));
    }
    // A payment-level sync must observe the complete order ledger, including a
    // capture whose response was lost before its ID could be persisted locally.
    // A closed authorization still needs aggregate reconciliation for webhooks.
    if metadata.capture_context.as_ref().is_some_and(|context| {
        matches!(
            context.capture_method,
            Some(common_enums::CaptureMethod::ManualMultiple | common_enums::CaptureMethod::Manual)
        )
    }) {
        if let Some(order) = metadata.order_id.as_ref() {
            if ![
                Some(order),
                metadata.authorize_id.as_ref(),
                metadata.capture_id.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|id| id == &supplied)
            {
                return Err(sync_invalid(
                    "Supplied payment id conflicts with canonical order resources",
                ));
            }
            return Ok(PaypalSyncTarget::Order(order.clone()));
        }
    }
    if metadata.capture_context.as_ref().is_some_and(|context| {
        context.capture_method == Some(common_enums::CaptureMethod::ManualMultiple)
            && context.authorization_closed != Some(true)
    }) && metadata.capture_id.is_some()
    {
        if let Some(authorization) = metadata.authorize_id.as_ref() {
            if ![
                metadata.order_id.as_ref(),
                metadata.capture_id.as_ref(),
                Some(authorization),
            ]
            .into_iter()
            .flatten()
            .any(|id| id == &supplied)
            {
                return Err(sync_invalid(
                    "Supplied payment id conflicts with original authorization/capture",
                ));
            }
            return Ok(PaypalSyncTarget::Authorization(authorization.clone()));
        }
    }
    let target = match metadata.psync_flow {
        PaypalPaymentIntent::Capture => metadata.capture_id.clone().map(PaypalSyncTarget::Capture),
        PaypalPaymentIntent::Authorize if metadata.capture_id.is_none() => metadata
            .authorize_id
            .clone()
            .map(PaypalSyncTarget::Authorization),
        PaypalPaymentIntent::Authenticate
            if metadata.capture_id.is_none() && metadata.authorize_id.is_none() =>
        {
            None
        }
        PaypalPaymentIntent::Authorize | PaypalPaymentIntent::Authenticate => {
            return Err(sync_invalid(
                "Sync intent conflicts with settled resource ids",
            ))
        }
    };
    if let Some(target) = target {
        let resource = match &target {
            PaypalSyncTarget::Capture(id) | PaypalSyncTarget::Authorization(id) => id,
            PaypalSyncTarget::Order(id)
            | PaypalSyncTarget::SetupToken(id)
            | PaypalSyncTarget::PaymentToken(id) => id,
        };
        if metadata
            .order_id
            .as_ref()
            .is_some_and(|order| &supplied != order && &supplied != resource)
        {
            return Err(sync_invalid(
                "Supplied payment id conflicts with original order/resource",
            ));
        }
        return Ok(target);
    }
    if metadata.continuation.as_ref().is_some_and(|value| {
        matches!(
            value.stage,
            PaypalContinuationStage::SetupApprovalRequired
                | PaypalContinuationStage::SetupApproved
                | PaypalContinuationStage::PaymentTokenCreated
        )
    }) {
        return Err(sync_invalid("Vault setup is not an Orders resource; resume its verified setup continuation before payment sync"));
    }
    if metadata
        .order_id
        .as_ref()
        .is_some_and(|order| order != &supplied)
    {
        return Err(sync_invalid(
            "Pending order id conflicts with saved order identity",
        ));
    }
    Ok(PaypalSyncTarget::Order(
        metadata.order_id.clone().unwrap_or(supplied),
    ))
}

fn validate_sync_order_identity(
    data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    metadata: &PaypalMeta,
    actual: &str,
    http_code: u16,
) -> Result<(), Report<ConnectorError>> {
    let expected = sync_target(data, metadata).change_context(
        crate::utils::response_handling_fail_for_connector(http_code, "paypal"),
    )?;
    match expected {
        PaypalSyncTarget::Order(id) if id == actual => Ok(()),
        PaypalSyncTarget::Order(_)
        | PaypalSyncTarget::SetupToken(_)
        | PaypalSyncTarget::PaymentToken(_)
        | PaypalSyncTarget::Authorization(_)
        | PaypalSyncTarget::Capture(_) => {
            Err(crate::utils::response_handling_fail_for_connector(http_code, "paypal").into())
        }
    }
}
fn sync_order_state(
    data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    mut metadata: PaypalMeta,
    id: String,
    order_status: PaypalOrderStatus,
    http_code: u16,
) -> Result<
    RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    Report<ConnectorError>,
> {
    validate_sync_order_identity(&data, &metadata, &id, http_code)?;
    metadata.order_id = Some(id.clone());
    let mut status = match order_status {
        PaypalOrderStatus::Approved => common_enums::AttemptStatus::AuthenticationSuccessful,
        PaypalOrderStatus::PayerActionRequired => {
            common_enums::AttemptStatus::AuthenticationPending
        }
        PaypalOrderStatus::Voided => common_enums::AttemptStatus::Voided,
        PaypalOrderStatus::Completed
        | PaypalOrderStatus::Created
        | PaypalOrderStatus::Saved
        | PaypalOrderStatus::Pending
        | PaypalOrderStatus::Unknown => common_enums::AttemptStatus::Pending,
    };
    let mut captured = None;
    if let Some(context) = metadata.capture_context.as_mut() {
        // Every status-only Orders observation lacks the current resource ledger;
        // COMPLETED alone cannot certify an outstanding hold has been closed.
        context.capture_history_complete = Some(false);
        if matches!(order_status, PaypalOrderStatus::Voided) {
            context.authorization_closed = Some(true);
            // This response has no capture collection. A pre-capture snapshot
            // cannot prove that no capture occurred while its response was lost.
            context.capture_history_complete = Some(false);
        }
        status = financial_sync_status(context, status, false).change_context(
            crate::utils::response_handling_fail_for_connector(http_code, "paypal"),
        )?;
        captured = captured_ledger_money(context)
            .change_context(crate::utils::response_handling_fail_for_connector(
                http_code, "paypal",
            ))?
            .or_else(|| data.resource_common_data.amount_captured.clone());
    } else if matches!(order_status, PaypalOrderStatus::Voided) {
        status = common_enums::AttemptStatus::Pending;
    }
    // A bare COMPLETED order contains no collected-resource amount oracle. The
    // full Orders variant above maps the actual nested authorization/capture.
    let connector_metadata = merge_setup_metadata(&data.resource_common_data, &metadata)
        .change_context(crate::utils::response_handling_fail_for_connector(
            http_code, "paypal",
        ))?;
    Ok(RouterDataV2 {
        resource_common_data: PaymentFlowData {
            status,
            amount_captured: captured,
            ..data.resource_common_data
        },
        response: Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: Some(connector_metadata),
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
        }),
        ..data
    })
}

fn paypal_resource_status(
    status: &PaypalPaymentStatus,
    is_capture: bool,
) -> common_enums::AttemptStatus {
    match status {
        PaypalPaymentStatus::Created => match is_capture {
            true => common_enums::AttemptStatus::Pending,
            false => common_enums::AttemptStatus::Authorized,
        },
        PaypalPaymentStatus::Failed => match is_capture {
            true => common_enums::AttemptStatus::CaptureFailed,
            false => common_enums::AttemptStatus::Failure,
        },
        PaypalPaymentStatus::Captured
        | PaypalPaymentStatus::Completed
        | PaypalPaymentStatus::Refunded
        | PaypalPaymentStatus::PartiallyRefunded => common_enums::AttemptStatus::Charged,
        PaypalPaymentStatus::Declined
        | PaypalPaymentStatus::Denied
        | PaypalPaymentStatus::Expired => common_enums::AttemptStatus::Failure,
        PaypalPaymentStatus::PartiallyCaptured => common_enums::AttemptStatus::PartialCharged,
        PaypalPaymentStatus::Voided => common_enums::AttemptStatus::Voided,
        PaypalPaymentStatus::Pending | PaypalPaymentStatus::Unknown => {
            common_enums::AttemptStatus::Pending
        }
    }
}

/// Additive metadata: None history means unknown, not zero; unique IDs prevent replay double counting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaypalCaptureContext {
    // Only Some(true) certifies a complete actual resource history.
    // None is legacy/unknown; false retains observed resources with missing money.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_history_complete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_closed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorized_amount: Option<OrderAmount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_amounts: Option<std::collections::BTreeMap<String, OrderAmount>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_method: Option<common_enums::CaptureMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_descriptor: Option<String>,
}
impl PaypalCaptureContext {
    fn from_request(
        capture_method: Option<common_enums::CaptureMethod>,
        invoice_id: Option<String>,
        descriptor: Option<&domain_types::connector_types::BillingDescriptor>,
    ) -> Self {
        let soft_descriptor = paypal_descriptor_value(descriptor);
        Self {
            capture_method: Some(capture_method.unwrap_or(common_enums::CaptureMethod::Automatic)),
            invoice_id,
            soft_descriptor,
            ..Default::default()
        }
    }
}
fn saved_capture_context(data: &PaymentFlowData) -> Option<PaypalCaptureContext> {
    data.connector_feature_data
        .as_ref()
        .and_then(|value| serde_json::from_value::<PaypalMeta>(value.clone().expose()).ok())
        .and_then(|metadata| metadata.capture_context)
}
fn capture_minor(amount: &OrderAmount) -> Result<i64, Report<IntegrationError>> {
    let minor = StringMajorUnitForConnector
        .convert_back(amount.value.clone(), amount.currency_code)
        .change_context(IntegrationError::AmountConversionFailed {
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some(
                    "PayPal resource amount cannot be represented in currency minor units"
                        .to_owned(),
                ),
                suggested_action: Some(
                    "Retain the actual authorization/capture amount and currency without rounding"
                        .to_owned(),
                ),
                doc_url: Some("https://developer.paypal.com/docs/api/payments/v2/".to_owned()),
            },
        })?
        .get_amount_as_i64();
    if minor < 0 {
        return Err(sync_invalid("PayPal resource amount cannot be negative"));
    }
    Ok(minor)
}
fn captured_total(context: &PaypalCaptureContext) -> Result<Option<i64>, Report<IntegrationError>> {
    if context.capture_history_complete != Some(true) {
        return Ok(None);
    }
    let Some(captures) = context.capture_amounts.as_ref() else {
        return Ok(None);
    };
    let mut currency = context
        .authorized_amount
        .as_ref()
        .map(|amount| amount.currency_code);
    let mut total = 0_i64;
    for amount in captures.values() {
        if currency.is_some_and(|c| c != amount.currency_code) {
            return Err(sync_invalid("PayPal capture history currency mismatch"));
        }
        currency = Some(amount.currency_code);
        total = total
            .checked_add(capture_minor(amount)?)
            .ok_or_else(|| sync_invalid("PayPal captured total overflow"))?;
    }
    Ok(Some(total))
}
fn captured_ledger_money(
    context: &PaypalCaptureContext,
) -> Result<Option<Money>, Report<IntegrationError>> {
    let total = captured_total(context)?;
    Ok(total.filter(|total| *total > 0).and_then(|total| {
        context
            .capture_amounts
            .as_ref()?
            .values()
            .next()
            .map(|amount| Money {
                amount: MinorUnit::new(total),
                currency: amount.currency_code,
            })
    }))
}
fn authorization_expired(expiration: Option<&str>) -> Result<bool, Report<IntegrationError>> {
    expiration
        .map(|value| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
                .map(|expires| {
                    expires.unix_timestamp() <= common_utils::date_time::now_unix_timestamp()
                })
                .map_err(|_| sync_invalid("PayPal authorization expiration_time is invalid"))
        })
        .transpose()
        .map(|expired| expired.unwrap_or(false))
}

fn financial_sync_status(
    context: &PaypalCaptureContext,
    fallback: common_enums::AttemptStatus,
    current_hold_open: bool,
) -> Result<common_enums::AttemptStatus, Report<IntegrationError>> {
    use common_enums::AttemptStatus as S;
    let Some(total) = captured_total(context)? else {
        return Ok(if context.authorization_closed == Some(true) {
            S::Pending
        } else {
            fallback
        });
    };
    if total == 0 {
        return Ok(if context.authorization_closed == Some(true) {
            S::Voided
        } else {
            fallback
        });
    }
    let authorized = context
        .authorized_amount
        .as_ref()
        .map(capture_minor)
        .transpose()?;
    if authorized.is_some_and(|amount| total > amount) {
        return Err(sync_invalid("PayPal captured amount exceeds authorization"));
    }
    if authorized == Some(total)
        || (authorized.is_none()
            && fallback == S::Charged
            && matches!(
                context.capture_method,
                Some(
                    common_enums::CaptureMethod::Automatic
                        | common_enums::CaptureMethod::SequentialAutomatic
                )
            ))
    {
        return Ok(S::Charged);
    }
    Ok(
        if context.authorization_closed == Some(true) || !current_hold_open {
            S::PartialCharged
        } else {
            S::PartialChargedAndChargeable
        },
    )
}
fn capture_remaining(
    context: &PaypalCaptureContext,
) -> Result<Option<i64>, Report<IntegrationError>> {
    let authorized = context.authorized_amount.as_ref();
    let authorized_minor = authorized.map(capture_minor).transpose()?;
    let Some(captures) = context.capture_amounts.as_ref() else {
        return Ok(context
            .authorization_closed
            .filter(|closed| *closed)
            .map(|_| 0));
    };
    let mut currency = authorized.map(|amount| amount.currency_code);
    let mut total = 0_i64;
    for amount in captures.values() {
        if currency.is_some_and(|currency| currency != amount.currency_code) {
            return Err(sync_invalid("PayPal capture history currency mismatch"));
        }
        currency = Some(amount.currency_code);
        total = total
            .checked_add(capture_minor(amount)?)
            .ok_or_else(|| sync_invalid("PayPal captured total overflow"))?;
    }
    let Some(authorized_minor) = authorized_minor else {
        return Ok(context
            .authorization_closed
            .filter(|closed| *closed)
            .map(|_| 0));
    };
    let remaining = authorized_minor
        .checked_sub(total)
        .filter(|remaining| *remaining >= 0)
        .ok_or_else(|| sync_invalid("PayPal captured amount exceeds authorization"))?;
    if context.capture_history_complete != Some(true) && context.authorization_closed != Some(true)
    {
        return Ok(None);
    }
    Ok(Some(if context.authorization_closed == Some(true) {
        0
    } else {
        remaining
    }))
}
fn record_capture_amount(
    context: &mut PaypalCaptureContext,
    id: &str,
    amount: &OrderAmount,
) -> Result<(), Report<IntegrationError>> {
    if id.is_empty()
        || id
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(sync_invalid(
            "Actual PayPal capture id must be nonempty and unambiguous",
        ));
    }
    capture_minor(amount)?;
    if context
        .authorized_amount
        .as_ref()
        .is_some_and(|authorized| authorized.currency_code != amount.currency_code)
    {
        return Err(sync_invalid(
            "PayPal capture currency differs from authorization",
        ));
    }
    if let Some(captures) = context.capture_amounts.as_mut() {
        if captures.get(id).is_some_and(|previous| previous != amount) {
            return Err(sync_invalid(
                "PayPal capture id returned conflicting actual amount",
            ));
        }
        captures.insert(id.to_owned(), amount.clone());
    }
    capture_remaining(context)?;
    Ok(())
}
pub fn capture_metadata(
    data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> Result<PaypalMeta, Report<IntegrationError>> {
    let request = data
        .request
        .connector_feature_data
        .as_ref()
        .map(|value| value.clone().expose());
    let common = data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .map(|value| value.clone().expose());
    if matches!((&request,&common),(Some(a),Some(b)) if a!=b) {
        return Err(sync_invalid("Capture request and common metadata conflict"));
    }
    let metadata: PaypalMeta = serde_json::from_value(
        request
            .or(common)
            .ok_or_else(|| sync_invalid("Capture requires original authorization metadata"))?,
    )
    .map_err(|_| sync_invalid("Capture metadata is invalid"))?;
    let authorization = metadata
        .authorize_id
        .as_ref()
        .filter(|id| {
            !id.is_empty()
                && !id
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '/' | '?' | '#'))
        })
        .ok_or_else(|| sync_invalid("Capture requires a valid authorization id"))?;
    let provided = data
        .request
        .connector_transaction_id
        .get_connector_transaction_id()
        .change_context(IntegrationError::MissingConnectorTransactionID {
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some(
                    "Capture requires original order or authorization identifier".to_owned(),
                ),
                suggested_action: Some(
                    "Forward original PayPal connector_transaction_id and connector_feature_data"
                        .to_owned(),
                ),
                doc_url: Some("https://developer.paypal.com/docs/api/payments/v2/".to_owned()),
            },
        })?;
    if &provided != authorization && metadata.order_id.as_ref() != Some(&provided) {
        return Err(sync_invalid(
            "Capture identifier conflicts with original order or authorization",
        ));
    }
    if metadata
        .capture_context
        .as_ref()
        .and_then(|context| context.authorized_amount.as_ref())
        .is_some_and(|amount| amount.currency_code != data.request.currency)
    {
        return Err(sync_invalid(
            "Capture currency differs from actual authorization",
        ));
    }
    Ok(metadata)
}

/// Void targets the original authorization, retaining order identity and all continuation/context fields.
pub fn void_metadata(
    data: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
) -> Result<PaypalMeta, Report<IntegrationError>> {
    let request = data
        .request
        .connector_feature_data
        .as_ref()
        .map(|value| value.clone().expose());
    let common = data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .map(|value| value.clone().expose());
    if matches!((&request,&common),(Some(a),Some(b)) if a!=b) {
        return Err(sync_invalid("Void request and common metadata conflict"));
    }
    let metadata: PaypalMeta = serde_json::from_value(
        request
            .or(common)
            .ok_or_else(|| sync_invalid("Void requires original authorization metadata"))?,
    )
    .map_err(|_| sync_invalid("Void metadata is invalid"))?;
    let authorization = metadata
        .authorize_id
        .as_ref()
        .filter(|id| {
            !id.is_empty()
                && !id
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '/' | '?' | '#'))
        })
        .ok_or_else(|| sync_invalid("Void requires a valid authorization id"))?;
    if &data.request.connector_transaction_id != authorization
        && metadata.order_id.as_ref() != Some(&data.request.connector_transaction_id)
    {
        return Err(sync_invalid(
            "Void identifier conflicts with original order or authorization",
        ));
    }
    if metadata.capture_id.is_some()
        && metadata
            .capture_context
            .as_ref()
            .map(capture_remaining)
            .transpose()?
            .flatten()
            == Some(0)
    {
        return Err(sync_invalid(
            "Fully captured authorization cannot be voided; refund the actual capture instead",
        ));
    }
    Ok(metadata)
}

/// Refund request fields are checked independently of the original authorization balance.
fn refund_invalid(field_name: &'static str, message: &str) -> Report<IntegrationError> {
    IntegrationError::InvalidDataFormat {
        field_name,
        context: domain_types::errors::IntegrationErrorContext {
            additional_context: Some(message.to_owned()),
            suggested_action: Some(
                "Forward original PayPal capture metadata and the actual merchant refund request"
                    .to_owned(),
            ),
            doc_url: Some(
                "https://developer.paypal.com/docs/api/payments/v2/#captures_refund".to_owned(),
            ),
        },
    }
    .into()
}
fn refund_text(
    value: &str,
    field: &'static str,
    maximum: usize,
) -> Result<String, Report<IntegrationError>> {
    if value.trim().is_empty()
        || value.chars().count() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(refund_invalid(
            field,
            "PayPal refund text must be nonempty and within its documented field limit",
        ));
    }
    Ok(value.to_owned())
}
pub fn refund_metadata(
    data: &RouterDataV2<
        domain_types::connector_flow::Refund,
        RefundFlowData,
        RefundsData,
        RefundsResponseData,
    >,
) -> Result<PaypalMeta, Report<IntegrationError>> {
    let request = data
        .request
        .connector_feature_data
        .as_ref()
        .map(|value| value.clone().expose());
    let common = data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .map(|value| value.clone().expose());
    if matches!((&request,&common),(Some(a),Some(b)) if a!=b) {
        return Err(refund_invalid(
            "connector_feature_data",
            "Refund request and common payment metadata conflict",
        ));
    }
    let metadata: PaypalMeta = serde_json::from_value(request.or(common).ok_or_else(|| {
        refund_invalid(
            "connector_feature_data",
            "Refund requires original captured payment metadata",
        )
    })?)
    .map_err(|_| {
        refund_invalid(
            "connector_feature_data",
            "Refund payment metadata is invalid",
        )
    })?;
    let supplied = &data.request.connector_transaction_id;
    if !valid_sync_id(supplied) {
        return Err(refund_invalid(
            "connector_transaction_id",
            "Refund requires a valid capture or original order identity",
        ));
    }
    let ledger = metadata
        .capture_context
        .as_ref()
        .and_then(|context| context.capture_amounts.as_ref());
    // An explicit owned capture wins over a legacy saved first-capture pointer.
    // An order ID can use only the saved actual target, never an arbitrary map entry.
    let capture = if ledger.is_some_and(|captures| captures.contains_key(supplied))
        || metadata.capture_id.as_ref() == Some(supplied)
    {
        supplied.clone()
    } else if metadata.order_id.as_ref() == Some(supplied) {
        metadata
            .capture_id
            .clone()
            .filter(|id| valid_sync_id(id))
            .ok_or_else(|| {
                refund_invalid(
                    "connector_feature_data.capture_id",
                    "Select an actual owned capture ID; an order ledger is not a refund target",
                )
            })?
    } else {
        return Err(refund_invalid(
            "connector_transaction_id",
            "Refund target is not a known owned capture or original order",
        ));
    };
    let actual = ledger.and_then(|captures| captures.get(&capture));
    if metadata
        .capture_context
        .as_ref()
        .is_some_and(|context| context.capture_history_complete == Some(true))
        && actual.is_none()
    {
        return Err(refund_invalid(
            "connector_feature_data.capture_id",
            "Selected capture is absent from the complete owned ledger",
        ));
    }
    if let Some(actual) = actual {
        let gross = capture_minor(actual)?;
        if actual.currency_code != data.request.currency
            || data.request.refund_amount.currency != actual.currency_code
            || gross <= 0
            || data.request.refund_amount.amount.get_amount_as_i64() > gross
        {
            return Err(refund_invalid(
                "refund_amount",
                "Refund must fit the selected actual capture gross amount and currency",
            ));
        }
    }
    if data.request.refund_amount.amount.get_amount_as_i64() <= 0
        || data.request.refund_amount.amount.get_amount_as_i64()
            > data.request.payment_amount.amount.get_amount_as_i64()
    {
        return Err(refund_invalid(
            "refund_amount",
            "Refund must be positive and within the original payment amount",
        ));
    }
    // Closure of the remaining authorization does not erase captured funds.
    // Prior refunds are not guessed from gross money: PayPal validates net limits.
    let mut metadata = metadata;
    metadata.capture_id = Some(capture);
    Ok(metadata)
}

/// GET targets a refund ID directly; optional payment metadata can corroborate lineage.
pub fn rsync_refund_id(
    data: &RouterDataV2<
        domain_types::connector_flow::RSync,
        RefundFlowData,
        RefundSyncData,
        RefundsResponseData,
    >,
) -> Result<String, Report<IntegrationError>> {
    let id = &data.request.connector_refund_id;
    if !valid_sync_id(id) {
        return Err(refund_invalid(
            "refund_id",
            "Refund sync requires a nonempty unescaped refund resource ID",
        ));
    }
    let request = data
        .request
        .connector_feature_data
        .as_ref()
        .map(|value| value.clone().expose());
    let common = data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .map(|value| value.clone().expose());
    if matches!((&request,&common),(Some(a),Some(b))if a!=b) {
        return Err(refund_invalid(
            "connector_feature_data",
            "Refund sync payment metadata conflicts",
        ));
    }
    if let Some(value) = request.or(common) {
        let metadata: PaypalMeta = serde_json::from_value(value).map_err(|_| {
            refund_invalid(
                "connector_feature_data",
                "Refund sync payment metadata is invalid",
            )
        })?;
        let supplied = &data.request.connector_transaction_id;
        if !valid_sync_id(supplied) {
            return Err(refund_invalid(
                "connector_transaction_id",
                "Refund sync requires a valid original order or capture identity",
            ));
        }
        let ledger = metadata
            .capture_context
            .as_ref()
            .and_then(|context| context.capture_amounts.as_ref());
        let explicit_owned_capture = ledger.and_then(|captures| captures.get(supplied));
        let actual_capture = explicit_owned_capture.or_else(|| {
            (metadata.order_id.as_ref() == Some(supplied))
                .then(|| {
                    metadata
                        .capture_id
                        .as_ref()
                        .and_then(|id| ledger.and_then(|captures| captures.get(id)))
                })
                .flatten()
        });
        if metadata.order_id.as_ref() != Some(supplied)
            && metadata.capture_id.as_ref() != Some(supplied)
            && explicit_owned_capture.is_none()
        {
            return Err(refund_invalid(
                "connector_transaction_id",
                "Refund sync payment ID conflicts with original order or owned capture",
            ));
        }
        if let Some(actual) = actual_capture {
            let gross = capture_minor(actual)?;
            if gross <= 0
                || data.request.refund_money.as_ref().is_some_and(|requested| {
                    requested.currency != actual.currency_code
                        || requested.amount.get_amount_as_i64() <= 0
                        || requested.amount.get_amount_as_i64() > gross
                })
            {
                return Err(refund_invalid(
                    "refund_amount",
                    "Refund sync amount/currency contradicts the selected owned capture",
                ));
            }
        }
    }
    Ok(id.clone())
}

/// No issuer authorization/capture is inferred from saving a payment token.
pub fn setup_mandate_metadata<T: PaymentMethodDataTypes>(
    data: &RouterDataV2<
        domain_types::connector_flow::SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> Result<Option<PaypalMeta>, Report<IntegrationError>> {
    if data.request.amount.as_ref().is_some_and(|amount| {
        amount.amount != MinorUnit::zero() || amount.currency != data.request.currency
    }) {
        return Err(IntegrationError::NotSupported {message:"PayPal noncharging vault setup requires absent or zero amount in the requested currency".to_owned(),connector:"paypal",context:domain_types::errors::IntegrationErrorContext {additional_context:Some("A payment or Smart Dollar Auth hold is a distinct monetary operation; do not label it zero-value vault setup".to_owned()),doc_url:Some("https://developer.paypal.com/docs/api/payment-tokens/v3/".to_owned()),..Default::default()}}.into());
    }
    let Some(value) = data.resource_common_data.connector_feature_data.as_ref() else {
        return Ok(None);
    };
    if value.peek().get("continuation").is_none() && value.peek().get("psync_flow").is_none() {
        return Ok(None);
    }
    let metadata: PaypalMeta = serde_json::from_value(value.clone().expose())
        .map_err(|_| network_token_invalid("Vault setup continuation metadata is invalid"))?;
    if metadata.continuation.as_ref().is_some_and(|continuation| {
        continuation.operation != PaypalContinuationOperation::SetupRecurring
    }) {
        return Err(network_token_invalid(
            "A payment continuation cannot be executed as noncharging setup",
        ));
    }
    Ok(Some(metadata))
}
pub fn is_setup_encrypted_apple<T: PaymentMethodDataTypes>(
    data: &RouterDataV2<
        domain_types::connector_flow::SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> bool {
    matches!(&data.request.payment_method_data,PaymentMethodData::Wallet(WalletData::ApplePay(apple))if apple.payment_data.get_encrypted_apple_pay_payment_data_optional().is_some())
}
fn get_setup_redirect_url(links: Vec<PaypalLinks>) -> Result<Option<Url>, Report<ConnectorError>> {
    let mut matches = links
        .into_iter()
        .filter(|link| matches!(link.rel.as_str(), "approve" | "payer-action"))
        .filter_map(|link| link.href);
    let link = matches.next();
    if matches.next().is_some() || link.as_ref().is_some_and(|url| url.scheme() != "https") {
        return Err(ConnectorError::response_handling_failed_with_context(
            200,
            Some("Vault approval link is ambiguous or not HTTPS".to_owned()),
        )
        .into());
    }
    Ok(link)
}
pub fn merge_setup_metadata(
    common: &PaymentFlowData,
    metadata: &PaypalMeta,
) -> Result<serde_json::Value, serde_json::Error> {
    let mut value = common
        .connector_feature_data
        .as_ref()
        .map(|value| value.peek().clone())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    let updated = serde_json::to_value(metadata)?;
    if let (Some(original), Some(updated)) = (value.as_object_mut(), updated.as_object()) {
        original.extend(updated.clone());
    }
    Ok(value)
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
    > for PaypalSetupMandateRequest
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
        if let Some(metadata) = setup_mandate_metadata(&item.router_data)? {
            if let Some(continuation) = metadata.continuation {
                if continuation.stage != PaypalContinuationStage::SetupApproved {
                    return Err(network_token_invalid("Pending setup must be authoritatively approved before token exchange; completed setup must not be repeated"));
                }
                let id = continuation
                    .setup_token_id
                    .filter(|id| !id.peek().is_empty())
                    .ok_or_else(|| {
                        network_token_invalid("Approved setup has no original setup token ID")
                    })?;
                return Ok(Self::Exchange(PaypalTokenExchangeRequest {
                    payment_source: PaypalExchangeSource {
                        token: PaypalSetupTokenReference {
                            id,
                            token_type: "SETUP_TOKEN",
                        },
                    },
                }));
            }
        }
        if let PaymentMethodData::Wallet(WalletData::ApplePay(apple)) =
            &item.router_data.request.payment_method_data
        {
            if let Some(token) = apple
                .payment_data
                .get_encrypted_apple_pay_payment_data_optional()
            {
                if token.is_empty() || token.len() > 10000 {
                    return Err(network_token_invalid(
                        "Encrypted Apple Pay vault token must have1 to10000 characters",
                    ));
                }
                return Ok(Self::AppleSetup(PaypalAppleSetupRequest {
                    payment_source: PaypalAppleSetupSource {
                        apple_pay: PaypalEncryptedAppleToken {
                            token: Secret::new(token.clone()),
                        },
                    },
                }));
            }
        }
        Ok(Self::Direct(Box::new(PaypalZeroMandateRequest::try_from(
            item,
        )?)))
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaypalSetupMandateResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaypalSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let fail = || crate::utils::response_handling_fail_for_connector(item.http_code, "paypal");
        let previous = setup_mandate_metadata(&item.router_data).change_context(fail())?;
        let mut metadata = previous.unwrap_or(PaypalMeta {
            capture_context: None,
            authorize_id: None,
            capture_id: None,
            incremental_authorization_id: None,
            psync_flow: PaypalPaymentIntent::Authenticate,
            next_action: None,
            order_id: None,
            reference_id: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            continuation: None,
            authentication_result: None,
        });
        let mut common = item.router_data.resource_common_data;
        let (id, status, redirect, mandate, network_id, error) = match item.response {
            PaypalSetupMandateResponse::Setup(setup) => {
                if !matches!(&item.router_data.request.payment_method_data,PaymentMethodData::Wallet(WalletData::ApplePay(apple))if apple.payment_data.get_encrypted_apple_pay_payment_data_optional().is_some())
                    || setup.id.peek().is_empty()
                {
                    return Err(fail().into());
                }
                let (stage, status, redirect) = match setup.status {
                    PaypalAppleSetupStatus::Approved => (
                        PaypalContinuationStage::SetupApproved,
                        common_enums::AttemptStatus::Pending,
                        None,
                    ),
                    PaypalAppleSetupStatus::PayerActionRequired => {
                        let link = get_setup_redirect_url(setup.links)?.ok_or_else(fail)?;
                        (
                            PaypalContinuationStage::SetupApprovalRequired,
                            common_enums::AttemptStatus::AuthenticationPending,
                            Some(Box::new(RedirectForm::Form {
                                endpoint: link.to_string(),
                                method: Method::Get,
                                form_fields: std::collections::HashMap::new(),
                            })),
                        )
                    }
                    PaypalAppleSetupStatus::Created | PaypalAppleSetupStatus::Unknown => (
                        PaypalContinuationStage::SetupApprovalRequired,
                        common_enums::AttemptStatus::Pending,
                        None,
                    ),
                };
                metadata.continuation = Some(PaypalContinuation {
                    operation: PaypalContinuationOperation::SetupRecurring,
                    stage,
                    intent: PaypalPaymentIntent::Authenticate,
                    setup_token_id: Some(setup.id.clone()),
                    payment_token_id: None,
                });
                (setup.id.expose(), status, redirect, None, None, None)
            }
            PaypalSetupMandateResponse::PaymentToken(token) => {
                if token.id.trim().is_empty() || !(item.http_code == 200 || item.http_code == 201) {
                    return Err(fail().into());
                }
                let expected_apple = matches!(&item.router_data.request.payment_method_data,PaymentMethodData::Wallet(WalletData::ApplePay(apple))if apple.payment_data.get_encrypted_apple_pay_payment_data_optional().is_some());
                if token.payment_source.card.is_some() == token.payment_source.apple_pay.is_some()
                    || expected_apple != token.payment_source.apple_pay.is_some()
                {
                    return Err(fail().into());
                }
                if expected_apple
                    && metadata.continuation.as_ref().is_none_or(|continuation| {
                        continuation.stage != PaypalContinuationStage::SetupApproved
                    })
                {
                    return Err(fail().into());
                }
                let card = token.payment_source.card.as_ref();
                let status = match card.and_then(|card| card.verification_status.as_deref()) {
                    Some("FAILED" | "FAILURE") => common_enums::AttemptStatus::Failure,
                    Some("VERIFIED") | None => common_enums::AttemptStatus::Charged,
                    Some(_) => common_enums::AttemptStatus::Pending,
                };
                let verification = card.and_then(|card| card.verification.as_ref());
                let processor =
                    verification.and_then(|verification| verification.processor_response.as_ref());
                if let Some(processor) = processor {
                    common.connector_response=Some(domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {authentication_data:None,payment_checks:Some(serde_json::json!({"avs_code":processor.avs_code,"cvv_code":processor.cvv_code})),card_network:None,domestic_network:None,auth_code:None}));
                }
                common.connector_customer = token
                    .customer
                    .map(|customer| customer.id)
                    .filter(|id| !id.trim().is_empty())
                    .or(common.connector_customer);
                let network_id = verification
                    .and_then(|verification| verification.network_transaction_id.clone())
                    .or_else(|| {
                        card.and_then(|card| card.network_transaction_reference.as_ref())
                            .map(|reference| reference.id.clone())
                    })
                    .filter(|id| !id.trim().is_empty());
                let mandate = if status == common_enums::AttemptStatus::Charged {
                    Some(Box::new(MandateReference {
                        connector_mandate_id: Some(token.id.clone()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: card
                            .map(serde_json::to_value)
                            .transpose()
                            .change_context(fail())?
                            .map(Secret::new),
                    }))
                } else {
                    None
                };
                let error = if status == common_enums::AttemptStatus::Failure {
                    Some(domain_types::router_data::ErrorResponse {
                        code: "VAULT_VERIFICATION_FAILED".to_owned(),
                        message: "PayPal card verification failed".to_owned(),
                        reason: card.and_then(|card| card.verification_status.clone()),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(status)),
                        ..Default::default()
                    })
                } else {
                    None
                };
                if status == common_enums::AttemptStatus::Charged {
                    let setup_id = metadata
                        .continuation
                        .as_ref()
                        .and_then(|continuation| continuation.setup_token_id.clone());
                    metadata.continuation = Some(PaypalContinuation {
                        operation: PaypalContinuationOperation::SetupRecurring,
                        stage: PaypalContinuationStage::Completed,
                        intent: PaypalPaymentIntent::Authenticate,
                        setup_token_id: setup_id,
                        payment_token_id: Some(Secret::new(token.id.clone())),
                    });
                }
                (token.id, status, None, mandate, network_id, error)
            }
        };
        let value = merge_setup_metadata(&common, &metadata).change_context(fail())?;
        common.status = status;
        common.amount_captured = None;
        common.connector_feature_data = Some(Secret::new(value.clone()));
        let response = if let Some(error) = error {
            Err(error)
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(id.clone()),
                redirection_data: redirect,
                mandate_reference: mandate,
                connector_metadata: Some(value),
                network_txn_id: network_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };
        Ok(Self {
            resource_common_data: common,
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
pub struct PaypalAppleSaved {
    vault_id: Secret<String>,
}
#[derive(Debug, Serialize)]
pub struct PaypalMitCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    number: Option<RawCardNumber<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vault_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_token: Option<PaypalNetworkTokenCredential>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<CardRequestAttributes>,
    stored_credential: PaypalStoredCredential,
}
#[derive(Debug, Serialize)]
pub struct PaypalStoredCredential {
    payment_initiator: &'static str,
    payment_type: &'static str,
    usage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_transaction_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_network_transaction_reference: Option<PaypalNetworkTransactionReference>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PaypalMitMetadata {
    version: u8,
    previous_transaction_reference: Option<String>,
    previous_network_transaction_reference: Option<PaypalNetworkTransactionReference>,
}
fn paypal_mit_network(
    network: Option<&common_enums::CardNetwork>,
) -> Result<Option<String>, Report<IntegrationError>> {
    use common_enums::CardNetwork as N;
    network
        .map(|n| {
            Ok(match n {
                N::Visa => "VISA",
                N::Mastercard => "MASTERCARD",
                N::Discover => "DISCOVER",
                N::AmericanExpress => "AMEX",
                N::JCB => "JCB",
                N::DinersClub => "DINERS",
                N::CartesBancaires => "CARTE_BANCAIRE",
                N::UnionPay => "CHINA_UNION_PAY",
                N::RuPay => "RUPAY",
                N::Maestro => "MAESTRO",
                N::Star => "STAR",
                N::Pulse => "PULSE",
                N::Accel => "ACCEL",
                N::Nyce => "NYCE",
                _ => {
                    return Err(network_token_invalid(
                        "Unsupported network for PayPal MIT reference",
                    ))
                }
            }
            .to_owned())
        })
        .transpose()
}
fn paypal_mit_unsupported(message: &str) -> Report<IntegrationError> {
    IntegrationError::NotSupported {
        message: message.to_owned(),
        connector: "paypal",
        context: domain_types::errors::IntegrationErrorContext {
            doc_url: Some(
                "https://developer.paypal.com/sdk/orders/v2/definitions/card_stored_credential/"
                    .to_owned(),
            ),
            ..Default::default()
        },
    }
    .into()
}
fn paypal_mit_source<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    data: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
) -> Result<PaymentSourceItem<T>, Report<IntegrationError>> {
    use domain_types::connector_types::MandateReferenceId as M;
    let req = &data.request;
    let metadata = req
        .metadata
        .as_ref()
        .and_then(|m| m.peek().get("paypal_mit"))
        .map(|m| {
            serde_json::from_value::<PaypalMitMetadata>(m.clone())
                .map_err(|_| network_token_invalid("Invalid versioned metadata.paypal_mit"))
        })
        .transpose()?;
    if metadata.as_ref().is_some_and(|m| {
        m.version != 1
            || (m.previous_transaction_reference.is_some()
                && m.previous_network_transaction_reference.is_some())
    }) {
        return Err(network_token_invalid(
            "MIT reference metadata requires version1 and mutually exclusive references",
        ));
    }
    let kind = match req.mit_category {
        Some(common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription) => {
            "RECURRING"
        }
        None | Some(common_enums::MitCategory::Unscheduled) => "UNSCHEDULED",
        Some(common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission) => {
            return Err(paypal_mit_unsupported(
                "PayPal stored credential payment_type has no Installment/Resubmission equivalent",
            ))
        }
    };
    let mut card = PaypalMitCard {
        number: None,
        expiry: None,
        name: None,
        billing_address: get_address_info(data.resource_common_data.get_optional_payment_billing()),
        vault_id: None,
        network_token: None,
        attributes: (data.resource_common_data.auth_type
            == common_enums::AuthenticationType::ThreeDs)
            .then_some(CardRequestAttributes {
                vault: None,
                verification: Some(ThreeDsMethod {
                    method: ThreeDsType::ScaAlways,
                }),
                customer: None,
            }),
        stored_credential: PaypalStoredCredential {
            payment_initiator: "MERCHANT",
            payment_type: kind,
            usage: "SUBSEQUENT",
            previous_transaction_reference: None,
            previous_network_transaction_reference: None,
        },
    };
    if let M::ConnectorMandateId(m) = &req.mandate_reference {
        let id = m
            .get_connector_mandate_id()
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| network_token_invalid("MIT requires a nonempty reusable vault ID"))?;
        if metadata
            .as_ref()
            .is_some_and(|m| m.previous_network_transaction_reference.is_some())
        {
            return Err(network_token_invalid(
                "Vault MIT must not import a network mandate reference",
            ));
        }
        card.stored_credential.previous_transaction_reference =
            metadata.and_then(|m| m.previous_transaction_reference);
        if card
            .stored_credential
            .previous_transaction_reference
            .as_ref()
            .is_some_and(|s| s.is_empty())
        {
            return Err(network_token_invalid(
                "Original PayPal transaction reference must not be empty",
            ));
        }
        if req.payment_method_type != Some(common_enums::PaymentMethodType::Card)
            && card
                .stored_credential
                .previous_transaction_reference
                .is_some()
        {
            return Err(network_token_invalid(
                "Original PayPal card reference is not a field of wallet saved-source schema",
            ));
        }
        return match req.payment_method_type {
            Some(common_enums::PaymentMethodType::Card) => {
                card.vault_id = Some(id.into());
                // PayPal vault ID and submitted billing address are incompatible source alternatives.
                card.billing_address = None;
                Ok(PaymentSourceItem::Card(CardRequest::Mit(Box::new(card))))
            }
            Some(common_enums::PaymentMethodType::ApplePay) => {
                Ok(PaymentSourceItem::AppleSaved(PaypalAppleSaved {
                    vault_id: id.into(),
                }))
            }
            Some(common_enums::PaymentMethodType::Paypal) => Ok(PaymentSourceItem::Paypal(
                PaypalRedirectionRequest::PaypalVaultStruct(PaypalVaultStruct {
                    vault_id: id.into(),
                    experience_context: req
                        .complete_authorize_url
                        .clone()
                        .or(req.router_return_url.clone())
                        .map(|url| ContextStruct {
                            return_url: Some(url.clone()),
                            cancel_url: Some(url),
                            user_action: Some(UserAction::PayNow),
                            shipping_preference: ShippingPreference::GetFromFile,
                        }),
                }),
            )),
            _ => Err(network_token_invalid(
                "Unsupported saved source type for PayPal MIT",
            )),
        };
    }
    let nti = match &req.mandate_reference {
        M::NetworkMandateId(m) => &m.network_transaction_id,
        M::NetworkTokenWithNTI(m) => &m.network_transaction_id,
        M::ConnectorMandateId(_) => {
            return Err(network_token_invalid("Network MIT reference expected"))
        }
    };
    if !(9..=36).contains(&nti.len())
        || !nti
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_@.:&+=*^'~#!$%()".contains(&c))
    {
        return Err(network_token_invalid(
            "Original NTID must match PayPal network-reference schema",
        ));
    }
    if metadata
        .as_ref()
        .is_some_and(|m| m.previous_transaction_reference.is_some())
    {
        return Err(network_token_invalid(
            "Network MIT cannot carry previous PayPal transaction reference",
        ));
    }
    let mut reference = metadata.and_then(|m| m.previous_network_transaction_reference);
    if reference.as_ref().is_some_and(|r| r.id != *nti) {
        return Err(network_token_invalid(
            "Metadata network reference must equal supplied NTID",
        ));
    }
    if let Some(saved) = data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .and_then(|m| m.peek().get("paypal_network_reference"))
    {
        if saved.get("version").and_then(|v| v.as_u64()) == Some(1) {
            if let Some(saved) = saved
                .get("reference")
                .and_then(|v| {
                    serde_json::from_value::<PaypalNetworkTransactionReference>(v.clone()).ok()
                })
                .filter(|r| r.id == *nti)
            {
                if let Some(current) = reference.as_mut() {
                    for (supplied, observed) in [
                        (&mut current.date, saved.date),
                        (&mut current.network, saved.network),
                        (
                            &mut current.acquirer_reference_number,
                            saved.acquirer_reference_number,
                        ),
                    ] {
                        if matches!((&*supplied,&observed),(Some(a),Some(b)) if a!=b) {
                            return Err(network_token_invalid("Supplied original network evidence contradicts saved same-NTID evidence"));
                        }
                        if supplied.is_none() {
                            *supplied = observed;
                        }
                    }
                } else {
                    reference = Some(saved);
                }
            }
        }
    }
    let mut reference = reference.unwrap_or(PaypalNetworkTransactionReference {
        id: nti.clone(),
        date: None,
        network: None,
        acquirer_reference_number: None,
    });
    let network = match &req.payment_method_data {
        PaymentMethodData::Card(c) => {
            card.number = Some(c.card_number.clone());
            card.expiry = Some(c.get_expiry_date_as_yyyymm("-"));
            card.name = c.card_holder_name.clone();
            c.card_network.as_ref()
        }
        PaymentMethodData::CardWithNoCvc(c) => {
            card.number = Some(RawCardNumber(T::inner_from_card_number(
                c.card_number.clone(),
            )));
            card.expiry = Some(c.get_expiry_date_as_yyyymm("-")?);
            card.name = c.card_holder_name.clone();
            c.card_network.as_ref()
        }
        PaymentMethodData::CardDetailsForNetworkTransactionId(c) => {
            card.number = Some(c.card_number.clone());
            card.expiry = Some(c.get_expiry_date_as_yyyymm("-"));
            card.name = c.card_holder_name.clone();
            c.card_network.as_ref()
        }
        PaymentMethodData::NetworkToken(c) => {
            card.network_token = Some(PaypalNetworkTokenCredential {
                number: c.token_number.clone(),
                expiry: Secret::new(format!(
                    "{}-{:0>2}",
                    c.get_expiry_year_4_digit().peek(),
                    c.token_exp_month.peek()
                )),
                cryptogram: None,
                eci_flag: None,
            });
            c.card_network.as_ref()
        }
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(c) => {
            card.network_token = Some(PaypalNetworkTokenCredential {
                number: c.decrypted_token.clone(),
                expiry: c.get_expiry_date_as_yyyymm("-"),
                cryptogram: None,
                eci_flag: None,
            });
            c.card_network.as_ref()
        }
        _ => {
            return Err(network_token_invalid(
                "Network MIT requires actual matching card or network-token credential",
            ))
        }
    };
    if let Some(expiry) = card
        .expiry
        .as_ref()
        .or_else(|| card.network_token.as_ref().map(|token| &token.expiry))
    {
        let value = expiry.peek();
        let valid = value.split_once('-').is_some_and(|(year, month)| {
            year.len() == 4
                && year.bytes().all(|b| b.is_ascii_digit())
                && month.len() == 2
                && month.parse::<u8>().is_ok_and(|m| (1..=12).contains(&m))
        });
        if !valid {
            return Err(network_token_invalid(
                "MIT credential expiry must be YYYY-MM with valid month",
            ));
        }
    }
    if card.network_token.is_some()
        && data.resource_common_data.auth_type == common_enums::AuthenticationType::ThreeDs
    {
        return Err(paypal_mit_unsupported("Hosted native3DS is unsupported for network-token MIT; use corroborated original network reference without replaying authentication"));
    }
    if matches!(req.mandate_reference, M::NetworkTokenWithNTI(_)) && card.network_token.is_none() {
        return Err(network_token_invalid(
            "NetworkTokenWithNTI requires a network-token credential",
        ));
    }
    let network = paypal_mit_network(network)?;
    let inferred = match &req.payment_method_data {
        PaymentMethodData::Card(c) => utils::get_card_issuer(c.card_number.peek()).ok(),
        PaymentMethodData::CardWithNoCvc(c) => c.get_card_issuer().ok(),
        PaymentMethodData::CardDetailsForNetworkTransactionId(c) => c.get_card_issuer().ok(),
        PaymentMethodData::NetworkToken(c) => c.get_card_issuer().ok(),
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(c) => {
            utils::get_card_issuer(c.decrypted_token.peek()).ok()
        }
        _ => None,
    }
    .and_then(|issuer| match issuer {
        utils::CardIssuer::Visa => Some("VISA"),
        utils::CardIssuer::Master => Some("MASTERCARD"),
        utils::CardIssuer::Discover => Some("DISCOVER"),
        utils::CardIssuer::AmericanExpress => Some("AMEX"),
        _ => None,
    })
    .map(str::to_owned);
    if matches!((&network,&inferred),(Some(a),Some(b)) if a!=b) {
        return Err(network_token_invalid(
            "Supplied card network contradicts credential issuer",
        ));
    }
    let network = network.or(inferred);

    if matches!((&network,&reference.network),(Some(a),Some(b)) if a!=b) {
        return Err(network_token_invalid(
            "Supplied scheme contradicts actual credential network",
        ));
    }
    reference.network = reference.network.or(network);
    if reference.network.as_ref().is_some_and(|n| {
        ![
            "VISA",
            "MASTERCARD",
            "DISCOVER",
            "AMEX",
            "SOLO",
            "JCB",
            "STAR",
            "DELTA",
            "SWITCH",
            "MAESTRO",
            "CB_NATIONALE",
            "CONFIGOGA",
            "CONFIDIS",
            "ELECTRON",
            "CETELEM",
            "CHINA_UNION_PAY",
            "DINERS",
            "ELO",
            "HIPER",
            "HIPERCARD",
            "RUPAY",
            "GE",
            "SYNCHRONY",
            "EFTPOS",
            "CARTE_BANCAIRE",
            "STAR_ACCESS",
            "PULSE",
            "NYCE",
            "ACCEL",
        ]
        .contains(&n.as_str())
    }) {
        return Err(network_token_invalid(
            "Unsupported original scheme network enum",
        ));
    }
    if reference.network.is_none() {
        return Err(network_token_invalid(
            "Original network must be identifiable to enforce scheme date requirements",
        ));
    }

    if reference
        .date
        .as_ref()
        .is_some_and(|v| v.len() != 4 || !v.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(network_token_invalid(
            "Original scheme date must contain exactly4 digits",
        ));
    }
    if matches!(
        reference.network.as_deref(),
        Some("MASTERCARD" | "DISCOVER")
    ) && reference.date.is_none()
    {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "metadata.paypal_mit.previous_network_transaction_reference.date",
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some("Mastercard and Discover MIT require the original network transaction date as four digits".to_owned()),
                suggested_action: Some("Forward the original transaction date from the saved CIT network reference; never substitute today's date".to_owned()),
                doc_url: Some("https://developer.paypal.com/sdk/orders/v2/definitions/card_stored_credential/".to_owned()),
            },
        }
        .into());
    }
    if reference
        .acquirer_reference_number
        .as_ref()
        .is_some_and(|v| {
            v.is_empty() || v.len() > 36 || !v.bytes().all(|c| c.is_ascii_alphanumeric())
        })
    {
        return Err(network_token_invalid("Invalid original acquirer reference"));
    }
    card.stored_credential
        .previous_network_transaction_reference = Some(reference);
    Ok(PaymentSourceItem::Card(CardRequest::Mit(Box::new(card))))
}

pub type PaypalAuthenticateRequest<T> = PaypalPaymentsRequest<T>;
const AUTH_ORDER_FIELDS: &[&str] = &[
    "merchant_transaction_id",
    "merchant_order_id",
    "order_tax_amount",
    "shipping_cost",
    "order_details",
    "setup_future_usage",
    "billing_descriptor",
    "statement_descriptor_name",
    "statement_descriptor_suffix",
    "l2_l3_data",
    "description",
    "customer",
];
pub fn native_authorize_router<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    data: &RouterDataV2<
        domain_types::connector_flow::Authenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    Report<IntegrationError>,
> {
    use grpc_api_types::payments as P;
    use utils::{ForeignFrom, ForeignTryFrom};
    let req = &data.request;
    if req
        .currency
        .is_some_and(|currency| currency != req.amount.currency)
    {
        return Err(network_token_invalid("Native amount and currency disagree"));
    }

    let pm = req
        .payment_method_data
        .clone()
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "payment_method",
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some("Native PayPal authentication requires the actual raw-card payment method".to_owned()),
                suggested_action: Some("Provide the original card or card-without-CVC credential for native Authenticate; wallets use ordinary Authorize".to_owned()),
                doc_url: Some("https://developer.paypal.com/docs/checkout/advanced/customize/3d-secure/".to_owned()),
            },
        })?;
    if !matches!(
        pm,
        PaymentMethodData::Card(_) | PaymentMethodData::CardWithNoCvc(_)
    ) {
        return Err(paypal_mit_unsupported(
            "Native Authenticate accepts raw card; typed wallet calls use ordinary Authorize",
        ));
    }
    let return_url = req
        .router_return_url
        .as_ref()
        .or(req.continue_redirection_url.as_ref())
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "return_url",
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some("Native PayPal authentication requires a browser return URL".to_owned()),
                suggested_action: Some("Supply return_url or continue_redirection_url for the native card authentication continuation".to_owned()),
                doc_url: Some("https://developer.paypal.com/docs/checkout/advanced/customize/3d-secure/".to_owned()),
            },
        })?;
    if req.capture_method == Some(common_enums::CaptureMethod::Scheduled) {
        return Err(IntegrationError::CaptureMethodNotSupported {
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some("PayPal native authentication does not support scheduled capture".to_owned()),
                suggested_action: Some("Choose automatic capture or manual authorization followed by an explicit Capture call after authentication".to_owned()),
                doc_url: Some("https://developer.paypal.com/docs/api/orders/v2/".to_owned()),
            },
        }
        .into());
    }
    reject_external_authentication(req.authentication_data.as_ref())?;
    if data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .is_some_and(|m| !m.peek().is_object())
    {
        return Err(network_token_invalid(
            "Authentication metadata must be an object",
        ));
    }
    let mut proto = P::PaymentServiceAuthorizeRequest::default();
    if let Some(envelope) = data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .and_then(|m| m.peek().get("authentication_order_context"))
    {
        if envelope.get("version").and_then(|v| v.as_u64()) != Some(1)
            || envelope.as_object().is_none_or(|o| o.len() != 2)
        {
            return Err(network_token_invalid(
                "Invalid authentication_order_context version/shape",
            ));
        }
        let request = envelope
            .get("request")
            .and_then(|v| v.as_object())
            .ok_or_else(|| {
                network_token_invalid("Safe authentication context request must be an object")
            })?;
        if request
            .keys()
            .any(|k| !AUTH_ORDER_FIELDS.contains(&k.as_str()))
        {
            return Err(network_token_invalid(
                "Credential/unknown fields forbidden in authentication order context",
            ));
        }
        let mut value = serde_json::to_value(&proto)
            .map_err(|_| network_token_invalid("Cannot serialize canonical order context"))?;
        value
            .as_object_mut()
            .ok_or_else(|| network_token_invalid("Canonical request must be object"))?
            .extend(request.clone());
        proto = serde_json::from_value(value)
            .map_err(|_| network_token_invalid("Invalid typed safe order context"))?;
    }
    let mut common = data.resource_common_data.clone();
    // This RPC creates a new order. Browser continuations use PostAuthenticate instead.
    if let Some(metadata) = common.connector_feature_data.as_mut() {
        let mut value = metadata.peek().clone();
        let object = value
            .as_object_mut()
            .ok_or_else(|| network_token_invalid("Native order metadata must be object"))?;
        if object
            .get("continuation")
            .is_some_and(|value| !value.is_null())
        {
            return Err(network_token_invalid("Existing PayPal continuation must resume through PostAuthenticate, not create a new native order"));
        }
        for key in [
            "order_id",
            "authorize_id",
            "capture_id",
            "incremental_authorization_id",
            "reference_id",
            "psync_flow",
            "next_action",
            "continuation",
            "authentication_result",
            "capture_context",
            "paypal_network_reference",
            "paypal_mandate_reference",
        ] {
            object.remove(key);
        }
        *metadata = value.into();
    }

    if let Some(customer) = proto.customer.as_ref() {
        common.customer_id = customer
            .id
            .as_ref()
            .map(|id| {
                <common_utils::id_type::CustomerId as std::str::FromStr>::from_str(id)
                    .map_err(|_| network_token_invalid("Invalid original merchant customer ID"))
            })
            .transpose()?
            .or(common.customer_id);
        common.connector_customer = customer
            .connector_customer_id
            .clone()
            .or(common.connector_customer);
    }

    if let Some(id) = proto.merchant_transaction_id.as_ref() {
        if id.is_empty() {
            return Err(network_token_invalid(
                "Original merchant reference cannot be empty",
            ));
        }
        common.connector_request_reference_id = id.clone();
    }
    common.auth_type = common_enums::AuthenticationType::ThreeDs;
    common.amount = Some(req.amount.clone());
    if proto.description.is_some() {
        common.description = proto.description.clone();
    }
    if !proto.order_details.is_empty() {
        common.order_details = Some(
            proto
                .order_details
                .clone()
                .into_iter()
                .map(domain_types::payment_address::OrderDetailsWithAmount::foreign_try_from)
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if let Some(l23) = proto.l2_l3_data.as_ref() {
        common.l2_l3_data = Some(Box::new(
            domain_types::connector_types::L2L3Data::foreign_try_from((
                l23,
                &common.address,
                proto.customer.as_ref(),
                req.amount.currency,
            ))?,
        ));
    }
    proto.amount = Some(P::Money {
        minor_amount: req.amount.amount.get_amount_as_i64(),
        currency: i32::from(
            P::Currency::from_str_name(req.amount.currency.to_string().as_str()).ok_or_else(
                || network_token_invalid("Currency unsupported by canonical request"),
            )?,
        ),
    });
    proto.auth_type = i32::from(P::AuthenticationType::ThreeDs);
    proto.capture_method = Some(i32::from(match req.capture_method {
        Some(common_enums::CaptureMethod::Manual) => P::CaptureMethod::Manual,
        Some(common_enums::CaptureMethod::ManualMultiple) => P::CaptureMethod::ManualMultiple,
        _ => P::CaptureMethod::Automatic,
    }));
    proto.return_url = Some(return_url.to_string());
    proto.complete_authorize_url = Some(return_url.to_string());
    // Supply the actual credential before canonical conversion checks the payment
    // method. This temporary request is never stored in the nonsecret order context.
    let canonical_method = P::PaymentMethod {
        payment_method: Some(match &pm {
            PaymentMethodData::Card(card) => {
                P::payment_method::PaymentMethod::Card(P::CardDetails {
                    card_number: Some(card.card_number.try_card_number("paypal")?),
                    card_exp_month: Some(card.card_exp_month.clone()),
                    card_exp_year: Some(card.card_exp_year.clone()),
                    card_cvc: Some(card.card_cvc.clone()),
                    card_holder_name: card.card_holder_name.clone(),
                    card_issuer: card.card_issuer.clone(),
                    card_network: card
                        .card_network
                        .clone()
                        .map(P::CardNetwork::foreign_from)
                        .map(i32::from),
                    card_type: card.card_type.clone(),
                    card_issuing_country_alpha2: card.card_issuing_country.clone(),
                    bank_code: card.bank_code.clone(),
                    nick_name: card.nick_name.as_ref().map(|name| name.peek().clone()),
                })
            }
            PaymentMethodData::CardWithNoCvc(card) => {
                P::payment_method::PaymentMethod::CardWithNoCvc(P::CardDetailsWithNoCvc {
                    card_number: Some(card.card_number.clone()),
                    card_exp_month: Some(card.card_exp_month.clone()),
                    card_exp_year: Some(card.card_exp_year.clone()),
                    card_holder_name: card.card_holder_name.clone(),
                    card_issuer: card.card_issuer.clone(),
                    card_network: card
                        .card_network
                        .clone()
                        .map(P::CardNetwork::foreign_from)
                        .map(i32::from),
                    card_type: card.card_type.clone(),
                    card_issuing_country_alpha2: card.card_issuing_country.clone(),
                    bank_code: card.bank_code.clone(),
                    nick_name: card.nick_name.as_ref().map(|name| name.peek().clone()),
                })
            }
            _ => {
                return Err(paypal_mit_unsupported(
                    "Native Authenticate requires the actual raw-card credential",
                ))
            }
        }),
    };
    let canonical_data = match canonical_method.payment_method.as_ref() {
        Some(P::payment_method::PaymentMethod::Card(card)) => PaymentMethodData::Card(
            domain_types::payment_method_data::Card::foreign_try_from(card.clone())?,
        ),
        Some(P::payment_method::PaymentMethod::CardWithNoCvc(card)) => {
            PaymentMethodData::CardWithNoCvc(
                domain_types::payment_method_data::CardWithNoCvc::foreign_try_from(card.clone())?,
            )
        }
        _ => {
            return Err(paypal_mit_unsupported(
                "Native canonical payment method must remain the actual card credential",
            ))
        }
    };
    proto.payment_method = Some(canonical_method);
    let base = PaymentsAuthorizeData::<domain_types::payment_method_data::DefaultPCIHolder>::foreign_try_from((
        domain_types::types::AuthorizationRequest::from(proto),
        canonical_data,
    ))?;
    let request = PaymentsAuthorizeData::<T> {
        payment_method_data: pm,
        amount: base.amount,
        order_tax_amount: base.order_tax_amount,
        surcharge_amount: base.surcharge_amount,
        email: base.email,
        customer_document_details: base.customer_document_details,
        customer_date_of_birth: base.customer_date_of_birth,
        customer_name: base.customer_name,
        currency: base.currency,
        confirm: base.confirm,
        billing_descriptor: base.billing_descriptor,
        capture_method: base.capture_method,
        router_return_url: base.router_return_url,
        webhook_url: base.webhook_url,
        complete_authorize_url: base.complete_authorize_url,
        mandate_id: base.mandate_id,
        setup_future_usage: base.setup_future_usage,
        off_session: base.off_session,
        browser_info: base.browser_info,
        order_category: base.order_category,
        session_token: base.session_token,
        access_token: base.access_token,
        customer_acceptance: base.customer_acceptance,
        enrolled_for_3ds: base.enrolled_for_3ds,
        related_transaction_id: base.related_transaction_id,
        payment_experience: base.payment_experience,
        payment_method_type: base.payment_method_type,
        customer_id: base.customer_id,
        request_incremental_authorization: base.request_incremental_authorization,
        metadata: base.metadata,
        authentication_data: base.authentication_data,
        split_payments: base.split_payments,
        split_settlement: base.split_settlement,
        merchant_order_id: base.merchant_order_id,
        shipping_cost: base.shipping_cost,
        merchant_account_id: base.merchant_account_id,
        integrity_object: base.integrity_object,
        merchant_config_currency: base.merchant_config_currency,
        all_keys_required: base.all_keys_required,
        request_extended_authorization: base.request_extended_authorization,
        enable_overcapture: base.enable_overcapture,
        setup_mandate_details: base.setup_mandate_details,
        connector_feature_data: base.connector_feature_data,
        connector_testing_data: base.connector_testing_data,
        payment_channel: base.payment_channel,
        enable_partial_authorization: base.enable_partial_authorization,
        locale: base.locale,
        redirect_response: base.redirect_response,
        threeds_method_comp_ind: base.threeds_method_comp_ind,
        continue_redirection_url: base.continue_redirection_url,
        tokenization: base.tokenization,
        mit_category: base.mit_category,
        domain_data: base.domain_data,
        partner_merchant_identifier_details: base.partner_merchant_identifier_details,
        currency_conversion_data: base.currency_conversion_data,
        is_account_funding_transaction: base.is_account_funding_transaction,
        recipient_details: base.recipient_details,
        additional_connector_details: base.additional_connector_details,
        customer: base.customer,
        business_country: base.business_country,
        enable_avs_check: base.enable_avs_check,
    };
    Ok(RouterDataV2 {
        flow: std::marker::PhantomData,
        resource_common_data: common,
        connector_config: data.connector_config.clone(),
        request,
        response: data.response.clone(),
    })
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaypalRouterData<
            RouterDataV2<
                domain_types::connector_flow::Authenticate,
                PaymentFlowData,
                domain_types::connector_types::PaymentsAuthenticateData<T>,
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
                domain_types::connector_flow::Authenticate,
                PaymentFlowData,
                domain_types::connector_types::PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = native_authorize_router(&item.router_data)?;
        Self::try_from(PaypalRouterData {
            connector: item.connector,
            router_data,
        })
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaypalAuthResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::Authenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PaypalAuthResponse, Self>) -> Result<Self, Self::Error> {
        let router = native_authorize_router(&item.router_data).change_context(
            crate::utils::response_handling_fail_for_connector(item.http_code, "paypal"),
        )?;
        let original = router.resource_common_data.connector_feature_data.clone();
        let mut converted = RouterDataV2::<Authorize, _, _, _>::try_from(ResponseRouterData {
            response: item.response,
            router_data: router,
            http_code: item.http_code,
        })?;
        let response = match converted.response {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data,
                connector_metadata,
                connector_response_reference_id,
                mandate_reference,
                ..
            }) => {
                let mut metadata = original
                    .map(|v| v.expose())
                    .unwrap_or_else(|| serde_json::json!({}));
                if let Some(next) = connector_metadata {
                    let object = metadata.as_object_mut().ok_or_else(|| {
                        crate::utils::response_handling_fail_for_connector(item.http_code, "paypal")
                    })?;
                    object.extend(
                        next.as_object()
                            .ok_or_else(|| {
                                crate::utils::response_handling_fail_for_connector(
                                    item.http_code,
                                    "paypal",
                                )
                            })?
                            .clone(),
                    );
                }
                if let Some(reference) = mandate_reference
                    .as_ref()
                    .filter(|reference| reference.connector_mandate_id.is_some())
                {
                    metadata.as_object_mut().ok_or_else(||crate::utils::response_handling_fail_for_connector(item.http_code,"paypal"))?.insert("paypal_mandate_reference".to_owned(),serde_json::json!({"version":1,"connector_mandate_id":reference.connector_mandate_id,"payment_method_id":reference.payment_method_id,"connector_mandate_request_reference_id":reference.connector_mandate_request_reference_id,"mandate_metadata":reference.mandate_metadata}));
                }
                converted.resource_common_data.connector_feature_data =
                    Some(metadata.clone().into());
                Ok(PaymentsResponseData::AuthenticateResponse {
                    resource_id: Some(resource_id),
                    redirection_data,
                    authentication_data: None,
                    connector_feature_data: Some(metadata),
                    connector_response_reference_id: connector_response_reference_id.or_else(
                        || {
                            converted
                                .resource_common_data
                                .connector_feature_data
                                .as_ref()
                                .and_then(|v| v.peek().get("reference_id"))
                                .and_then(|v| v.as_str())
                                .map(str::to_owned)
                        },
                    ),
                    status_code: item.http_code,
                })
            }
            Err(error) => Err(error),
            _ => {
                return Err(crate::utils::response_handling_fail_for_connector(
                    item.http_code,
                    "paypal",
                )
                .into())
            }
        };
        Ok(Self {
            flow: item.router_data.flow,
            resource_common_data: converted.resource_common_data,
            connector_config: item.router_data.connector_config,
            request: item.router_data.request,
            response,
        })
    }
}
pub fn post_authenticate_path<T: PaymentMethodDataTypes>(
    data: &RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<String, Report<IntegrationError>> {
    let value = data
        .resource_common_data
        .connector_feature_data
        .as_ref()
        .ok_or_else(|| {
            network_token_invalid("PostAuthenticate requires original order metadata")
        })?;
    let meta: PaypalMeta = serde_json::from_value(value.clone().expose())
        .map_err(|_| network_token_invalid("Invalid original PayPal metadata"))?;
    if let Some(c) = meta
        .continuation
        .as_ref()
        .filter(|c| c.stage == PaypalContinuationStage::SetupApprovalRequired)
    {
        let id = c
            .setup_token_id
            .as_ref()
            .filter(|id| valid_sync_id(id.peek()))
            .ok_or_else(|| network_token_invalid("Original setup token missing"))?;
        return Ok(format!("v3/vault/setup-tokens/{}", id.peek()));
    }
    let order = meta
        .order_id
        .as_ref()
        .filter(|id| valid_sync_id(id))
        .ok_or_else(|| network_token_invalid("Original PayPal order ID missing"))?;
    if data
        .request
        .connector_order_reference_id
        .as_ref()
        .or(data.resource_common_data.reference_id.as_ref())
        .is_some_and(|id| id != order)
    {
        return Err(network_token_invalid(
            "PostAuthenticate order identity mismatch",
        ));
    }
    Ok(format!("v2/checkout/orders/{order}?fields=payment_source"))
}
