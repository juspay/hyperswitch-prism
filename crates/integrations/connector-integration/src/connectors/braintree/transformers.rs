use crate::{connectors::braintree::BraintreeRouterData, types::ResponseRouterData, utils};
use base64::Engine;
use common_enums::enums;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::XmlExt,
    pii,
    types::{MinorUnit, StringMajorUnit},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PaymentMethodToken, RSync,
        RepeatPayment, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        self, AmountInfo, ApplePayPaymentRequest, ApplePaySessionResponse,
        ApplepayClientAuthenticationResponse, ClientAuthenticationTokenData,
        ClientAuthenticationTokenRequestData, GooglePaySessionResponse,
        GpayAllowedMethodsParameters, GpayAllowedPaymentMethods, GpayClientAuthenticationResponse,
        GpayMerchantInfo, GpayShippingAddressParameters, GpayTokenParameters,
        GpayTokenizationSpecification, GpayTransactionInfo, MandateReference, NextActionCall,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentRequestMetadata, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        PaypalClientAuthenticationResponse, PaypalTransactionInfo, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SdkNextAction,
        SecretInfoToInitiateSdk, SetupMandateRequestData, ThirdPartySdkSessionResponse,
    },
    errors::{ConnectorError, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::{AddressDetails, OrderDetailsWithAmount, PhoneDetails},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData},
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types,
    router_response_types::RedirectForm,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use strum::Display;
use time::PrimitiveDateTime;
use tracing::info;

pub const BRAINTREE_CONNECTOR_NAME: &str = "braintree";

/// Selection set shared by the four card-Authorize mutations. It is a `macro_rules!`
/// rather than a `const` so `concat!` can splice it into each `&'static str` query at
/// compile time.
///
/// Two placement rules from the Braintree GraphQL SDL are encoded here and are not
/// obvious from the field names:
///
/// 1. `processorAuthorizationResponse` is the flat, fragment-free read path for AVS,
///    CVV and the processor response code/text. `Transaction.processorResponse` is
///    deprecated in its favour, and `Transaction.processorSettlementResponse` carries
///    AVS/CVV only as deprecated fields (they are not evaluated at capture time).
/// 2. `merchantAdviceCodeResponse` has **no** flat path on `Transaction` at all. It
///    exists on exactly three types — `ProcessorDeclinedEvent`, `GatewayRejectedEvent`
///    and `FailedEvent` — while the `PaymentStatusEvent` interface exposes only
///    `status`/`timestamp`/`amount`/`source`/`terminal`. Reading it therefore forces the
///    inline-fragment walk over `statusHistory` below.
///
/// `... on SettledEvent` is deliberately omitted: its `processorResponse` resolves to a
/// different SDL type (`TransactionSettlementProcessorResponse`), and an Authorize
/// response can never contain a settled event.
macro_rules! card_transaction_fields {
    () => {
        "id legacyId createdAt status orderId amount { value currencyCode } \
         processorAuthorizationResponse { legacyCode message cvvResponse avsPostalCodeResponse \
         avsStreetAddressResponse authorizationId additionalInformation retrievalReferenceNumber } \
         statusHistory { status terminal \
         ... on AuthorizedEvent { processorResponse { legacyCode message cvvResponse \
         avsPostalCodeResponse avsStreetAddressResponse authorizationId retrievalReferenceNumber } \
         networkResponse { code message } } \
         ... on ProcessorDeclinedEvent { declineType processorResponse { legacyCode message \
         cvvResponse avsPostalCodeResponse avsStreetAddressResponse additionalInformation \
         retrievalReferenceNumber } networkResponse { code message } \
         merchantAdviceCodeResponse { code message } } \
         ... on GatewayRejectedEvent { gatewayRejectionReason processorResponse { legacyCode \
         message cvvResponse avsPostalCodeResponse avsStreetAddressResponse } \
         networkResponse { code message } merchantAdviceCodeResponse { code message } } \
         ... on FailedEvent { processorResponse { legacyCode message additionalInformation } \
         networkResponse { code message } merchantAdviceCodeResponse { code message } } }"
    };
}

pub mod constants {
    pub const CHANNEL_CODE: &str = "HyperSwitchBT_Ecom";
    pub const CLIENT_TOKEN_MUTATION: &str = "mutation createClientToken($input: CreateClientTokenInput!) { createClientToken(input: $input) { clientToken}}";
    pub const TOKENIZE_CREDIT_CARD: &str = "mutation  tokenizeCreditCard($input: TokenizeCreditCardInput!) { tokenizeCreditCard(input: $input) { clientMutationId paymentMethod { id } } }";
    // `paymentMethod { id }` is intentionally NOT selected on the two non-vault mutations:
    // the response mapper derives `mandate_reference` from it, and a plain Authorize must
    // not start reporting a mandate reference for a single-use payment method.
    pub const CHARGE_CREDIT_CARD_MUTATION: &str = concat!(
        "mutation ChargeCreditCard($input: ChargeCreditCardInput!) { chargeCreditCard(input: $input) { transaction { ",
        card_transaction_fields!(),
        " } } }"
    );
    pub const AUTHORIZE_CREDIT_CARD_MUTATION: &str = concat!(
        "mutation authorizeCreditCard($input: AuthorizeCreditCardInput!) { authorizeCreditCard(input: $input) { transaction { ",
        card_transaction_fields!(),
        " } } }"
    );
    pub const CAPTURE_TRANSACTION_MUTATION: &str = "mutation captureTransaction($input: CaptureTransactionInput!) { captureTransaction(input: $input) { clientMutationId transaction { id legacyId amount { value currencyCode } status } } }";
    pub const VOID_TRANSACTION_MUTATION: &str = "mutation voidTransaction($input:  ReverseTransactionInput!) { reverseTransaction(input: $input) { clientMutationId reversal { ...  on Transaction { id legacyId amount { value currencyCode } status } } } }";
    pub const REFUND_TRANSACTION_MUTATION: &str = "mutation refundTransaction($input:  RefundTransactionInput!) { refundTransaction(input: $input) {clientMutationId refund { id legacyId amount { value currencyCode } status } } }";
    // The vault variants keep `paymentMethod { id }` — it is the vaulted (multi-use) token
    // the mandate reference is built from.
    pub const AUTHORIZE_AND_VAULT_CREDIT_CARD_MUTATION: &str = concat!(
        "mutation authorizeCreditCard($input: AuthorizeCreditCardInput!) { authorizeCreditCard(input: $input) { transaction { ",
        card_transaction_fields!(),
        " paymentMethod { id } } } }"
    );
    pub const CHARGE_AND_VAULT_TRANSACTION_MUTATION: &str = concat!(
        "mutation ChargeCreditCard($input: ChargeCreditCardInput!) { chargeCreditCard(input: $input) { transaction { ",
        card_transaction_fields!(),
        " paymentMethod { id } } } }"
    );
    pub const DELETE_PAYMENT_METHOD_FROM_VAULT_MUTATION: &str = "mutation deletePaymentMethodFromVault($input: DeletePaymentMethodFromVaultInput!) { deletePaymentMethodFromVault(input: $input) { clientMutationId } }";
    pub const TRANSACTION_QUERY: &str = "query($input: TransactionSearchInput!) { search { transactions(input: $input) { edges { node { id status } } } } }";
    pub const REFUND_QUERY: &str = "query($input: RefundSearchInput!) { search { refunds(input: $input, first: 1) { edges { node { id status createdAt amount { value currencyCode } orderId } } } } }";
    pub const CHARGE_GOOGLE_PAY_MUTATION: &str = "mutation ChargeGPay($input: ChargePaymentMethodInput!) { chargePaymentMethod(input: $input) { transaction { id status amount { value currencyCode } } } }";
    pub const AUTHORIZE_GOOGLE_PAY_MUTATION: &str = "mutation authorizeGPay($input: AuthorizePaymentMethodInput!) { authorizePaymentMethod(input: $input) { transaction { id legacyId amount { value currencyCode } status } } }";
    pub const CHARGE_APPLE_PAY_MUTATION: &str = "mutation ChargeApplepay($input: ChargePaymentMethodInput!) { chargePaymentMethod(input: $input) { transaction { id status amount { value currencyCode } } } }";
    pub const AUTHORIZE_APPLE_PAY_MUTATION: &str = "mutation authorizeApplepay($input: AuthorizePaymentMethodInput!) { authorizePaymentMethod(input: $input) { transaction { id legacyId amount { value currencyCode } status } } }";
    pub const CHARGE_AND_VAULT_APPLE_PAY_MUTATION: &str = "mutation ChargeApplepay($input: ChargePaymentMethodInput!) { chargePaymentMethod(input: $input) { transaction { id status amount { value currencyCode } paymentMethod { id } } } }";
    pub const AUTHORIZE_AND_VAULT_APPLE_PAY_MUTATION: &str = "mutation authorizeApplepay($input: AuthorizePaymentMethodInput!) { authorizePaymentMethod(input: $input) { transaction { id legacyId amount { value currencyCode } status paymentMethod { id } } } }";
    pub const CHARGE_PAYPAL_MUTATION: &str = "mutation ChargePaypal($input: ChargePaymentMethodInput!) { chargePaymentMethod(input: $input) { transaction { id status amount { value currencyCode } } } }";
    pub const AUTHORIZE_PAYPAL_MUTATION: &str = "mutation authorizePaypal($input: AuthorizePaymentMethodInput!) { authorizePaymentMethod(input: $input) { transaction { id legacyId amount { value currencyCode } status } } }";
}

pub type CardPaymentRequest = GenericBraintreeRequest<VariablePaymentInput>;
pub type MandatePaymentRequest = GenericBraintreeRequest<VariablePaymentInput>;
pub type BraintreeClientTokenRequest = GenericBraintreeRequest<VariableClientTokenInput>;
pub type BraintreeTokenRequest<T> = GenericBraintreeRequest<VariableInput<T>>;
pub type BraintreeCaptureRequest = GenericBraintreeRequest<VariableCaptureInput>;
pub type BraintreeRefundRequest = GenericBraintreeRequest<BraintreeRefundVariables>;
pub type BraintreePSyncRequest = GenericBraintreeRequest<PSyncInput>;
pub type BraintreeRSyncRequest = GenericBraintreeRequest<RSyncInput>;
pub type BraintreeWalletRequest = GenericBraintreeRequest<GenericVariableInput<WalletPaymentInput>>;

pub type BraintreeRefundResponse = GenericBraintreeResponse<RefundResponse>;
pub type BraintreeCaptureResponse = GenericBraintreeResponse<CaptureResponse>;
pub type BraintreePSyncResponse = GenericBraintreeResponse<PSyncResponse>;

pub type VariablePaymentInput = GenericVariableInput<PaymentInput>;
pub type VariableClientTokenInput = GenericVariableInput<InputClientTokenData>;
pub type VariableInput<T> = GenericVariableInput<InputData<T>>;
pub type VariableCaptureInput = GenericVariableInput<CaptureInputData>;
pub type BraintreeRefundVariables = GenericVariableInput<BraintreeRefundInput>;
pub type PSyncInput = GenericVariableInput<TransactionSearchInput>;
pub type RSyncInput = GenericVariableInput<RefundSearchInput>;

#[derive(Debug, Clone, Serialize)]
pub struct GenericBraintreeRequest<T> {
    query: String,
    variables: T,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GenericBraintreeResponse<T> {
    SuccessResponse(Box<T>),
    ErrorResponse(Box<ErrorResponse>),
}
#[derive(Debug, Clone, Serialize)]
pub struct GenericVariableInput<T> {
    input: T,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletTransactionBody {
    amount: StringMajorUnit,
    merchant_account_id: Secret<String>,
    order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_details: Option<CustomerBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vault_payment_method_after_transacting: Option<TransactionTiming>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletPaymentInput {
    payment_method_id: Secret<String>,
    transaction: WalletTransactionBody,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeApiErrorResponse {
    pub api_error_response: ApiErrorResponse,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorsObject {
    pub errors: Vec<ErrorObject>,

    pub transaction: Option<TransactionError>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionError {
    pub errors: Vec<ErrorObject>,
    pub credit_card: Option<CreditCardError>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreditCardError {
    pub errors: Vec<ErrorObject>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorObject {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeErrorResponse {
    pub errors: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ErrorResponses {
    BraintreeApiErrorResponse(Box<BraintreeApiErrorResponse>),
    BraintreeErrorResponse(Box<BraintreeErrorResponse>),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiErrorResponse {
    pub message: String,
    pub errors: ErrorsObject,
}

pub struct BraintreeAuthType {
    pub(super) public_key: Secret<String>,
    pub(super) private_key: Secret<String>,
    pub(super) merchant_account_id: Option<Secret<String>>,
    pub(super) merchant_config_currency: Option<String>,
    pub(super) apple_pay_supported_networks: Vec<String>,
    pub(super) apple_pay_merchant_capabilities: Vec<String>,
    pub(super) apple_pay_label: Option<String>,
    pub(super) gpay_merchant_name: Option<String>,
    pub(super) gpay_merchant_id: Option<String>,
    pub(super) gpay_allowed_auth_methods: Vec<String>,
    pub(super) gpay_allowed_card_networks: Vec<String>,
    pub(super) paypal_client_id: Option<String>,
    pub(super) gpay_gateway_merchant_id: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for BraintreeAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Braintree {
            public_key,
            private_key,
            merchant_account_id,
            merchant_config_currency,
            apple_pay_supported_networks,
            apple_pay_merchant_capabilities,
            apple_pay_label,
            gpay_merchant_name,
            gpay_merchant_id,
            gpay_allowed_auth_methods,
            gpay_allowed_card_networks,
            paypal_client_id,
            gpay_gateway_merchant_id,
            ..
        } = item
        {
            Ok(Self {
                public_key: public_key.to_owned(),
                private_key: private_key.to_owned(),
                merchant_account_id: merchant_account_id.clone(),
                merchant_config_currency: merchant_config_currency.clone(),
                apple_pay_supported_networks: apple_pay_supported_networks.clone(),
                apple_pay_merchant_capabilities: apple_pay_merchant_capabilities.clone(),
                apple_pay_label: apple_pay_label.clone(),
                gpay_merchant_name: gpay_merchant_name.clone(),
                gpay_merchant_id: gpay_merchant_id.clone(),
                gpay_allowed_auth_methods: gpay_allowed_auth_methods.clone(),
                gpay_allowed_card_networks: gpay_allowed_card_networks.clone(),
                paypal_client_id: paypal_client_id.clone(),
                gpay_gateway_merchant_id: gpay_gateway_merchant_id.clone(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into())
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInput {
    payment_method_id: Secret<String>,
    transaction: TransactionBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<CreditCardTransactionOptions>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum BraintreePaymentsRequest {
    Card(CardPaymentRequest),
    CardThreeDs(BraintreeClientTokenRequest),
    Mandate(MandatePaymentRequest),
    Wallet(BraintreeWalletRequest),
}

#[derive(Debug, Deserialize)]
pub struct BraintreeMeta {
    merchant_account_id: Secret<String>,
    merchant_config_currency: enums::Currency,
}

impl TryFrom<&Option<pii::SecretSerdeValue>> for BraintreeMeta {
    type Error = Report<IntegrationError>;
    fn try_from(meta_data: &Option<pii::SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata: Self = utils::to_connector_meta_from_secret::<Self>(meta_data.clone())
            .change_context(IntegrationError::InvalidConnectorConfig {
                config: "metadata",
                context: Default::default(),
            })?;
        Ok(metadata)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerBody {
    email: pii::Email,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegularTransactionBody {
    amount: StringMajorUnit,
    merchant_account_id: Secret<String>,
    channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_details: Option<CustomerBody>,
    order_id: String,
    /// Level 2/Level 3, shipping and descriptor fields, flattened onto `TransactionInput`.
    #[serde(flatten)]
    enrichment: TransactionEnrichment,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultTransactionBody {
    amount: StringMajorUnit,
    merchant_account_id: Secret<String>,
    vault_payment_method_after_transacting: TransactionTiming,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_details: Option<CustomerBody>,
    order_id: String,
    /// Level 2/Level 3, shipping and descriptor fields, flattened onto `TransactionInput`.
    #[serde(flatten)]
    enrichment: TransactionEnrichment,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MandateTransactionBody {
    amount: StringMajorUnit,
    merchant_account_id: Secret<String>,
    channel: String,
    order_id: String,
    payment_initiator: PaymentInitiatorType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentInitiatorType {
    Unscheduled,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum TransactionBody {
    Regular(RegularTransactionBody),
    Vault(VaultTransactionBody),
    Mandate(MandateTransactionBody),
}

/// Braintree GraphQL `PhoneInput`. Both `countryPhoneCode` and `phoneNumber` are
/// non-null in the SDL, so the object is only emitted when UCS holds both halves.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreePhoneInput {
    country_phone_code: String,
    phone_number: Secret<String>,
}

/// Braintree GraphQL `AddressInput`. The SDL declares **one** shared address type used
/// by both `options.billingAddress` and `transaction.shipping.shippingAddress`.
///
/// `AddressInput` offers two parallel naming schemes for the same concepts
/// (`streetAddress`/`addressLine1`, `extendedAddress`/`addressLine2`,
/// `locality`/`adminArea2`, `region`/`adminArea1`). Only the first scheme is used here so
/// a single address never mixes both.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeAddressInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    street_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extended_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locality: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postal_code: Option<Secret<String>>,
    /// `AddressInput.countryCode` is the *version-sensitive* `CountryCode` scalar: the SDL
    /// docstring says clients on a `Braintree-Version` prior to `2021-02-01` must send an
    /// ISO 3166-1 **alpha-3** code. This connector pins `2019-01-01`
    /// (`BRAINTREE_VERSION_VALUE`), so the alpha-2 UCS stores is converted to alpha-3 here.
    /// The removed `countryCodeAlpha2`/`countryCodeAlpha3`/`countryCodeNumeric`/`countryName`
    /// fields must never be emitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    country_code: Option<common_enums::CountryAlpha3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<BraintreePhoneInput>,
}

impl BraintreeAddressInput {
    /// `true` when nothing at all could be mapped — the caller then omits the address
    /// object entirely rather than sending an empty `{}`.
    fn is_empty(&self) -> bool {
        self.first_name.is_none()
            && self.last_name.is_none()
            && self.street_address.is_none()
            && self.extended_address.is_none()
            && self.locality.is_none()
            && self.region.is_none()
            && self.postal_code.is_none()
            && self.country_code.is_none()
            && self.phone.is_none()
    }
}

/// Braintree GraphQL `TransactionTaxInput`, nested under `transaction.tax`.
///
/// Note the money-type asymmetry the SDL imposes: `tax.taxAmount` is the custom scalar
/// `Amount`, while every other L2/L3 money field (`discountAmount`, `shippingAmount`,
/// `shippingTaxAmount`, the line-item amounts) is a plain `String`. Both are produced by
/// the connector's `StringMajorUnit` amount converter, so the Rust type is the same.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionTaxInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    tax_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax_exempt: Option<bool>,
}

/// Braintree GraphQL `TransactionShippingInput`, nested under `transaction.shipping`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionShippingInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address: Option<BraintreeAddressInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_tax_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ships_from_postal_code: Option<Secret<String>>,
}

/// Braintree GraphQL `TransactionLineItemType`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionLineItemType {
    Debit,
}

/// Braintree GraphQL `TransactionLineItemInput` — Level 3 line-item detail.
///
/// `name`, `kind`, `quantity`, `unitAmount` and `totalAmount` are the five non-null
/// fields in the SDL; everything else is optional Level 3 enrichment.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionLineItemInput {
    name: String,
    kind: TransactionLineItemType,
    quantity: String,
    unit_amount: StringMajorUnit,
    total_amount: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit_of_measure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    product_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commodity_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

/// Braintree GraphQL `TransactionDescriptorInput` — the dynamic descriptor that appears
/// on the cardholder's statement. The SDL declares exactly three optional strings:
/// `name`, `phone`, `url`. There is no `dynamicDescriptor` / `descriptorName` field.
///
/// UCS `BillingDescriptor` also carries `city`, `statement_descriptor`,
/// `statement_descriptor_suffix` and `reference`; none of those has a Braintree
/// descriptor counterpart, and they are mapped field-for-field rather than composed into
/// `name`, so this connector never invents a `<prefix>*<suffix>` string the merchant did
/// not ask for. `url` has no UCS source today.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDescriptorInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<Secret<String>>,
}

/// Level 2 / Level 3, shipping and dynamic-descriptor fields that hang off
/// `TransactionInput`. Held in one struct and `#[serde(flatten)]`-ed into the transaction
/// bodies so the SDL's flat-vs-nested placement is expressed exactly once:
///
/// * FLAT on `TransactionInput`: `purchaseOrderNumber`, `discountAmount`, `lineItems`,
///   `descriptor`.
/// * NESTED: `taxAmount`/`taxExempt` under `tax`; `shippingAmount`/`shippingTaxAmount`/
///   `shipsFromPostalCode`/`shippingAddress` under `shipping`.
///
/// `L2L3Data.order_info.duty_amount` is deliberately unmapped: `TransactionInput` has no
/// duty field in the SDL, and folding duty into `surchargeAmount` (which Braintree
/// reserves for the Visa Rent Discount Program) would misreport it.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionEnrichment {
    #[serde(skip_serializing_if = "Option::is_none")]
    purchase_order_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax: Option<TransactionTaxInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping: Option<TransactionShippingInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_items: Option<Vec<TransactionLineItemInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor: Option<TransactionDescriptorInput>,
}

/// `TransactionLineItemInput.name` is capped at 35 characters by Braintree.
const LINE_ITEM_NAME_MAX_LEN: usize = 35;
/// `TransactionInput.lineItems` accepts at most 249 entries.
const MAX_LINE_ITEMS: usize = 249;

/// Converts a UCS address (plus its phone, when the caller has one) into Braintree's
/// shared `AddressInput`.
///
/// Returns `None` when nothing at all could be mapped, so an empty `{}` is never sent.
fn build_braintree_address(
    address: Option<&AddressDetails>,
    phone: Option<&PhoneDetails>,
) -> Option<BraintreeAddressInput> {
    // `PhoneInput` declares both `countryPhoneCode` and `phoneNumber` non-null, so the
    // phone object is emitted only when UCS holds both halves. `extract_country_code`
    // strips the leading `+`; Braintree wants the bare E.164 calling code.
    let phone_input = phone.and_then(|details| {
        Some(BraintreePhoneInput {
            country_phone_code: details.extract_country_code().ok()?,
            phone_number: details.number.clone()?,
        })
    });

    let built = BraintreeAddressInput {
        first_name: address.and_then(|address| address.first_name.clone()),
        last_name: address.and_then(|address| address.last_name.clone()),
        street_address: address.and_then(|address| address.line1.clone()),
        extended_address: address.and_then(|address| address.line2.clone()),
        locality: address.and_then(|address| address.city.clone()),
        region: address.and_then(|address| address.state.clone()),
        postal_code: address.and_then(|address| address.zip.clone()),
        country_code: address
            .and_then(|address| address.country)
            .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
        phone: phone_input,
    };

    (!built.is_empty()).then_some(built)
}

/// Builds `options.billingAddress` from the payment-method billing address UCS already
/// holds. Purely additive: before this, the only billing datum reaching Braintree was
/// `customerDetails.email`.
fn build_billing_address(flow_data: &PaymentFlowData) -> Option<BraintreeAddressInput> {
    let billing = flow_data.get_optional_billing();
    build_braintree_address(
        billing.and_then(|billing| billing.address.as_ref()),
        billing.and_then(|billing| billing.phone.as_ref()),
    )
}

/// Maps one UCS order line onto Braintree's `TransactionLineItemInput`.
///
/// All five non-null SDL fields are always produced:
/// * `kind` is always `DEBIT` — a Level 3 line item on an Authorize is a purchase.
///   `CREDIT` describes a refunded line and is unreachable on this flow.
/// * `totalAmount` uses the caller's own total when it supplied one, otherwise the SDL's
///   own definition of the field, `quantity * unitAmount`. This is a derivation, not a
///   silent default.
fn build_line_item(
    item: &OrderDetailsWithAmount,
    currency: enums::Currency,
    amount_converter: &'static (dyn common_utils::types::AmountConvertor<Output = StringMajorUnit>
                  + Sync),
) -> Result<TransactionLineItemInput, Report<IntegrationError>> {
    let convert = |amount: MinorUnit| -> Result<StringMajorUnit, Report<IntegrationError>> {
        amount_converter.convert(amount, currency).change_context(
            IntegrationError::AmountConversionFailed {
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that every line-item amount is a valid minor-unit value for the \
                         payment currency."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "failed to convert a Level 3 line-item amount to major units".to_string(),
                    ),
                },
            },
        )
    };

    let total_amount = item.total_amount.unwrap_or_else(|| {
        MinorUnit::new(item.amount.get_amount_as_i64() * i64::from(item.quantity))
    });

    Ok(TransactionLineItemInput {
        // Braintree caps the name at 35 characters and rejects the whole transaction
        // beyond that, so a long product name is truncated rather than failing an
        // otherwise-good payment over Level 3 enrichment.
        name: item
            .product_name
            .chars()
            .take(LINE_ITEM_NAME_MAX_LEN)
            .collect(),
        kind: TransactionLineItemType::Debit,
        quantity: item.quantity.to_string(),
        unit_amount: convert(item.amount)?,
        total_amount: convert(total_amount)?,
        tax_amount: item.total_tax_amount.map(convert).transpose()?,
        discount_amount: item.unit_discount_amount.map(convert).transpose()?,
        unit_of_measure: item.unit_of_measure.clone(),
        product_code: item.product_id.clone(),
        commodity_code: item.commodity_code.clone(),
        description: item.description.clone(),
        url: item.product_link.clone(),
    })
}

/// Builds the Level 2 / Level 3, shipping and dynamic-descriptor block for an Authorize.
///
/// Every money field goes through the connector's `StringMajorUnit` amount converter —
/// including `tax.taxAmount`, which the SDL types as the `Amount` scalar while the rest of
/// this area is plain `String`. Both land on the wire as the same major-unit decimal
/// string, so one converter covers both.
fn build_transaction_enrichment<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    amount_converter: &'static (dyn common_utils::types::AmountConvertor<Output = StringMajorUnit>
                  + Sync),
) -> Result<TransactionEnrichment, Report<IntegrationError>> {
    let currency = router_data.request.currency;
    let convert = |amount: MinorUnit| -> Result<StringMajorUnit, Report<IntegrationError>> {
        amount_converter.convert(amount, currency).change_context(
            IntegrationError::AmountConversionFailed {
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that every Level 2/Level 3 amount is a valid minor-unit value for \
                         the payment currency."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "failed to convert a Level 2/Level 3 amount to major units".to_string(),
                    ),
                },
            },
        )
    };

    let flow_data = &router_data.resource_common_data;
    let l2_l3 = flow_data.l2_l3_data.as_deref();

    // --- FLAT on TransactionInput ------------------------------------------------
    // Braintree's `purchaseOrderNumber` is the merchant's own purchase-order / order
    // reference and is required for Level 2 processing. `orderId` stays the UCS
    // `connector_request_reference_id` so Authorize and PSync keep reporting one id.
    let purchase_order_number = l2_l3.and_then(|data| data.get_merchant_order_reference_id());
    let discount_amount = l2_l3
        .and_then(|data| data.get_discount_amount())
        .map(convert)
        .transpose()?;

    // --- NESTED under `tax` ------------------------------------------------------
    let tax_amount = l2_l3
        .and_then(|data| data.get_order_tax_amount())
        .or(router_data.request.order_tax_amount)
        .map(convert)
        .transpose()?;
    let tax_exempt = l2_l3
        .and_then(|data| data.get_tax_status())
        .map(|status| matches!(status, common_enums::TaxStatus::Exempt));
    let tax = (tax_amount.is_some() || tax_exempt.is_some()).then_some(TransactionTaxInput {
        tax_amount,
        tax_exempt,
    });

    // --- NESTED under `shipping` -------------------------------------------------
    let shipping_amount = l2_l3
        .and_then(|data| data.get_shipping_cost())
        .map(convert)
        .transpose()?;
    let shipping_tax_amount = l2_l3
        .and_then(|data| data.get_shipping_amount_tax())
        .map(convert)
        .transpose()?;
    let ships_from_postal_code = l2_l3.and_then(|data| data.get_shipping_origin_zip());
    let shipping_address = build_braintree_address(
        // The Level 3 shipping address wins when the caller sent one; otherwise the
        // ordinary shipping address on the payment request is used.
        l2_l3
            .and_then(|data| data.shipping_details.as_ref())
            .or_else(|| {
                flow_data
                    .get_optional_shipping()
                    .and_then(|shipping| shipping.address.as_ref())
            }),
        flow_data
            .get_optional_shipping()
            .and_then(|shipping| shipping.phone.as_ref()),
    );
    let shipping = (shipping_address.is_some()
        || shipping_amount.is_some()
        || shipping_tax_amount.is_some()
        || ships_from_postal_code.is_some())
    .then_some(TransactionShippingInput {
        shipping_address,
        shipping_amount,
        shipping_tax_amount,
        ships_from_postal_code,
    });

    // --- FLAT: Level 3 line items ------------------------------------------------
    let line_items = l2_l3
        .and_then(|data| data.get_order_details())
        .or_else(|| flow_data.order_details.clone())
        .map(|items| {
            items
                .iter()
                .take(MAX_LINE_ITEMS)
                .map(|item| build_line_item(item, currency, amount_converter))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .filter(|items| !items.is_empty());

    // --- FLAT: dynamic descriptor ------------------------------------------------
    let descriptor =
        router_data
            .request
            .billing_descriptor
            .as_ref()
            .and_then(|billing_descriptor| {
                let descriptor = TransactionDescriptorInput {
                    name: billing_descriptor.name.clone(),
                    phone: billing_descriptor.phone.clone(),
                };
                (descriptor.name.is_some() || descriptor.phone.is_some()).then_some(descriptor)
            });

    Ok(TransactionEnrichment {
        purchase_order_number,
        discount_amount,
        tax,
        shipping,
        line_items,
        descriptor,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VaultTiming {
    Always,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionTiming {
    when: VaultTiming,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        BraintreeRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        String,
        BraintreeMeta,
    )> for MandatePaymentRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        (item, connector_mandate_id, metadata): (
            BraintreeRouterData<
                RouterDataV2<
                    RepeatPayment,
                    PaymentFlowData,
                    RepeatPaymentData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            String,
            BraintreeMeta,
        ),
    ) -> Result<Self, Self::Error> {
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
        let (query, transaction_body) = (
            match item.router_data.request.is_auto_capture() {
                true => constants::CHARGE_CREDIT_CARD_MUTATION.to_string(),
                false => constants::AUTHORIZE_CREDIT_CARD_MUTATION.to_string(),
            },
            TransactionBody::Mandate(MandateTransactionBody {
                amount,
                merchant_account_id: metadata.merchant_account_id,
                channel: constants::CHANNEL_CODE.to_string(),
                order_id: item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                payment_initiator: PaymentInitiatorType::Unscheduled,
            }),
        );
        Ok(Self {
            query,
            variables: VariablePaymentInput {
                input: PaymentInput {
                    payment_method_id: connector_mandate_id.into(),
                    transaction: transaction_body,
                    options: None,
                },
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BraintreePaymentsRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let metadata: BraintreeMeta = if let (
            Some(merchant_account_id),
            Some(merchant_config_currency),
        ) = (
            item.router_data.request.merchant_account_id.clone(),
            item.router_data.request.merchant_config_currency,
        ) {
            info!(
                "BRAINTREE: Picking merchant_account_id and merchant_config_currency from payments request"
              );
            BraintreeMeta {
                merchant_account_id: merchant_account_id.into(),
                merchant_config_currency,
            }
        } else {
            let auth = BraintreeAuthType::try_from(&item.router_data.connector_config)?;
            let merchant_account_id =
                auth.merchant_account_id
                    .ok_or(IntegrationError::InvalidConnectorConfig {
                        config: "merchant_account_id",
                        context: Default::default(),
                    })?;
            let merchant_config_currency = auth
                .merchant_config_currency
                .as_deref()
                .and_then(|s| s.parse::<enums::Currency>().ok())
                .ok_or(IntegrationError::InvalidConnectorConfig {
                    config: "merchant_config_currency",
                    context: Default::default(),
                })?;
            BraintreeMeta {
                merchant_account_id,
                merchant_config_currency,
            }
        };
        validate_currency(
            item.router_data.request.currency,
            Some(metadata.merchant_config_currency),
        )?;
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(_) => {
                if item.router_data.resource_common_data.is_three_ds()
                    && item.router_data.request.authentication_data.is_none()
                {
                    Ok(Self::CardThreeDs(BraintreeClientTokenRequest::try_from(
                        metadata,
                    )?))
                } else {
                    Ok(Self::Card(CardPaymentRequest::try_from((item, metadata))?))
                }
            }
            PaymentMethodData::Wallet(ref wallet_data) => {
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
                let order_id = item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone();
                let merchant_account_id = metadata.merchant_account_id.clone();
                let is_auto_capture = item.router_data.request.is_auto_capture();

                match wallet_data {
                    WalletData::GooglePayThirdPartySdk(ref req_wallet) => {
                        let payment_method_id = &req_wallet.token;
                        let query = if is_auto_capture {
                            constants::CHARGE_GOOGLE_PAY_MUTATION.to_string()
                        } else {
                            constants::AUTHORIZE_GOOGLE_PAY_MUTATION.to_string()
                        };
                        Ok(Self::Wallet(BraintreeWalletRequest {
                            query,
                            variables: GenericVariableInput {
                                input: WalletPaymentInput {
                                    payment_method_id: payment_method_id.clone().ok_or(
                                        IntegrationError::MissingRequiredField {
                                            field_name: "google_pay token",
                                            context: Default::default(),
                                        },
                                    )?,
                                    transaction: WalletTransactionBody {
                                        amount: amount.clone(),
                                        merchant_account_id: merchant_account_id.clone(),
                                        order_id: order_id.clone(),
                                        customer_details: None,
                                        vault_payment_method_after_transacting: None,
                                    },
                                },
                            },
                        }))
                    }
                    WalletData::ApplePayThirdPartySdk(ref req_wallet) => {
                        let payment_method_id = &req_wallet.token;
                        let is_mandate = item.router_data.request.is_mandate_payment();

                        let (query, customer_details, vault_payment_method_after_transacting) =
                            if is_mandate {
                                (
                                    if is_auto_capture {
                                        constants::CHARGE_AND_VAULT_APPLE_PAY_MUTATION.to_string()
                                    } else {
                                        constants::AUTHORIZE_AND_VAULT_APPLE_PAY_MUTATION
                                            .to_string()
                                    },
                                    item.router_data
                                        .resource_common_data
                                        .get_billing_email()
                                        .ok()
                                        .map(|email| CustomerBody { email }),
                                    Some(TransactionTiming {
                                        when: VaultTiming::Always,
                                    }),
                                )
                            } else {
                                (
                                    if is_auto_capture {
                                        constants::CHARGE_APPLE_PAY_MUTATION.to_string()
                                    } else {
                                        constants::AUTHORIZE_APPLE_PAY_MUTATION.to_string()
                                    },
                                    None,
                                    None,
                                )
                            };

                        Ok(Self::Wallet(BraintreeWalletRequest {
                            query,
                            variables: GenericVariableInput {
                                input: WalletPaymentInput {
                                    payment_method_id: payment_method_id.clone().ok_or(
                                        IntegrationError::MissingRequiredField {
                                            field_name: "apple_pay token",
                                            context: Default::default(),
                                        },
                                    )?,
                                    transaction: WalletTransactionBody {
                                        amount: amount.clone(),
                                        merchant_account_id: merchant_account_id.clone(),
                                        order_id: order_id.clone(),
                                        customer_details,
                                        vault_payment_method_after_transacting,
                                    },
                                },
                            },
                        }))
                    }
                    WalletData::PaypalSdk(ref req_wallet) => {
                        let payment_method_id = req_wallet.token.clone();
                        let query = match is_auto_capture {
                            true => constants::CHARGE_PAYPAL_MUTATION.to_string(),
                            false => constants::AUTHORIZE_PAYPAL_MUTATION.to_string(),
                        };
                        Ok(Self::Wallet(BraintreeWalletRequest {
                            query,
                            variables: GenericVariableInput {
                                input: WalletPaymentInput {
                                    payment_method_id: payment_method_id.into(),
                                    transaction: WalletTransactionBody {
                                        amount: amount.clone(),
                                        merchant_account_id: merchant_account_id.clone(),
                                        order_id: order_id.clone(),
                                        customer_details: None,
                                        vault_payment_method_after_transacting: None,
                                    },
                                },
                            },
                        }))
                    }
                    _ => Err(error_stack::report!(IntegrationError::NotSupported {
                        message: utils::get_unimplemented_payment_method_error_message("braintree"),
                        connector: "Braintree",
                        context: Default::default(),
                    })),
                }
            }
            // A card the caller has already exchanged for a Braintree single-use token via
            // `PaymentMethodService/Tokenize` arrives here as `PaymentMethodToken`. Every
            // Braintree card mutation is keyed on that token — `CardPaymentRequest` reads it
            // straight out of `payment_method_data` — so a tokenized card routes to the same
            // builder as a raw card. The 3DS client-token branch above stays on
            // `PaymentMethodData::Card` because it needs the card BIN to build the redirect
            // form, which a bare token cannot supply.
            //
            // A token tagged Apple Pay or Google Pay is NOT a card token: those wallets are
            // served by the `chargePaymentMethod` / `authorizePaymentMethod` mutations off the
            // `Wallet` arm above, so they stay unsupported here rather than being charged as a
            // credit card.
            PaymentMethodData::PaymentMethodToken(ref token_data)
                if token_data.token_payment_method_type.is_none() =>
            {
                Ok(Self::Card(CardPaymentRequest::try_from((item, metadata))?))
            }
            PaymentMethodData::MandatePayment
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("braintree"),
                    connector: "Braintree",
                    context: Default::default(),
                }))
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthResponse {
    data: DataAuthResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeAuthResponse {
    AuthResponse(Box<AuthResponse>),
    ClientTokenResponse(Box<ClientTokenResponse>),
    ErrorResponse(Box<ErrorResponse>),
    WalletAuthResponse(Box<WalletAuthResponse>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeCompleteAuthResponse {
    AuthResponse(Box<AuthResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaymentMethodInfo {
    pub id: Secret<String>,
}

/// Braintree GraphQL `AvsCvvResponseCode` — the processing bank's verdict on the AVS and
/// CVV checks. A closed enum in the SDL; `Unknown` only guards against Braintree adding a
/// value, so an unrecognised code never fails the whole response deserialization.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreeAvsCvvResponse {
    Bypass,
    DoesNotMatch,
    IssuerDoesNotParticipate,
    Matches,
    NotApplicable,
    NotProvided,
    NotVerified,
    SystemError,
    #[serde(other)]
    Unknown,
}

/// Braintree GraphQL `TransactionAuthorizationProcessorResponse`.
///
/// The field names here are the ones that actually exist in the SDL. The plausible
/// `…ResponseCode` spellings (`avsPostalCodeResponseCode`, `cvvResponseCode`,
/// `processorResponseCode`, `processorResponseText`, `additionalProcessorResponse`) belong
/// to `ExternalProcessorResponseInput` — an *input* type for importing a third-party
/// processor's result — and are not valid in a selection set.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeProcessorResponse {
    /// The processor's authorization response code, e.g. `2000` "Do Not Honor".
    pub legacy_code: Option<String>,
    /// The text explanation of `legacyCode`.
    pub message: Option<String>,
    pub cvv_response: Option<BraintreeAvsCvvResponse>,
    pub avs_postal_code_response: Option<BraintreeAvsCvvResponse>,
    pub avs_street_address_response: Option<BraintreeAvsCvvResponse>,
    pub authorization_id: Option<String>,
    pub additional_information: Option<String>,
    pub retrieval_reference_number: Option<String>,
}

/// Braintree GraphQL `PaymentNetworkResponse` and `MerchantAdviceCodeResponse` — both are
/// `{ code, message }`, so one Rust type covers both.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeCodeMessage {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// One entry of `Transaction.statusHistory`.
///
/// The `PaymentStatusEvent` interface exposes only `status`/`timestamp`/`amount`/`source`/
/// `terminal`, so everything useful here arrives through an inline fragment and is present
/// on some concrete event types and absent on others. Every field is therefore optional and
/// this one struct serves `AuthorizedEvent`, `ProcessorDeclinedEvent`,
/// `GatewayRejectedEvent` and `FailedEvent` alike.
///
/// This type is read ONLY for issuer/network diagnostics. The attempt status is never
/// derived from it — `Transaction.status` remains the single source of truth — so a missing
/// or unrecognised `statusHistory` can never turn a payment terminal.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreePaymentStatusEvent {
    pub status: Option<BraintreePaymentStatus>,
    pub terminal: Option<bool>,
    /// `ProcessorDeclinedEvent.declineType` — whether the decline is temporary (soft).
    pub decline_type: Option<String>,
    /// `GatewayRejectedEvent.gatewayRejectionReason` — AVS, CVV, fraud, duplicate, …
    pub gateway_rejection_reason: Option<String>,
    pub processor_response: Option<BraintreeProcessorResponse>,
    pub network_response: Option<BraintreeCodeMessage>,
    /// Present only on `ProcessorDeclinedEvent`, `GatewayRejectedEvent` and `FailedEvent`.
    pub merchant_advice_code_response: Option<BraintreeCodeMessage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAuthChargeResponseBody {
    id: String,
    status: BraintreePaymentStatus,
    payment_method: Option<PaymentMethodInfo>,
    /// Echo of `transaction.orderId`, i.e. the `connector_request_reference_id` this
    /// Authorize sent. Surfaced as `connector_response_reference_id` so the caller sees the
    /// same reference id Braintree stored.
    order_id: Option<String>,
    /// Flat, fragment-free AVS / CVV / processor read path. Populated on approvals and
    /// declines alike.
    processor_authorization_response: Option<BraintreeProcessorResponse>,
    /// Reverse-chronological (most recent first). Walked only to reach
    /// `merchantAdviceCodeResponse`, which has no flat path on `Transaction`.
    status_history: Option<Vec<BraintreePaymentStatusEvent>>,
}

impl TransactionAuthChargeResponseBody {
    /// The first `statusHistory` entry that carries issuer diagnostics. `statusHistory` is
    /// reverse-chronological, so this is the most recent decline/reject/fail event.
    fn diagnostic_event(&self) -> Option<&BraintreePaymentStatusEvent> {
        self.status_history.as_ref()?.iter().find(|event| {
            event.merchant_advice_code_response.is_some()
                || event.network_response.is_some()
                || event.processor_response.is_some()
        })
    }

    /// Best available processor response: the flat `processorAuthorizationResponse` when
    /// Braintree populated it, otherwise the one hanging off the status event.
    fn processor_response(&self) -> Option<&BraintreeProcessorResponse> {
        self.processor_authorization_response
            .as_ref()
            .or_else(|| self.diagnostic_event()?.processor_response.as_ref())
    }

    /// Surfaces the AVS and CVV verdicts on `PaymentFlowData.connector_response` so callers
    /// can reason about address/CVV mismatches without re-fetching the transaction.
    fn build_connector_response_data(&self) -> Option<ConnectorResponseData> {
        let processor_response = self.processor_response()?;
        let payment_checks = serde_json::json!({
            "avs_postal_code_response": processor_response.avs_postal_code_response.map(|code| code.to_string()),
            "avs_street_address_response": processor_response.avs_street_address_response.map(|code| code.to_string()),
            "cvv_response": processor_response.cvv_response.map(|code| code.to_string()),
            "processor_response_code": processor_response.legacy_code,
            "processor_response_text": processor_response.message,
            "authorization_id": processor_response.authorization_id,
            "retrieval_reference_number": processor_response.retrieval_reference_number,
            "additional_information": processor_response.additional_information,
        });

        Some(ConnectorResponseData::with_additional_payment_method_data(
            AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: Some(payment_checks),
                card_network: None,
                domestic_network: None,
                auth_code: processor_response.authorization_id.clone(),
            },
        ))
    }

    /// Builds the terminal-decline error, carrying the issuer codes Hyperswitch GSM keys
    /// smart-retry off.
    ///
    /// This is only ever reached once `Transaction.status` has already been mapped to a
    /// terminal failure by `is_payment_failure` — the codes read here refine that error,
    /// they never create one. An ambiguous or missing `statusHistory` leaves the extra
    /// fields `None` and changes nothing.
    fn build_failure_error_response(
        &self,
        status: enums::AttemptStatus,
        http_code: u16,
    ) -> domain_types::router_data::ErrorResponse {
        let mut error_response =
            create_failure_error_response(self.status.clone(), Some(self.id.clone()), http_code);

        let event = self.diagnostic_event();
        let processor_response = self.processor_response();

        // `network_advice_code` is the Mastercard merchant advice code (MAC). It exists on
        // no flat path — only on ProcessorDeclinedEvent / GatewayRejectedEvent / FailedEvent.
        error_response.network_advice_code = event
            .and_then(|event| event.merchant_advice_code_response.as_ref())
            .and_then(|advice| advice.code.clone());
        // `network_decline_code` is the processor's own authorization response code
        // (`legacyCode`), falling back to the card network's response code.
        error_response.network_decline_code = processor_response
            .and_then(|response| response.legacy_code.clone())
            .or_else(|| {
                event
                    .and_then(|event| event.network_response.as_ref())
                    .and_then(|network| network.code.clone())
            });
        error_response.network_error_message = processor_response
            .and_then(|response| response.message.clone())
            .or_else(|| {
                event
                    .and_then(|event| event.merchant_advice_code_response.as_ref())
                    .and_then(|advice| advice.message.clone())
            });

        // Prefer the processor's own code/text over the bare Braintree status string, so the
        // merchant sees "2000 / Do Not Honor" rather than just "PROCESSOR_DECLINED".
        if let Some(response) = processor_response {
            if let Some(code) = response.legacy_code.clone() {
                error_response.code = code;
            }
            if let Some(message) = response.message.clone() {
                error_response.message = message;
            }
        }
        // Carry the decline/rejection qualifier through as the reason when Braintree gave one.
        if let Some(reason) = event.and_then(|event| {
            event
                .gateway_rejection_reason
                .clone()
                .or_else(|| event.decline_type.clone())
        }) {
            error_response.reason = Some(reason);
        }

        // Keep the precise status the caller just computed rather than letting the error
        // report an unspecified one. `status` here is always a terminal payment failure:
        // this method is only reached from a branch gated on `is_payment_failure`, i.e. on
        // an explicit Braintree decline, never on an ambiguous or transport-level outcome.
        error_response.attempt_status = Some(FlowStatus::Payment(status));

        error_response
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataAuthResponse {
    authorize_credit_card: AuthChargeCreditCard,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthChargeCreditCard {
    transaction: TransactionAuthChargeResponseBody,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreeAuthResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreeAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeAuthResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreeAuthResponse::AuthResponse(auth_response) => {
                let transaction_data = auth_response.data.authorize_credit_card.transaction;
                let status = enums::AttemptStatus::from(transaction_data.status.clone());
                // AVS/CVV/processor codes are reported on approvals and declines alike.
                let connector_response = transaction_data.build_connector_response_data();
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(transaction_data.build_failure_error_response(status, item.http_code))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            transaction_data.id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: transaction_data.payment_method.as_ref().map(|pm| {
                            Box::new(MandateReference {
                                connector_mandate_id: Some(pm.id.clone().expose()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata: None,
                            })
                        }),
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: transaction_data.order_id.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    })
                };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_response,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
            BraintreeAuthResponse::WalletAuthResponse(wallet_response) => {
                let transaction_data = &wallet_response.data.authorize_payment_method.transaction;
                let status = enums::AttemptStatus::from(transaction_data.status.clone());

                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_failure_error_response(
                        transaction_data.status.clone(),
                        Some(transaction_data.id.clone()),
                        item.http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            transaction_data.id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: transaction_data.legacy_id.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    })
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
            BraintreeAuthResponse::ClientTokenResponse(client_token_data) => {
                let payment_method_token = match &item.router_data.request.payment_method_data {
                    PaymentMethodData::PaymentMethodToken(t) => t.token.clone(),
                    _ => {
                        return Err(utils::response_handling_fail_for_connector(
                            item.http_code,
                            "braintree",
                        )
                        .into());
                    }
                };
                let complete_authorize_url =
                    match item.router_data.request.get_complete_authorize_url() {
                        Ok(u) => u,
                        Err(_) => {
                            return Err(utils::response_handling_fail_for_connector(
                                item.http_code,
                                "braintree",
                            )
                            .into());
                        }
                    };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: enums::AttemptStatus::AuthenticationPending,
                        ..item.router_data.resource_common_data.clone()
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::NoResponseId,
                        redirection_data: Some(Box::new(get_braintree_redirect_form(
                            *client_token_data,
                            payment_method_token,
                            item.router_data.request.payment_method_data.clone(),
                            complete_authorize_url,
                        )?)),
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
    }
}

fn build_error_response<T>(
    response: &[ErrorDetails],
    http_code: u16,
) -> Result<T, Box<domain_types::router_data::ErrorResponse>> {
    let error_messages = response
        .iter()
        .map(|error| error.message.to_string())
        .collect::<Vec<String>>();

    let reason = match !error_messages.is_empty() {
        true => Some(error_messages.join(" ")),
        false => None,
    };

    get_error_response(
        response
            .first()
            .and_then(|err_details| err_details.extensions.as_ref())
            .and_then(|extensions| extensions.legacy_code.clone()),
        response
            .first()
            .map(|err_details| err_details.message.clone()),
        reason,
        http_code,
    )
}

fn get_error_response<T>(
    error_code: Option<String>,
    error_msg: Option<String>,
    error_reason: Option<String>,
    http_code: u16,
) -> Result<T, Box<domain_types::router_data::ErrorResponse>> {
    Err(Box::new(domain_types::router_data::ErrorResponse {
        code: error_code.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: error_msg.unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: error_reason,
        status_code: http_code,
        attempt_status: None,
        connector_transaction_id: None,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }))
}

fn create_failure_error_response<T: ToString>(
    status: T,
    connector_id: Option<String>,
    http_code: u16,
) -> domain_types::router_data::ErrorResponse {
    let status_string = status.to_string();
    domain_types::router_data::ErrorResponse {
        code: status_string.clone(),
        message: status_string.clone(),
        reason: Some(status_string),
        attempt_status: None,
        connector_transaction_id: connector_id,
        status_code: http_code,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreePaymentStatus {
    Authorized,
    Authorizing,
    AuthorizedExpired,
    Failed,
    ProcessorDeclined,
    GatewayRejected,
    Voided,
    Settling,
    Settled,
    SettlementPending,
    SettlementDeclined,
    SettlementConfirmed,
    SubmittedForSettlement,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorDetails {
    pub message: String,
    pub extensions: Option<AdditionalErrorDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalErrorDetails {
    pub legacy_code: Option<String>,
}

impl From<BraintreePaymentStatus> for enums::AttemptStatus {
    fn from(item: BraintreePaymentStatus) -> Self {
        match item {
            BraintreePaymentStatus::Settling
            | BraintreePaymentStatus::Settled
            | BraintreePaymentStatus::SettlementConfirmed
            | BraintreePaymentStatus::SubmittedForSettlement
            | BraintreePaymentStatus::SettlementPending => Self::Charged,
            BraintreePaymentStatus::Authorizing => Self::Authorizing,
            BraintreePaymentStatus::AuthorizedExpired => Self::AuthorizationFailed,
            BraintreePaymentStatus::Failed
            | BraintreePaymentStatus::GatewayRejected
            | BraintreePaymentStatus::ProcessorDeclined
            | BraintreePaymentStatus::SettlementDeclined => Self::Failure,
            BraintreePaymentStatus::Authorized => Self::Authorized,
            BraintreePaymentStatus::Voided => Self::Voided,
        }
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreePaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreePaymentsResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors.clone(), item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreePaymentsResponse::PaymentsResponse(payment_response) => {
                let transaction_data = payment_response.data.charge_credit_card.transaction;
                let status = enums::AttemptStatus::from(transaction_data.status.clone());
                // AVS/CVV/processor codes are reported on approvals and declines alike.
                let connector_response = transaction_data.build_connector_response_data();
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(transaction_data.build_failure_error_response(status, item.http_code))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            transaction_data.id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: transaction_data.payment_method.as_ref().map(|pm| {
                            Box::new(MandateReference {
                                connector_mandate_id: Some(pm.id.clone().expose()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata: None,
                            })
                        }),
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: transaction_data.order_id.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    })
                };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_response,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
            BraintreePaymentsResponse::WalletPaymentsResponse(wallet_response) => {
                let transaction_data = &wallet_response.data.charge_payment_method.transaction;
                let status = enums::AttemptStatus::from(transaction_data.status.clone());

                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_failure_error_response(
                        transaction_data.status.clone(),
                        Some(transaction_data.id.clone()),
                        item.http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            transaction_data.id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: transaction_data.payment_method.as_ref().map(|pm| {
                            Box::new(MandateReference {
                                connector_mandate_id: Some(pm.id.clone().expose()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata: None,
                            })
                        }),
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: transaction_data.legacy_id.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    })
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
            BraintreePaymentsResponse::ClientTokenResponse(client_token_data) => {
                let payment_method_token = match &item.router_data.request.payment_method_data {
                    PaymentMethodData::PaymentMethodToken(t) => t.token.clone(),
                    _ => {
                        return Err(utils::response_handling_fail_for_connector(
                            item.http_code,
                            "braintree",
                        )
                        .into());
                    }
                };
                let complete_authorize_url =
                    match item.router_data.request.get_complete_authorize_url() {
                        Ok(u) => u,
                        Err(_) => {
                            return Err(utils::response_handling_fail_for_connector(
                                item.http_code,
                                "braintree",
                            )
                            .into());
                        }
                    };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: enums::AttemptStatus::AuthenticationPending,
                        ..item.router_data.resource_common_data.clone()
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::NoResponseId,
                        redirection_data: Some(Box::new(get_braintree_redirect_form(
                            *client_token_data,
                            payment_method_token,
                            item.router_data.request.payment_method_data.clone(),
                            complete_authorize_url,
                        )?)),

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
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaymentsResponse {
    data: DataResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WalletPaymentsResponse {
    pub data: WalletDataResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletDataResponse {
    pub charge_payment_method: WalletTransactionWrapper,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WalletTransactionWrapper {
    pub transaction: WalletTransaction,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletTransaction {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_id: Option<String>,
    pub status: BraintreePaymentStatus,
    pub amount: WalletAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<PaymentMethodInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAmount {
    pub value: String,
    pub currency_code: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WalletAuthResponse {
    pub data: WalletAuthDataResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletAuthDataResponse {
    pub authorize_payment_method: WalletTransactionWrapper,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreePaymentsResponse {
    PaymentsResponse(Box<PaymentsResponse>),
    WalletPaymentsResponse(Box<WalletPaymentsResponse>),
    ClientTokenResponse(Box<ClientTokenResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeCompleteChargeResponse {
    PaymentsResponse(Box<PaymentsResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataResponse {
    charge_credit_card: AuthChargeCreditCard,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundInputData {
    amount: StringMajorUnit,
    merchant_account_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order_id: Option<String>,
}
#[derive(Serialize, Debug, Clone)]
struct IdFilter {
    is: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionSearchInput {
    id: IdFilter,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeRefundInput {
    transaction_id: String,
    refund: RefundInputData,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for BraintreeRefundRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = BraintreeAuthType::try_from(&item.router_data.connector_config)?;
        let metadata: BraintreeMeta = if let (Some(merchant_account_id), merchant_config_currency) = (
            item.router_data.request.merchant_account_id.clone(),
            item.router_data.request.currency,
        ) {
            BraintreeMeta {
                merchant_account_id: merchant_account_id.into(),
                merchant_config_currency,
            }
        } else {
            let merchant_account_id =
                auth.merchant_account_id
                    .ok_or(IntegrationError::InvalidConnectorConfig {
                        config: "merchant_account_id",
                        context: Default::default(),
                    })?;
            let merchant_config_currency = auth
                .merchant_config_currency
                .as_deref()
                .and_then(|s| s.parse::<enums::Currency>().ok())
                .ok_or(IntegrationError::InvalidConnectorConfig {
                    config: "merchant_config_currency",
                    context: Default::default(),
                })?;
            BraintreeMeta {
                merchant_account_id,
                merchant_config_currency,
            }
        };

        validate_currency(
            item.router_data.request.currency,
            Some(metadata.merchant_config_currency),
        )?;
        let query = constants::REFUND_TRANSACTION_MUTATION.to_string();
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
        let variables = BraintreeRefundVariables {
            input: BraintreeRefundInput {
                transaction_id: item.router_data.request.connector_transaction_id.clone(),
                refund: RefundInputData {
                    amount,
                    merchant_account_id: metadata.merchant_account_id,
                    order_id: Some(item.router_data.request.refund_id),
                },
            },
        };
        Ok(Self { query, variables })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreeRefundStatus {
    SettlementPending,
    Settling,
    Settled,
    SubmittedForSettlement,
    Failed,
}

impl From<BraintreeRefundStatus> for enums::RefundStatus {
    fn from(item: BraintreeRefundStatus) -> Self {
        match item {
            BraintreeRefundStatus::Settled
            | BraintreeRefundStatus::Settling
            | BraintreeRefundStatus::SubmittedForSettlement
            | BraintreeRefundStatus::SettlementPending => Self::Success,
            BraintreeRefundStatus::Failed => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeRefundTransactionBody {
    pub id: String,
    pub status: BraintreeRefundStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeRefundTransaction {
    pub refund: BraintreeRefundTransactionBody,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeRefundResponseData {
    pub refund_transaction: BraintreeRefundTransaction,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefundResponse {
    pub data: BraintreeRefundResponseData,
}

impl<F> TryFrom<ResponseRouterData<BraintreeRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreeRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: match item.response {
                BraintreeRefundResponse::ErrorResponse(error_response) => {
                    build_error_response(&error_response.errors, item.http_code).map_err(|err| *err)
                }
                BraintreeRefundResponse::SuccessResponse(refund_data) => {
                    let refund_data = refund_data.data.refund_transaction.refund;
                    let refund_status = enums::RefundStatus::from(refund_data.status.clone());
                    if utils::is_refund_failure(refund_status) {
                        Err(create_failure_error_response(
                            refund_data.status,
                            Some(refund_data.id),
                            item.http_code,
                        ))
                    } else {
                        Ok(RefundsResponseData {
                            connector_refund_id: refund_data.id.clone(),
                            refund_status,
                            status_code: item.http_code,
                            acquirer_reference_number: None,
                        })
                    }
                }
            },
            ..item.router_data
        })
    }
}

fn extract_metadata_field<T>(
    metadata: &Option<pii::SecretSerdeValue>,
    field_name: &'static str,
) -> Result<T, Report<IntegrationError>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    metadata
        .as_ref()
        .and_then(|metadata| {
            let exposed = metadata.clone().expose();
            exposed
                .get(field_name)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
        })
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name,
                context: Default::default(),
            }
            .into()
        })
}

fn extract_metadata_string_field(
    metadata: &Option<pii::SecretSerdeValue>,
    field_name: &'static str,
) -> Result<Secret<String>, Report<IntegrationError>> {
    metadata
        .as_ref()
        .and_then(|metadata| {
            let exposed = metadata.clone().expose();
            exposed
                .get(field_name)
                .and_then(|v| v.as_str())
                .map(|s| Secret::new(s.to_string()))
        })
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name,
                context: Default::default(),
            }
            .into()
        })
}

#[derive(Debug, Clone, Serialize)]
pub struct RefundSearchInput {
    id: IdFilter,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for BraintreeRSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = BraintreeAuthType::try_from(&item.router_data.connector_config)?;
        let metadata: BraintreeMeta =
            if let (Some(merchant_account_id), Some(merchant_config_currency)) = (
                extract_metadata_string_field(
                    &item.router_data.request.refund_connector_metadata,
                    "merchant_account_id",
                )
                .ok(),
                extract_metadata_field(
                    &item.router_data.request.refund_connector_metadata,
                    "merchant_config_currency",
                )
                .ok(),
            ) {
                BraintreeMeta {
                    merchant_account_id,
                    merchant_config_currency,
                }
            } else {
                let merchant_account_id =
                    auth.merchant_account_id
                        .ok_or(IntegrationError::InvalidConnectorConfig {
                            config: "merchant_account_id",
                            context: Default::default(),
                        })?;
                let merchant_config_currency = auth
                    .merchant_config_currency
                    .as_deref()
                    .and_then(|s| s.parse::<enums::Currency>().ok())
                    .ok_or(IntegrationError::InvalidConnectorConfig {
                        config: "merchant_config_currency",
                        context: Default::default(),
                    })?;
                BraintreeMeta {
                    merchant_account_id,
                    merchant_config_currency,
                }
            };
        let currency = extract_metadata_field(
            &item.router_data.request.refund_connector_metadata,
            "currency",
        )?;
        validate_currency(currency, Some(metadata.merchant_config_currency))?;
        let refund_id = item.router_data.request.connector_refund_id;
        Ok(Self {
            query: constants::REFUND_QUERY.to_string(),
            variables: RSyncInput {
                input: RefundSearchInput {
                    id: IdFilter { is: refund_id },
                },
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RSyncNodeData {
    id: String,
    status: BraintreeRefundStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RSyncEdgeData {
    node: RSyncNodeData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefundData {
    edges: Vec<RSyncEdgeData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RSyncSearchData {
    refunds: RefundData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RSyncResponseData {
    search: RSyncSearchData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RSyncResponse {
    data: RSyncResponseData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeRSyncResponse {
    RSyncResponse(Box<RSyncResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

impl<F> TryFrom<ResponseRouterData<BraintreeRSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BraintreeRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeRSyncResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreeRSyncResponse::RSyncResponse(rsync_response) => {
                let edge_data = rsync_response
                    .data
                    .search
                    .refunds
                    .edges
                    .first()
                    .ok_or_else(|| {
                        Report::new(ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some("Braintree RSync: no refund in search results".to_string()),
                        ))
                    })?;
                let connector_refund_id = &edge_data.node.id;
                let response = Ok(RefundsResponseData {
                    connector_refund_id: connector_refund_id.to_string(),
                    refund_status: enums::RefundStatus::from(edge_data.node.status.clone()),
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                });
                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    number: RawCardNumber<T>,
    expiration_year: Secret<String>,
    expiration_month: Secret<String>,
    cvv: Secret<String>,
    cardholder_name: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTokenInput {
    merchant_account_id: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    credit_card: CreditCardData<T>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputClientTokenData {
    client_token: ClientTokenInput,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for BraintreeTokenRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card_data) => Ok(Self {
                query: constants::TOKENIZE_CREDIT_CARD.to_string(),
                variables: VariableInput {
                    input: InputData {
                        credit_card: CreditCardData {
                            number: card_data.card_number,
                            expiration_year: card_data.card_exp_year,
                            expiration_month: card_data.card_exp_month,
                            cvv: card_data.card_cvc,
                            cardholder_name: item
                                .router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                                .unwrap_or(Secret::new("".to_string())),
                        },
                    },
                },
            }),
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("braintree"),
                    connector: "Braintree",
                    context: Default::default(),
                }))
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenizePaymentMethodData {
    id: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizeCreditCardData {
    payment_method: TokenizePaymentMethodData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientToken {
    client_token: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizeCreditCard {
    tokenize_credit_card: TokenizeCreditCardData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTokenData {
    create_client_token: ClientToken,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTokenExtensions {
    request_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientTokenResponse {
    data: ClientTokenData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenResponse {
    data: TokenizeCreditCard,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorResponse {
    errors: Vec<ErrorDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeTokenResponse {
    TokenResponse(Box<TokenResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreeTokenResponse, Self>>
    for RouterDataV2<
        F,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreeTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: match item.response {
                BraintreeTokenResponse::ErrorResponse(error_response) => {
                    build_error_response(error_response.errors.as_ref(), item.http_code)
                        .map_err(|err| *err)
                }

                BraintreeTokenResponse::TokenResponse(token_response) => {
                    Ok(PaymentMethodTokenResponse {
                        token: token_response
                            .data
                            .tokenize_credit_card
                            .payment_method
                            .id
                            .expose()
                            .clone(),
                        connector_payment_method_id: None,
                        status_code: item.http_code,
                    })
                }
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTransactionBody {
    amount: StringMajorUnit,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureInputData {
    transaction_id: String,
    transaction: CaptureTransactionBody,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for BraintreeCaptureRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let query = constants::CAPTURE_TRANSACTION_MUTATION.to_string();
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
        let variables = VariableCaptureInput {
            input: CaptureInputData {
                transaction_id: item
                    .router_data
                    .request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    })?,
                transaction: CaptureTransactionBody { amount },
            },
        };
        Ok(Self { query, variables })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CaptureResponseTransactionBody {
    id: String,
    status: BraintreePaymentStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CaptureTransactionData {
    transaction: CaptureResponseTransactionBody,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponseData {
    capture_transaction: CaptureTransactionData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CaptureResponse {
    data: CaptureResponseData,
}

impl<F, T> TryFrom<ResponseRouterData<BraintreeCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreeCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeCaptureResponse::SuccessResponse(capture_data) => {
                let transaction_data = capture_data.data.capture_transaction.transaction;
                let status = enums::AttemptStatus::from(transaction_data.status.clone());
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_failure_error_response(
                        transaction_data.status,
                        Some(transaction_data.id),
                        item.http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_data.id),
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
                    })
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
            BraintreeCaptureResponse::ErrorResponse(error_data) => Ok(Self {
                response: build_error_response(&error_data.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePaymentMethodFromVaultInputData {
    payment_method_id: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct VariableDeletePaymentMethodFromVaultInput {
    input: DeletePaymentMethodFromVaultInputData,
}

#[derive(Debug, Serialize)]
pub struct BraintreeRevokeMandateRequest {
    query: String,
    variables: VariableDeletePaymentMethodFromVaultInput,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeRevokeMandateResponse {
    RevokeMandateResponse(Box<RevokeMandateResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevokeMandateResponse {
    data: DeletePaymentMethodFromVault,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePaymentMethodFromVault {
    client_mutation_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelInputData {
    transaction_id: String,
}

#[derive(Debug, Serialize)]
pub struct VariableCancelInput {
    input: CancelInputData,
}

#[derive(Debug, Serialize)]
pub struct BraintreeCancelRequest {
    query: String,
    variables: VariableCancelInput,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for BraintreeCancelRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let query = constants::VOID_TRANSACTION_MUTATION.to_string();
        let variables = VariableCancelInput {
            input: CancelInputData {
                transaction_id: item.router_data.request.connector_transaction_id.clone(),
            },
        };
        Ok(Self { query, variables })
    }
}

#[derive(Debug, Clone, Display, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GooglePayPriceStatus {
    #[strum(serialize = "FINAL")]
    Final,
}

#[derive(Debug, Clone, Display, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaypalFlow {
    Checkout,
}

impl From<PaypalFlow> for connector_types::PaypalFlow {
    fn from(item: PaypalFlow) -> Self {
        match item {
            PaypalFlow::Checkout => Self::Checkout,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeSessionResponse {
    SessionTokenResponse(Box<ClientTokenResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BraintreeClientTokenRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = BraintreeAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_account_id =
            auth.merchant_account_id
                .ok_or(IntegrationError::InvalidConnectorConfig {
                    config: "merchant_account_id",
                    context: Default::default(),
                })?;
        Ok(Self {
            query: constants::CLIENT_TOKEN_MUTATION.to_owned(),
            variables: VariableClientTokenInput {
                input: InputClientTokenData {
                    client_token: ClientTokenInput {
                        merchant_account_id,
                    },
                },
            },
        })
    }
}

impl<F> TryFrom<ResponseRouterData<BraintreeSessionResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreeSessionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        match response {
            BraintreeSessionResponse::SessionTokenResponse(res) => {
                let auth = match BraintreeAuthType::try_from(&item.router_data.connector_config) {
                    Ok(a) => a,
                    Err(_) => {
                        return Err(Report::new(
                            ConnectorError::response_handling_failed_with_context(
                                item.http_code,
                                Some("BraintreeAuthType: connector_config rejected".to_string()),
                            ),
                        ));
                    }
                };
                let session_token = match item.router_data.request.payment_method_type {
                    Some(common_enums::PaymentMethodType::ApplePay) => {
                        let payment_request_data = PaymentRequestMetadata {
                            supported_networks: auth.apple_pay_supported_networks,
                            merchant_capabilities: auth.apple_pay_merchant_capabilities,
                            label: match auth.apple_pay_label {
                                Some(l) => l,
                                None => {
                                    return Err(Report::new(
                                        ConnectorError::response_handling_failed_with_context(
                                            item.http_code,
                                            Some("Braintree config: apple_pay_label missing".to_string()),
                                        ),
                                    ));
                                }
                            },
                        };

                        let session_response = Some(ApplePaySessionResponse::ThirdPartySdk(
                            ThirdPartySdkSessionResponse {
                                secrets: SecretInfoToInitiateSdk {
                                    display: res.data.create_client_token.client_token.clone(),
                                    payment: None,
                                },
                            },
                        ));
                        ClientAuthenticationTokenData::ApplePay(Box::new(
                            ApplepayClientAuthenticationResponse {
                                session_response,
                                payment_request_data: Some(ApplePayPaymentRequest {
                                    country_code: item.router_data.request.country.ok_or_else(|| Report::new(
                                        ConnectorError::response_handling_failed_with_context(
                                            item.http_code,
                                            Some("Apple Pay session: country missing".to_string()),
                                        ),
                                    ))?,
                                    currency_code: item.router_data.request.currency,
                                    total: AmountInfo {
                                        label: payment_request_data.label,
                                        total_type: None,
                                        amount: item.router_data.request.amount,
                                    },
                                    merchant_capabilities: Some(
                                        payment_request_data.merchant_capabilities,
                                    ),
                                    supported_networks: Some(
                                        payment_request_data.supported_networks,
                                    ),
                                    merchant_identifier: None,
                                    required_billing_contact_fields: None,
                                    required_shipping_contact_fields: None,
                                    recurring_payment_request: None,
                                }),
                                connector: BRAINTREE_CONNECTOR_NAME.to_string(),
                                delayed_session_token: false,
                                sdk_next_action: SdkNextAction {
                                    next_action: NextActionCall::Confirm,
                                },
                                connector_reference_id: None,
                                connector_sdk_public_key: None,
                                connector_merchant_id: None,
                            },
                        ))
                    }
                    Some(common_enums::PaymentMethodType::GooglePay) => {
                        ClientAuthenticationTokenData::GooglePay(Box::new(
                            GpayClientAuthenticationResponse::GooglePaySession(
                                GooglePaySessionResponse {
                                    merchant_info: GpayMerchantInfo {
                                        merchant_name: auth.gpay_merchant_name.unwrap_or_default(),
                                        merchant_id: auth.gpay_merchant_id,
                                    },
                                    shipping_address_required: false,
                                    email_required: false,
                                    shipping_address_parameters: GpayShippingAddressParameters {
                                        phone_number_required: false,
                                    },
                                    allowed_payment_methods: vec![GpayAllowedPaymentMethods {
                                        payment_method_type: "CARD".to_string(),
                                        parameters: GpayAllowedMethodsParameters {
                                            allowed_auth_methods: auth.gpay_allowed_auth_methods,
                                            allowed_card_networks: auth.gpay_allowed_card_networks,
                                            billing_address_required: None,
                                            billing_address_parameters: None,
                                            assurance_details_required: None,
                                        },
                                        tokenization_specification: GpayTokenizationSpecification {
                                            token_specification_type: "PAYMENT_GATEWAY".to_string(),
                                            parameters: GpayTokenParameters {
                                                gateway: Some("braintree".to_string()),
                                                gateway_merchant_id: auth
                                                    .gpay_gateway_merchant_id
                                                    .clone(),
                                                protocol_version: None,
                                                public_key: None,
                                            },
                                        },
                                    }],
                                    transaction_info: GpayTransactionInfo {
                                        country_code: item.router_data.request.country.ok_or_else(|| Report::new(
                                            ConnectorError::response_handling_failed_with_context(
                                                item.http_code,
                                                Some("Google Pay session: country missing".to_string()),
                                            ),
                                        ))?,
                                        currency_code: item.router_data.request.currency,
                                        total_price_status: GooglePayPriceStatus::Final.to_string(),
                                        total_price: item.router_data.request.amount,
                                    },
                                    secrets: Some(SecretInfoToInitiateSdk {
                                        display: res.data.create_client_token.client_token.clone(),
                                        payment: None,
                                    }),
                                    delayed_session_token: false,
                                    connector: BRAINTREE_CONNECTOR_NAME.to_string(),
                                    sdk_next_action: SdkNextAction {
                                        next_action: NextActionCall::Confirm,
                                    },
                                },
                            ),
                        ))
                    }
                    Some(common_enums::PaymentMethodType::Paypal) => {
                        let paypal_client_id = match auth.paypal_client_id {
                            Some(id) => id,
                            None => {
                                return Err(Report::new(
                                    ConnectorError::response_handling_failed_with_context(
                                        item.http_code,
                                        Some(
                                            "Braintree config: paypal_client_id missing"
                                                .to_string(),
                                        ),
                                    ),
                                ));
                            }
                        };

                        ClientAuthenticationTokenData::Paypal(Box::new(
                            PaypalClientAuthenticationResponse {
                                connector: BRAINTREE_CONNECTOR_NAME.to_string(),
                                session_token: paypal_client_id,
                                sdk_next_action: SdkNextAction {
                                    next_action: NextActionCall::Confirm,
                                },
                                client_token: Some(
                                    res.data.create_client_token.client_token.clone().expose(),
                                ),
                                transaction_info: Some(PaypalTransactionInfo {
                                    flow: PaypalFlow::Checkout.into(),
                                    currency_code: item.router_data.request.currency,
                                    total_price: item.router_data.request.amount,
                                }),
                            },
                        ))
                    }
                    _ => {
                        return Err(Report::new(
                            ConnectorError::unexpected_response_error_with_context(
                                item.http_code,
                                Some(format!(
                                    "Braintree SDK session: unsupported PM {:?}",
                                    item.router_data.request.payment_method_type
                                )),
                            ),
                        ));
                    }
                };

                Ok(Self {
                    response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                        session_data: session_token,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            BraintreeSessionResponse::ErrorResponse(error_response) => {
                let err = build_error_response(error_response.errors.as_ref(), item.http_code)
                    .map_err(|err| *err);
                Ok(Self {
                    response: err,
                    ..item.router_data
                })
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CancelResponseTransactionBody {
    id: String,
    status: BraintreePaymentStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CancelTransactionData {
    reversal: CancelResponseTransactionBody,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelResponseData {
    reverse_transaction: CancelTransactionData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CancelResponse {
    data: CancelResponseData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeCancelResponse {
    CancelResponse(Box<CancelResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

impl<F> TryFrom<ResponseRouterData<BraintreeCancelResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BraintreeCancelResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeCancelResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreeCancelResponse::CancelResponse(void_response) => {
                let void_data = void_response.data.reverse_transaction.reversal;
                let status = enums::AttemptStatus::from(void_data.status.clone());
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_failure_error_response(
                        void_data.status,
                        None,
                        item.http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::NoResponseId,
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
                    })
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
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for BraintreePSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let transaction_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;
        Ok(Self {
            query: constants::TRANSACTION_QUERY.to_string(),
            variables: PSyncInput {
                input: TransactionSearchInput {
                    id: IdFilter { is: transaction_id },
                },
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeData {
    id: String,
    status: BraintreePaymentStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EdgeData {
    node: NodeData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionData {
    edges: Vec<EdgeData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchData {
    transactions: TransactionData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PSyncResponseData {
    search: SearchData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PSyncResponse {
    data: PSyncResponseData,
}

impl<F> TryFrom<ResponseRouterData<BraintreePSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreePSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreePSyncResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreePSyncResponse::SuccessResponse(psync_response) => {
                let edge_data = psync_response
                    .data
                    .search
                    .transactions
                    .edges
                    .first()
                    .ok_or_else(|| {
                        Report::new(ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some("Braintree PSync: no transaction in search results".to_string()),
                        ))
                    })?;
                let status = enums::AttemptStatus::from(edge_data.node.status.clone());
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_failure_error_response(
                        edge_data.node.status.clone(),
                        None,
                        item.http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(edge_data.node.id.clone()),
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
                    })
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
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeThreeDsResponse {
    pub nonce: Secret<String>,
    pub liability_shifted: bool,
    pub liability_shift_possible: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeThreeDsErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct BraintreeRedirectionResponse {
    pub authentication_response: String,
}

fn get_card_isin_from_payment_method_data<T>(
    card_details: &PaymentMethodData<T>,
) -> Result<String, Report<IntegrationError>>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    match card_details {
        PaymentMethodData::Card(card_data) => {
            let card_number_str = format!("{:?}", card_data.card_number.0);
            let cleaned_number = card_number_str
                .chars()
                .filter(|c| c.is_ascii_digit())
                .take(6)
                .collect::<String>();
            Ok(cleaned_number)
        }
        _ => Err(error_stack::report!(IntegrationError::NotSupported {
            message: "given payment method".to_owned(),
            connector: "Braintree",
            context: Default::default(),
        })),
    }
}

impl TryFrom<BraintreeMeta> for BraintreeClientTokenRequest {
    type Error = Report<IntegrationError>;
    fn try_from(metadata: BraintreeMeta) -> Result<Self, Self::Error> {
        Ok(Self {
            query: constants::CLIENT_TOKEN_MUTATION.to_owned(),
            variables: VariableClientTokenInput {
                input: InputClientTokenData {
                    client_token: ClientTokenInput {
                        merchant_account_id: metadata.merchant_account_id,
                    },
                },
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        BraintreeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        BraintreeMeta,
    )> for CardPaymentRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        (item, metadata): (
            BraintreeRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            BraintreeMeta,
        ),
    ) -> Result<Self, Self::Error> {
        // Check for external 3DS authentication data
        let three_ds_data = item
            .router_data
            .request
            .authentication_data
            .as_ref()
            .map(|auth_data| {
                Ok::<_, Report<IntegrationError>>(ThreeDSecureAuthenticationInput {
                    pass_through: Some(convert_external_three_ds_data(auth_data)?),
                })
            })
            .transpose()?;

        // `billingAddress` is a sibling of `input.transaction` under `input.options`, not a
        // field of `TransactionInput` — so it shares the options object with the 3DS
        // pass-through, and the object is emitted when either half is present.
        let billing_address = build_billing_address(&item.router_data.resource_common_data);
        let options = (three_ds_data.is_some() || billing_address.is_some()).then_some(
            CreditCardTransactionOptions {
                billing_address,
                three_d_secure_authentication: three_ds_data,
            },
        );

        let enrichment =
            build_transaction_enrichment(&item.router_data, item.connector.amount_converter)?;

        let reference_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );
        let order_id =
            reference_id.ok_or(IntegrationError::MissingConnectorRelatedTransactionID {
                id: "order_id".to_string(),
                context: Default::default(),
            })?;
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
        let (query, transaction_body) = if item.router_data.request.is_mandate_payment() {
            (
                if item.router_data.request.is_auto_capture() {
                    constants::CHARGE_AND_VAULT_TRANSACTION_MUTATION.to_string()
                } else {
                    constants::AUTHORIZE_AND_VAULT_CREDIT_CARD_MUTATION.to_string()
                },
                TransactionBody::Vault(VaultTransactionBody {
                    amount,
                    merchant_account_id: metadata.merchant_account_id,
                    vault_payment_method_after_transacting: TransactionTiming {
                        when: VaultTiming::Always,
                    },
                    customer_details: item
                        .router_data
                        .resource_common_data
                        .get_billing_email()
                        .ok()
                        .map(|email| CustomerBody { email }),
                    order_id,
                    enrichment,
                }),
            )
        } else {
            (
                if item.router_data.request.is_auto_capture() {
                    constants::CHARGE_CREDIT_CARD_MUTATION.to_string()
                } else {
                    constants::AUTHORIZE_CREDIT_CARD_MUTATION.to_string()
                },
                TransactionBody::Regular(RegularTransactionBody {
                    amount,
                    merchant_account_id: metadata.merchant_account_id,
                    channel: constants::CHANNEL_CODE.to_string(),
                    customer_details: item
                        .router_data
                        .resource_common_data
                        .get_billing_email()
                        .ok()
                        .map(|email| CustomerBody { email }),
                    order_id,
                    enrichment,
                }),
            )
        };
        Ok(Self {
            query,
            variables: VariablePaymentInput {
                input: PaymentInput {
                    payment_method_id: match &item.router_data.request.payment_method_data {
                        PaymentMethodData::PaymentMethodToken(t) => t.token.clone(),
                        _ => {
                            return Err(IntegrationError::MissingRequiredField {
                                field_name: "payment_method_token",
                                context: Default::default(),
                            }
                            .into())
                        }
                    },
                    transaction: transaction_body,
                    options,
                },
            },
        })
    }
}

fn get_braintree_redirect_form<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    client_token_data: ClientTokenResponse,
    payment_method_token: Secret<String>,
    card_details: PaymentMethodData<T>,
    complete_authorize_url: String,
) -> Result<RedirectForm, Report<ConnectorError>> {
    Ok(RedirectForm::Braintree {
        client_token: client_token_data
            .data
            .create_client_token
            .client_token
            .expose(),
        card_token: payment_method_token.expose(),
        bin: match card_details {
            PaymentMethodData::Card(_) => {
                match get_card_isin_from_payment_method_data(&card_details) {
                    Ok(bin) => bin,
                    Err(_) => {
                        return Err(
                            ConnectorError::unexpected_response_error_http_status_unknown().into(),
                        );
                    }
                }
            }
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                return Err(ConnectorError::unexpected_response_error_http_status_unknown().into());
            }
        },
        acs_url: complete_authorize_url,
    })
}

fn validate_currency(
    request_currency: enums::Currency,
    merchant_config_currency: Option<enums::Currency>,
) -> Result<(), IntegrationError> {
    let merchant_config_currency =
        merchant_config_currency.ok_or(IntegrationError::NoConnectorMetaData {
            context: Default::default(),
        })?;
    if request_currency != merchant_config_currency {
        Err(IntegrationError::NotSupported {
            message: format!(
                "currency {request_currency} is not supported for this merchant account",
            ),
            connector: "Braintree",
            context: Default::default(),
        })?
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct BraintreeWebhookResponse {
    pub bt_signature: String,
    pub bt_payload: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Notification {
    pub kind: String, // xml parse only string to fields
    pub timestamp: String,
    pub dispute: Option<BraintreeDisputeData>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BraintreeDisputeData {
    pub amount_disputed: MinorUnit,
    pub amount_won: Option<String>,
    pub case_number: Option<String>,
    pub chargeback_protection_level: Option<String>,
    pub currency_iso_code: enums::Currency,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub created_at: Option<PrimitiveDateTime>,
    pub evidence: Option<DisputeEvidence>,
    pub id: String,
    pub kind: String, // xml parse only string to fields
    pub status: String,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub updated_at: Option<PrimitiveDateTime>,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub reply_by_date: Option<PrimitiveDateTime>,
    pub transaction: DisputeTransaction,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DisputeTransaction {
    pub amount: StringMajorUnit,
    pub id: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct DisputeEvidence {
    pub comment: String,
    pub id: Secret<String>,
    pub created_at: Option<PrimitiveDateTime>,
    pub url: url::Url,
}

// Maps the Braintree notification `kind` to the prism webhook event type.
// Ports HS `get_status` (hyperswitch braintree/transformers.rs `get_status`) 1:1.
pub(super) fn get_status(status: &str) -> connector_types::EventType {
    match status {
        "dispute_opened" => connector_types::EventType::DisputeOpened,
        "dispute_lost" => connector_types::EventType::DisputeLost,
        "dispute_won" => connector_types::EventType::DisputeWon,
        "dispute_accepted" | "dispute_auto_accepted" => connector_types::EventType::DisputeAccepted,
        "dispute_expired" => connector_types::EventType::DisputeExpired,
        "dispute_disputed" => connector_types::EventType::DisputeChallenged,
        _ => connector_types::EventType::IncomingWebhookEventUnspecified,
    }
}

// Maps the Braintree notification `kind` to the prism dispute status.
// Mirrors how the HS router derives `DisputeStatus` from the webhook event.
pub(super) fn get_dispute_status(status: &str) -> enums::DisputeStatus {
    match status {
        "dispute_opened" => enums::DisputeStatus::DisputeOpened,
        "dispute_lost" => enums::DisputeStatus::DisputeLost,
        "dispute_won" => enums::DisputeStatus::DisputeWon,
        "dispute_accepted" | "dispute_auto_accepted" => enums::DisputeStatus::DisputeAccepted,
        "dispute_expired" => enums::DisputeStatus::DisputeExpired,
        "dispute_disputed" => enums::DisputeStatus::DisputeChallenged,
        _ => enums::DisputeStatus::DisputeOpened,
    }
}

// Maps the Braintree dispute `kind` to the prism dispute stage.
// Ports HS `get_dispute_stage` 1:1.
pub(super) fn get_dispute_stage(
    code: &str,
) -> Result<enums::DisputeStage, Report<domain_types::errors::WebhookError>> {
    match code {
        "CHARGEBACK" => Ok(enums::DisputeStage::Dispute),
        "PRE_ARBITRATION" => Ok(enums::DisputeStage::PreArbitration),
        "RETRIEVAL" => Ok(enums::DisputeStage::PreDispute),
        _ => Err(error_stack::report!(
            domain_types::errors::WebhookError::WebhookBodyDecodingFailed
        )),
    }
}

// Decodes the form-urlencoded webhook envelope (`bt_signature` + `bt_payload`).
pub(super) fn get_webhook_object_from_body(
    body: &[u8],
) -> Result<BraintreeWebhookResponse, Report<domain_types::errors::WebhookError>> {
    serde_urlencoded::from_bytes::<BraintreeWebhookResponse>(body)
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable(
            "failed to url-decode the Braintree webhook body (bt_signature/bt_payload)",
        )
}

// Base64-decodes the (newline-stripped) `bt_payload` and parses the XML `Notification`.
pub(super) fn decode_webhook_payload(
    payload: &[u8],
) -> Result<Notification, Report<domain_types::errors::WebhookError>> {
    let decoded_response = super::BASE64_ENGINE
        .decode(payload)
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("failed to base64-decode the Braintree bt_payload")?;

    let xml_response = String::from_utf8(decoded_response)
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("Braintree bt_payload is not valid UTF-8")?;

    xml_response
        .parse_xml::<Notification>()
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("failed to parse the Braintree notification XML")
}

// `bt_signature` is `pubkey1|sig1&pubkey2|sig2&...`; pick the signature whose
// public key matches the merchant's Braintree public key.
pub(super) fn get_matching_webhook_signature(
    signature_pairs: &[(&str, &str)],
    secret: &str,
) -> Option<String> {
    signature_pairs
        .iter()
        .find(|(public_key, _)| *public_key == secret)
        .map(|(_, signature)| signature.to_string())
}

// Full request -> `Notification` decode: urlencoded envelope, then base64 + XML on the
// newline-stripped `bt_payload`. Mirrors the two-step decode the trait methods performed inline.
pub(super) fn decode_from_request(
    request: &connector_types::RequestDetails,
) -> Result<Notification, Report<domain_types::errors::WebhookError>> {
    let notif = get_webhook_object_from_body(&request.body)?;
    decode_webhook_payload(notif.bt_payload.replace('\n', "").as_bytes())
}

// Builds the typed webhook resource reference for a dispute notification.
pub(super) fn get_webhook_reference(
    notification: &Notification,
) -> Result<
    Option<connector_types::WebhookResourceReference>,
    Report<domain_types::errors::WebhookError>,
> {
    match &notification.dispute {
        // HS emits `PaymentId(ConnectorTransactionId(transaction.id))`. The shadow normaliser
        // maps a prism Dispute reference via `connector_dispute_id.or(connector_transaction_id)`,
        // preferring connector_dispute_id, so it MUST be `None` here to match HS byte-for-byte.
        Some(dispute_data) => Ok(Some(connector_types::WebhookResourceReference::Dispute(
            connector_types::DisputeWebhookReference {
                connector_dispute_id: None,
                connector_transaction_id: Some(dispute_data.transaction.id.clone()),
            },
        ))),
        None => Err(error_stack::report!(
            domain_types::errors::WebhookError::WebhookReferenceIdNotFound
        )),
    }
}

// Builds the dispute webhook response, including the webhook amount conversion.
pub(super) fn build_webhook_dispute_response(
    notification: &Notification,
    raw_body: &[u8],
) -> Result<
    connector_types::DisputeWebhookDetailsResponse,
    Report<domain_types::errors::WebhookError>,
> {
    match &notification.dispute {
        Some(dispute_data) => Ok(connector_types::DisputeWebhookDetailsResponse {
            amount: domain_types::utils::convert_amount_for_webhook(
                &common_utils::types::StringMinorUnitForConnector,
                dispute_data.amount_disputed,
                dispute_data.currency_iso_code,
            )?,
            currency: dispute_data.currency_iso_code,
            dispute_id: dispute_data.id.clone(),
            status: get_dispute_status(notification.kind.as_str()),
            stage: get_dispute_stage(dispute_data.kind.as_str())?,
            connector_response_reference_id: None,
            dispute_message: dispute_data.reason.clone(),
            connector_reason_code: dispute_data.reason_code.clone(),
            raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
            status_code: 200,
            response_headers: None,
        }),
        None => Err(error_stack::report!(
            domain_types::errors::WebhookError::WebhookResourceObjectNotFound
        )),
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum BraintreeRepeatPaymentRequest {
    Mandate(MandatePaymentRequest),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BraintreeRepeatPaymentRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let metadata: BraintreeMeta = if let (
            Some(merchant_account_id),
            Some(merchant_config_currency),
        ) = (
            item.router_data.request.merchant_account_id.clone(),
            item.router_data.request.merchant_configured_currency,
        ) {
            info!(
                "BRAINTREE: Picking merchant_account_id and merchant_config_currency from repeatpayments request"
            );

            BraintreeMeta {
                merchant_account_id,
                merchant_config_currency,
            }
        } else {
            let auth = BraintreeAuthType::try_from(&item.router_data.connector_config)?;
            let merchant_account_id =
                auth.merchant_account_id
                    .ok_or(IntegrationError::InvalidConnectorConfig {
                        config: "merchant_account_id",
                        context: Default::default(),
                    })?;
            let merchant_config_currency = auth
                .merchant_config_currency
                .as_deref()
                .and_then(|s| s.parse::<enums::Currency>().ok())
                .ok_or(IntegrationError::InvalidConnectorConfig {
                    config: "merchant_config_currency",
                    context: Default::default(),
                })?;
            BraintreeMeta {
                merchant_account_id,
                merchant_config_currency,
            }
        };
        validate_currency(
            item.router_data.request.currency,
            Some(metadata.merchant_config_currency),
        )?;
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::MandatePayment => {
                let connector_mandate_id = item.router_data.request.connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    },
                )?;
                Ok(Self::Mandate(MandatePaymentRequest::try_from((
                    item,
                    connector_mandate_id,
                    metadata,
                ))?))
            }
            PaymentMethodData::Card(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("braintree"),
                    connector: "Braintree",
                    context: Default::default(),
                }))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardTransactionOptions {
    /// `CreditCardTransactionOptionsInput.billingAddress` — the billing address is a
    /// SIBLING of `input.transaction`, **not** a child of it: `TransactionInput` has no
    /// `billingAddress` field at all. Braintree merges this with any address captured at
    /// tokenisation, giving priority to the value sent here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<BraintreeAddressInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_authentication: Option<ThreeDSecureAuthenticationInput>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureAuthenticationInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass_through: Option<ThreeDSecurePassThroughInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecurePassThroughInput {
    /// `eciFlag: ECommerceIndicator!` is the ONLY non-null member of
    /// `ThreeDSecurePassThroughInput`, so it must never be serialized as `null` —
    /// GraphQL input coercion rejects the whole mutation. `ECommerceIndicator` is an
    /// unconstrained scalar (a two-character, network-scoped string such as Visa `05` /
    /// Mastercard `02`), not an enum, so the raw MPI value is passed through unmapped.
    pub eci_flag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_server_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory_server_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory_server_transaction_id: Option<String>,
}

/// Builds `options.threeDSecureAuthentication.passThrough` from an external-MPI
/// authentication result.
///
/// Fails closed when the caller supplied `authentication_data` without an ECI: Braintree
/// declares `eciFlag` non-null, so the alternative would be a mutation that is rejected
/// wholesale with an opaque validation error. A caller that sends no
/// `authentication_data` at all is unaffected — the pass-through object is simply absent.
fn convert_external_three_ds_data(
    auth_data: &router_request_types::AuthenticationData,
) -> Result<ThreeDSecurePassThroughInput, Report<IntegrationError>> {
    Ok(ThreeDSecurePassThroughInput {
        eci_flag: auth_data.eci.clone().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "authentication_data.eci",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the ECI returned by the 3DS MPI alongside the CAVV. Braintree \
                         declares options.threeDSecureAuthentication.passThrough.eciFlag as \
                         non-null, so an external-3DS authorization cannot omit it."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Input--ThreeDSecurePassThroughInput"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "external 3DS pass-through requested but authentication_data.eci is empty"
                            .to_string(),
                    ),
                },
            }
        })?,
        cavv: auth_data.cavv.clone(),
        three_d_secure_server_transaction_id: auth_data.threeds_server_transaction_id.clone(),
        version: auth_data
            .message_version
            .as_ref()
            .map(|semantic_version| semantic_version.to_string()),
        directory_server_response: auth_data
            .trans_status
            .as_ref()
            .map(map_transaction_status_to_code),
        directory_server_transaction_id: auth_data.ds_trans_id.clone(),
    })
}

fn map_transaction_status_to_code(status: &common_enums::TransactionStatus) -> String {
    match status {
        common_enums::TransactionStatus::Success => "Y".to_string(),
        common_enums::TransactionStatus::Failure => "N".to_string(),
        common_enums::TransactionStatus::VerificationNotPerformed => "U".to_string(),
        common_enums::TransactionStatus::NotVerified => "A".to_string(),
        common_enums::TransactionStatus::Rejected => "R".to_string(),
        common_enums::TransactionStatus::ChallengeRequired => "C".to_string(),
        common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication => {
            "D".to_string()
        }
        common_enums::TransactionStatus::InformationOnly => "I".to_string(),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeRepeatPaymentResponse {
    PaymentsResponse(Box<PaymentsResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreeRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreeRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeRepeatPaymentResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors.clone(), item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreeRepeatPaymentResponse::PaymentsResponse(payment_response) => {
                let transaction_data = payment_response.data.charge_credit_card.transaction;
                let status = enums::AttemptStatus::from(transaction_data.status.clone());
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_failure_error_response(
                        transaction_data.status,
                        Some(transaction_data.id),
                        item.http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_data.id),
                        redirection_data: None,
                        mandate_reference: transaction_data.payment_method.as_ref().map(|pm| {
                            Box::new(MandateReference {
                                connector_mandate_id: Some(pm.id.clone().expose()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                mandate_metadata: None,
                            })
                        }),
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    })
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
    }
}

// VoidPostCapture (Reverse) flow — Braintree uses the same reverseTransaction mutation
// for both pre-capture voids and post-capture reversals.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoidPCInputData {
    transaction_id: String,
}

#[derive(Debug, Serialize)]
pub struct VoidPCVariables {
    input: VoidPCInputData,
}

#[derive(Debug, Serialize)]
pub struct BraintreeVoidPCRequest {
    query: String,
    variables: VoidPCVariables,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BraintreeVoidPCRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let query = constants::VOID_TRANSACTION_MUTATION.to_string();
        let variables = VoidPCVariables {
            input: VoidPCInputData {
                transaction_id: item.router_data.request.connector_transaction_id.clone(),
            },
        };
        Ok(Self { query, variables })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoidPCResponseTransactionBody {
    id: String,
    status: BraintreePaymentStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoidPCTransactionData {
    reversal: VoidPCResponseTransactionBody,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoidPCResponseData {
    reverse_transaction: VoidPCTransactionData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoidPCResponse {
    data: VoidPCResponseData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeVoidPCResponse {
    VoidPCResponse(Box<VoidPCResponse>),
    ErrorResponse(Box<ErrorResponse>),
}

impl TryFrom<ResponseRouterData<BraintreeVoidPCResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BraintreeVoidPCResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeVoidPCResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreeVoidPCResponse::VoidPCResponse(void_pc_response) => {
                let reversal_data = void_pc_response.data.reverse_transaction.reversal;
                let post_capture_void_status = match reversal_data.status {
                    BraintreePaymentStatus::Voided => {
                        common_enums::PostCaptureVoidStatus::Succeeded
                    }
                    BraintreePaymentStatus::Failed
                    | BraintreePaymentStatus::GatewayRejected
                    | BraintreePaymentStatus::ProcessorDeclined
                    | BraintreePaymentStatus::SettlementDeclined
                    | BraintreePaymentStatus::AuthorizedExpired => {
                        common_enums::PostCaptureVoidStatus::Failed
                    }
                    BraintreePaymentStatus::Authorized
                    | BraintreePaymentStatus::Authorizing
                    | BraintreePaymentStatus::Settling
                    | BraintreePaymentStatus::Settled
                    | BraintreePaymentStatus::SettlementPending
                    | BraintreePaymentStatus::SettlementConfirmed
                    | BraintreePaymentStatus::SubmittedForSettlement => {
                        common_enums::PostCaptureVoidStatus::Pending
                    }
                };
                let response = if post_capture_void_status.is_post_capture_void_failure() {
                    Err(create_failure_error_response(
                        reversal_data.status,
                        None,
                        item.http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::PostCaptureVoidResponse {
                        post_capture_void_status,
                        connector_reference_id: Some(reversal_data.id),
                        description: None,
                        status_code: item.http_code,
                    })
                };
                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

// =============================================================================
// SETUP MANDATE (Pay.SetupRecurring) FLOW - REQUEST / RESPONSE TRANSFORMERS
// =============================================================================
// Braintree does not expose a pure "store-only" mutation that accepts raw card
// data and produces a multi-use payment method token in a single shot. The
// closest stable path that returns a `paymentMethod.id` we can hand back as a
// `connector_mandate_id` is `tokenizeCreditCard` — the same mutation the
// PaymentMethodToken flow already uses. SetupMandate therefore tokenizes the
// card and surfaces the resulting Braintree payment method id as the
// `connector_mandate_id` on a `MandateReference`, which RepeatPayment then
// consumes via the existing `MandatePayment` request path.

pub type BraintreeSetupMandateRequest<T> = GenericBraintreeRequest<VariableInput<T>>;
pub type BraintreeSetupMandateResponse = BraintreeTokenResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BraintreeSetupMandateRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(card_data) => Ok(Self {
                query: constants::TOKENIZE_CREDIT_CARD.to_string(),
                variables: VariableInput {
                    input: InputData {
                        credit_card: CreditCardData {
                            number: card_data.card_number,
                            expiration_year: card_data.card_exp_year,
                            expiration_month: card_data.card_exp_month,
                            cvv: card_data.card_cvc,
                            cardholder_name: item
                                .router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                                .unwrap_or(Secret::new("".to_string())),
                        },
                    },
                },
            }),
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: utils::get_unimplemented_payment_method_error_message("braintree"),
                    connector: "Braintree",
                    context: Default::default(),
                }))
            }
        }
    }
}

// Response transformer: a successful `tokenizeCreditCard` response carries a
// `paymentMethod.id` which we mirror on both `resource_id` (so PSync /
// downstream lookups have something to anchor on) and `mandate_reference`
// (so the orchestrator can persist it and replay it on RepeatPayment).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreeSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreeSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeSetupMandateResponse::ErrorResponse(error_response) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: build_error_response(error_response.errors.as_ref(), item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreeSetupMandateResponse::TokenResponse(token_response) => {
                let payment_method_id = token_response
                    .data
                    .tokenize_credit_card
                    .payment_method
                    .id
                    .expose();
                let mandate_reference = Some(Box::new(MandateReference {
                    connector_mandate_id: Some(payment_method_id.clone()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                }));
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: enums::AttemptStatus::Charged,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(payment_method_id),
                        redirection_data: None,
                        mandate_reference,
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
    }
}
