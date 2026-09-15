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
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
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
    /// Braintree-hosted 3DS, leg 1 (`PreAuthenticate`) — a SINGLE HTTP request carrying TWO root
    /// mutation fields.
    ///
    /// `tokenizeCreditCard` and `createClientToken` take disjoint inputs, so neither needs the
    /// other's output and GraphQL executes root mutation fields serially in document order. That
    /// lets one call produce both halves the 3DS chain needs: the single-use `paymentMethod.id`
    /// that leg 2's `performThreeDSecureLookup` spends as its mandatory `paymentMethodId`, and the
    /// client token a caller passes to `braintree.threeDSecure.create` when it wants to collect
    /// device data in the browser.
    ///
    /// Splicing the two into one document is what makes the trio reachable from
    /// `CompositePaymentService/Authorize` at all: that loop does NOT run the `PaymentMethodToken`
    /// flow (`process_composite_authorize` in `composite-service/src/payments.rs` has no
    /// tokenization step), so every authentication leg receives the raw card. Without a nonce
    /// minted here, leg 2 has nothing to look up.
    ///
    /// Multi-root acceptance was verified live against the Braintree sandbox at
    /// `Braintree-Version: 2019-01-01` before this constant was written — §P.6.1 of the technical
    /// specification listed it as "legal per the GraphQL spec, undocumented by Braintree" and
    /// required a sandbox gate; the gate passed and returned both fields in one payload.
    pub const PRE_AUTHENTICATE_MUTATION: &str = "mutation braintreeThreeDSecureSetup($card: TokenizeCreditCardInput!, $clientToken: CreateClientTokenInput!) { tokenizeCreditCard(input: $card) { paymentMethod { id } } createClientToken(input: $clientToken) { clientToken } }";

    /// Selection set shared by leg 2 (`performThreeDSecureLookup`) and leg 3 (`node(id:)`).
    ///
    /// `CreditCardDetails.threeDSecure` MUST be traversed through `.authentication`. At the pinned
    /// version the served schema already types it as `ThreeDSecureDetails` (the 2020-10-07 retype),
    /// whose flat scalars are every one `@deprecated`; selecting them directly is a hard GraphQL
    /// validation error. `Braintree-Version` gates the interpretation of values, not the shape of
    /// the schema.
    ///
    /// `details` is the `PaymentMethodDetails` UNION, so the inline fragment is mandatory — and it
    /// yields `{}` rather than an error for a non-card member, which is why the response structs
    /// make every level optional and detect an absent `authentication` block explicitly.
    macro_rules! three_d_secure_payment_method_fields {
        () => {
            "id details { ... on CreditCardDetails { bin last4 brandCode threeDSecure { \
             authentication { cavv eciFlag liabilityShifted liabilityShiftPossible cardEnrolled \
             authenticationStatus version directoryServerTransactionId xId \
             threeDSecureServerTransactionId acsTransactionId paresStatus transactionStatus \
             transactionStatusReason } } } }"
        };
    }

    /// Braintree-hosted 3DS, leg 2 (`Authenticate`): the server-side 3DS lookup.
    ///
    /// `ThreeDSecureLookupData` has exactly seven members and carries NONE of
    /// `authenticationStatus` / `cavv` / `eciFlag` / `liabilityShifted` — those live on
    /// `ThreeDSecureAuthentication`, reached through
    /// `paymentMethod.details -> CreditCardDetails.threeDSecure.authentication`. That split is why
    /// both blocks are selected and why `threeDSecureLookupData.is_some()` is NOT a usable
    /// challenge discriminator (see `BraintreeThreeDSecureLookupPayload::is_challenge`).
    ///
    /// `clientMutationId` is deliberately not selected — the request does not send one.
    pub const AUTHENTICATE_MUTATION: &str = concat!(
        "mutation braintreeThreeDSecureLookup($input: PerformThreeDSecureLookupInput!) { \
         performThreeDSecureLookup(input: $input) { threeDSecureLookupData { acsUrl \
         authenticationId version pareq md termUrl transactionId } paymentMethod { ",
        three_d_secure_payment_method_fields!(),
        " } } }"
    );

    /// Braintree-hosted 3DS, leg 3 (`PostAuthenticate`): read the settled authentication back off
    /// the payment method leg 2 returned.
    ///
    /// This is a QUERY, not a mutation, and that is not a shortcut. The root `Mutation` type has
    /// 112 fields and exactly one matches /3d|3ds|threeDSecure|challenge|authenticat/i —
    /// `performThreeDSecureLookup`, which is leg 2. There is no post-challenge completion mutation
    /// and the root `Query` type has no 3DS field either. The ACS posts its PaRes to Braintree's
    /// own `termUrl`, Braintree records the outcome ON the payment method, and `PaymentMethod`
    /// implements `Node` — so `node(id:)` is the only retrieval path that exists.
    ///
    /// Pass the id VERBATIM: base64 and Relay-style `"PaymentMethod:<id>"` global ids both return
    /// `NOT_FOUND`.
    ///
    /// Do NOT add `createdAt`: on a single-use payment method it returns a partial error
    /// (`NOT_IMPLEMENTED`, "Fetching `createdAt` on a single-use payment method is not supported
    /// from this operation.") alongside a perfectly good `data`, which — with the error variant
    /// first in the untagged response enum — would drag a successful readback into the error arm.
    /// Do NOT add `authenticationInsight`: it requires an `input` argument and selecting it bare is
    /// a hard validation error.
    pub const POST_AUTHENTICATE_QUERY: &str = concat!(
        "query braintreeThreeDSecureResult($id: ID!) { node(id: $id) { ... on PaymentMethod { ",
        three_d_secure_payment_method_fields!(),
        " } } }"
    );

    /// The `connector_feature_data` key that marks a Braintree-HOSTED 3DS authentication.
    ///
    /// Written by legs 1, 2 and 3; read by the Authorize builder to decide two things at once:
    /// which `paymentMethodId` to charge, and that the authentication already lives ON the payment
    /// method at Braintree and must therefore NOT be re-declared as external-MPI pass-through.
    pub const BRAINTREE_THREE_DS_FEATURE_KEY: &str = "braintree_three_ds";

    /// Key inside the [`BRAINTREE_THREE_DS_FEATURE_KEY`] object holding the nonce that the next
    /// leg — ultimately Authorize — must spend.
    pub const BRAINTREE_THREE_DS_PAYMENT_METHOD_ID_KEY: &str = "payment_method_id";

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
        // Set by every leg of the Braintree-HOSTED 3D Secure trio and by nothing else. Its
        // presence means the authentication already ran, lives ON the payment method at Braintree,
        // and is applied automatically at charge time — so this Authorize must charge the verified
        // nonce and declare nothing, rather than opening a fresh client-token journey or
        // re-declaring the result as an external-MPI pass-through.
        let is_braintree_hosted_three_ds = braintree_hosted_three_ds_nonce(
            item.router_data.request.connector_feature_data.as_ref(),
        )
        .is_some();

        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(_) => {
                if item.router_data.resource_common_data.is_three_ds()
                    && item.router_data.request.authentication_data.is_none()
                    && !is_braintree_hosted_three_ds
                {
                    // A raw-card `PaymentService/Authorize` that asked for 3D Secure but never ran
                    // the authentication trio. Braintree-hosted 3DS cannot happen inside a single
                    // Authorize call, so this hands the caller the client-SDK bootstrap rather
                    // than charging an unauthenticated card. Routing the payment through
                    // `CompositePaymentService/Authorize` runs the trio instead and lands in the
                    // branch below with the verified nonce already in `connector_feature_data`.
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
        // The Braintree-HOSTED 3D Secure trio plants this nonce in `connector_feature_data`, and
        // the composite dispatcher forwards it here. It is simultaneously:
        //   * the payment method this charge must spend — the lookup consumed the original nonce
        //     and returned a new one, and a replay of the old one is rejected outright; and
        //   * the discriminator that says the authentication was performed BY Braintree.
        let braintree_hosted_three_ds_payment_method = braintree_hosted_three_ds_nonce(
            item.router_data.request.connector_feature_data.as_ref(),
        );

        // External-MPI 3D Secure: the caller performed the authentication with its own MPI and
        // declares the result here as `options.threeDSecureAuthentication.passThrough`.
        //
        // The `is_none()` guard is what keeps the two topologies apart. The composite dispatcher
        // copies PostAuthenticate's `authentication_data` onto this request, so without it a
        // BRAINTREE-performed authentication would be re-declared as an externally performed one —
        // at best redundant, at worst a compliance misstatement. It is also unnecessary: a
        // 3DS-verified nonce charged with no pass-through comes back liability-shifted, because
        // Braintree attaches the authentication itself.
        let three_ds_data = braintree_hosted_three_ds_payment_method
            .is_none()
            .then_some(item.router_data.request.authentication_data.as_ref())
            .flatten()
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
                    // The 3DS-verified nonce wins when the hosted trio ran: on that path
                    // `payment_method_data` still holds the ORIGINAL instrument, whose nonce the
                    // lookup consumed.
                    payment_method_id: match (
                        braintree_hosted_three_ds_payment_method,
                        &item.router_data.request.payment_method_data,
                    ) {
                        (Some(verified_nonce), _) => verified_nonce,
                        (None, PaymentMethodData::PaymentMethodToken(t)) => t.token.clone(),
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

// =============================================================================================
// Braintree-hosted 3D Secure — the PreAuthenticate / Authenticate / PostAuthenticate trio.
//
// Mutually exclusive with the EXTERNAL / merchant-performed (MPI) 3DS pass-through that the
// Authorize builder already implements through `options.threeDSecureAuthentication.passThrough`.
// The discriminator is stated once, here, and enforced in three places:
//
//   * `authentication_data` present on the original Authorize request  => external MPI; the
//     caller performed 3DS itself and Authorize declares the result with `passThrough`.
//   * `authentication_data` absent + `is_three_ds()`                   => Braintree-hosted; this
//     trio performs the authentication and Braintree attaches the result to the payment method
//     itself, so Authorize declares NOTHING.
//
// The two must never both populate the request: re-declaring a Braintree-performed
// authentication through `passThrough` would assert an *externally* performed authentication
// that never happened. `braintree_hosted_three_ds_nonce` is the guard that keeps them apart.
// =============================================================================================

/// Braintree GraphQL `ThreeDSecureAuthenticationStatus`.
///
/// All 29 values served by the live sandbox schema at `Braintree-Version: 2019-01-01` are
/// modelled, including the four the SDL marks `@deprecated` — deprecated is not removed, live
/// introspection returns them, and they are reachable rather than dead arms.
///
/// `Unknown` exists because Braintree has added statuses repeatedly (four on 2022-09-30, four
/// more on 2023-05-23). Deserializing an unrecognised one into a catch-all keeps a single new
/// status from failing the whole response parse.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreeThreeDSecureAuthenticationStatus {
    AuthenticateSuccessful,
    AuthenticateAttemptSuccessful,
    AuthenticateSuccessfulIssuerNotParticipating,
    AuthenticateFrictionlessFailed,
    AuthenticateFailed,
    AuthenticateFailedAcsError,
    AuthenticateRejected,
    AuthenticateError,
    AuthenticateUnableToAuthenticate,
    AuthenticateSignatureVerificationFailed,
    AuthenticationBypassed,
    AuthenticationUnavailable,
    ChallengeRequired,
    DataOnlySuccessful,
    ExemptionLowValueSuccessful,
    ExemptionTraSuccessful,
    LookupBypassed,
    LookupCardError,
    LookupEnrolled,
    LookupError,
    LookupFailedAcsError,
    LookupNotEnrolled,
    LookupServerError,
    MpiServerError,
    SkippedDueToAdaptiveAuthentication,
    SkippedDueToRule,
    UnsupportedAccountType,
    UnsupportedCard,
    UnsupportedThreeDSecureVersion,
    #[serde(other)]
    Unknown,
}

impl BraintreeThreeDSecureAuthenticationStatus {
    /// Maps Braintree's authentication verdict onto `AttemptStatus`.
    ///
    /// The enum is partitioned into four classes and the partition is the whole point:
    ///
    /// 1. **Authenticated / no authentication needed** -> `AuthenticationSuccessful`. Includes the
    ///    bypass, exemption and not-enrolled families: in each the authentication step is complete
    ///    and the charge may proceed. `DATA_ONLY_SUCCESSFUL` is here too, but it carries NO
    ///    liability shift — read `liabilityShifted` off the payload, never infer it from a status.
    /// 2. **Issuer said no** -> `AuthenticationFailed`, terminal. Only an explicit issuer verdict
    ///    qualifies.
    /// 3. **Ambiguous / transport / directory-server failure** -> `Unspecified`, NON-terminal.
    ///    A lookup that timed out, an ACS that 500ed or an MPI that was unreachable says nothing
    ///    about the cardholder. Mapping these to `AuthenticationFailed` would report a terminal
    ///    authentication decline for what is an infrastructure blip, and mapping them to a success
    ///    would be worse. `Unspecified` leaves the caller's own previous status standing.
    /// 4. **The instrument cannot be 3D-Secured at all** -> `AuthenticationFailed`, terminal.
    ///    `UNSUPPORTED_*` is deterministic and re-running it changes nothing, so unlike class 3 it
    ///    is not ambiguous — but it is also not an issuer decline, hence the separate comment.
    fn to_attempt_status(self) -> enums::AttemptStatus {
        match self {
            // 1 — authenticated, or authentication legitimately not required.
            Self::AuthenticateSuccessful
            | Self::AuthenticateAttemptSuccessful
            | Self::AuthenticateSuccessfulIssuerNotParticipating
            | Self::AuthenticationBypassed
            | Self::DataOnlySuccessful
            | Self::ExemptionLowValueSuccessful
            | Self::ExemptionTraSuccessful
            | Self::LookupBypassed
            | Self::LookupNotEnrolled
            | Self::SkippedDueToAdaptiveAuthentication
            | Self::SkippedDueToRule => enums::AttemptStatus::AuthenticationSuccessful,

            // 2 — explicit issuer decline. Terminal.
            Self::AuthenticateFailed
            | Self::AuthenticateFrictionlessFailed
            | Self::AuthenticateRejected
            | Self::AuthenticateSignatureVerificationFailed => {
                enums::AttemptStatus::AuthenticationFailed
            }

            // 3 — challenge outstanding. Non-terminal by construction: a terminal status here
            // would make the challenge impossible to complete.
            Self::ChallengeRequired | Self::LookupEnrolled => {
                enums::AttemptStatus::AuthenticationPending
            }

            // 4 — ambiguous / transport. Deliberately NOT terminal and deliberately not a success.
            Self::AuthenticateError
            | Self::AuthenticateFailedAcsError
            | Self::AuthenticateUnableToAuthenticate
            | Self::AuthenticationUnavailable
            | Self::LookupCardError
            | Self::LookupError
            | Self::LookupFailedAcsError
            | Self::LookupServerError
            | Self::MpiServerError => enums::AttemptStatus::Unspecified,

            // 5 — 3DS is not available for this instrument at all. Deterministic, not an
            // infrastructure blip, so it does not belong in class 4.
            Self::UnsupportedAccountType
            | Self::UnsupportedCard
            | Self::UnsupportedThreeDSecureVersion => enums::AttemptStatus::AuthenticationFailed,

            // A status Braintree added after this code was written. Never guess.
            Self::Unknown => enums::AttemptStatus::Unspecified,
        }
    }
}

/// Braintree GraphQL `ThreeDSecureAuthenticationStatusIndicator` — the EMV 3DS `transStatus` /
/// `paresStatus` letter, spelled out. An exact 8-to-8 mapping onto `common_enums::TransactionStatus`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreeThreeDSecureStatusIndicator {
    SuccessfulAuthentication,
    FailedAuthentication,
    UnableToCompleteAuthentication,
    SuccessfulAttemptsTransaction,
    AuthenticationRejected,
    ChallengeRequiredForAuthentication,
    ChallengeRequiredDecoupledAuthentication,
    InformationalChallengePreferenceAcknowledged,
    #[serde(other)]
    Unknown,
}

impl BraintreeThreeDSecureStatusIndicator {
    /// `Unknown` maps to `None`, never to `TransactionStatus::default()` — that default is
    /// `Failure`, so an `unwrap_or_default()` anywhere on this path would silently report a failed
    /// authentication for a value Braintree merely added after this code was written.
    fn to_transaction_status(self) -> Option<common_enums::TransactionStatus> {
        match self {
            Self::SuccessfulAuthentication => Some(common_enums::TransactionStatus::Success),
            Self::FailedAuthentication => Some(common_enums::TransactionStatus::Failure),
            Self::UnableToCompleteAuthentication => {
                Some(common_enums::TransactionStatus::VerificationNotPerformed)
            }
            Self::SuccessfulAttemptsTransaction => {
                Some(common_enums::TransactionStatus::NotVerified)
            }
            Self::AuthenticationRejected => Some(common_enums::TransactionStatus::Rejected),
            Self::ChallengeRequiredForAuthentication => {
                Some(common_enums::TransactionStatus::ChallengeRequired)
            }
            Self::ChallengeRequiredDecoupledAuthentication => {
                Some(common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication)
            }
            Self::InformationalChallengePreferenceAcknowledged => {
                Some(common_enums::TransactionStatus::InformationOnly)
            }
            Self::Unknown => None,
        }
    }
}

/// Braintree GraphQL `ThreeDSecureCardEnrolled`. Informational only — it is surfaced verbatim in
/// `connector_feature_data` and no UCS status is ever derived from it.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreeThreeDSecureCardEnrolled {
    Bypass,
    Error,
    No,
    Unavailable,
    Yes,
    #[serde(other)]
    Unknown,
}

/// Braintree GraphQL `ThreeDSecureDeviceChannel`. Serialized only.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreeThreeDSecureDeviceChannel {
    Browser,
    Sdk,
}

/// Braintree GraphQL `ThreeDSecureAuthentication` — all 14 members, the authoritative home of
/// every 3DS result field.
///
/// Note what is NOT here: `ThreeDSecureLookupData` carries none of `authenticationStatus`, `cavv`,
/// `eciFlag` or `liabilityShifted`, which is why the lookup mutation has to select both blocks.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeThreeDSecureAuthentication {
    /// The cardholder authentication cryptogram — liability-shift proof, and secret for its whole
    /// life.
    pub cavv: Option<Secret<String>>,
    pub directory_server_transaction_id: Option<String>,
    /// `ECommerceIndicator`, an unconstrained network-scoped two-character scalar (Visa 05/06/07,
    /// Mastercard 02/01/00). Passed through unmapped precisely so it is not reinterpreted.
    pub eci_flag: Option<String>,
    pub liability_shifted: Option<bool>,
    pub liability_shift_possible: Option<bool>,
    pub card_enrolled: Option<BraintreeThreeDSecureCardEnrolled>,
    pub authentication_status: Option<BraintreeThreeDSecureAuthenticationStatus>,
    pub version: Option<String>,
    /// The legacy CardinalCommerce XID. Capital `I` — `xid` does not exist in the schema.
    pub x_id: Option<String>,
    pub three_d_secure_server_transaction_id: Option<String>,
    pub acs_transaction_id: Option<String>,
    /// 3DS 1.0 `paresStatus`. Recorded but not mapped: it was null on the challenge capture and
    /// duplicates `transactionStatus` on the successes.
    pub pares_status: Option<BraintreeThreeDSecureStatusIndicator>,
    /// 3DS 2.x `transStatus` — the field `AuthenticationData.trans_status` is built from.
    pub transaction_status: Option<BraintreeThreeDSecureStatusIndicator>,
    pub transaction_status_reason: Option<String>,
}

/// Braintree GraphQL `ThreeDSecureDetails`. Its flat scalars are all `@deprecated`, so only
/// `.authentication` is ever selected.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeThreeDSecureDetails {
    pub authentication: Option<BraintreeThreeDSecureAuthentication>,
}

/// The `... on CreditCardDetails` inline fragment of the `PaymentMethodDetails` union.
///
/// Every member is optional because the fragment yields `{}` — not an error — when the payment
/// method is not a credit card. An absent `three_d_secure` is therefore detected explicitly rather
/// than inferred from a deserialization failure.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeCreditCardDetails {
    pub bin: Option<String>,
    pub last4: Option<String>,
    pub brand_code: Option<String>,
    pub three_d_secure: Option<BraintreeThreeDSecureDetails>,
}

/// The `paymentMethod` node shared by leg 2's mutation payload and leg 3's `node(id:)` query.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeThreeDSecurePaymentMethod {
    /// A spendable payment credential, hence `Secret`.
    pub id: Secret<String>,
    pub details: Option<BraintreeCreditCardDetails>,
}

impl BraintreeThreeDSecurePaymentMethod {
    fn authentication(&self) -> Option<&BraintreeThreeDSecureAuthentication> {
        self.details
            .as_ref()
            .and_then(|details| details.three_d_secure.as_ref())
            .and_then(|three_ds| three_ds.authentication.as_ref())
    }

    fn card_details(&self) -> Option<&BraintreeCreditCardDetails> {
        self.details.as_ref()
    }
}

/// Braintree GraphQL `ThreeDSecureLookupData` — exactly seven members, every one nullable.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeThreeDSecureLookupData {
    pub acs_url: Option<String>,
    pub authentication_id: Option<String>,
    pub version: Option<String>,
    /// Named `pareq` for 3DS 1 legacy reasons but base64-decodes to a 3DS 2 `CReq`. Passed through
    /// opaquely: never decoded, re-encoded or re-derived.
    pub pareq: Option<Secret<String>>,
    /// Always exactly equal to `authentication_id` in every observed response. Echoed rather than
    /// synthesised from it, so the form stays correct if Braintree ever diverges them.
    pub md: Option<String>,
    /// A Braintree-hosted callback that embeds a signed `authorization_fingerprint` JWT, so the
    /// whole URL is a credential.
    pub term_url: Option<Secret<String>>,
    pub transaction_id: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------------------------

/// Metadata bag shape used only to look for a per-request `merchant_account_id`.
///
/// Deliberately narrower than [`BraintreeMeta`], which also requires `merchant_config_currency`:
/// the authentication legs move no money, so demanding a currency from them would reject a
/// perfectly valid metadata bag.
#[derive(Debug, Deserialize)]
struct BraintreeMerchantAccountMeta {
    merchant_account_id: Option<Secret<String>>,
}

/// Resolves `merchantAccountId` for a flow whose request carries only the generic `metadata` bag
/// rather than a dedicated `merchant_account_id` member.
///
/// Order is per-request first, connector config second — the same precedence the Authorize builder
/// applies. Fails closed: a Braintree merchant account is not something to guess or default.
///
/// `PostAuthenticate` passes `None`: `node(id: ID!)` takes exactly one argument and carries no
/// merchant scope at all — the Basic-auth credential pair IS the merchant scope — so that leg never
/// calls this helper.
fn resolve_merchant_account_id(
    metadata: Option<&pii::SecretSerdeValue>,
    connector_config: &ConnectorSpecificConfig,
) -> Result<Secret<String>, Report<IntegrationError>> {
    let from_metadata = metadata.and_then(|meta| {
        serde_json::from_value::<BraintreeMerchantAccountMeta>(meta.clone().expose())
            .ok()
            .and_then(|parsed| parsed.merchant_account_id)
    });

    match from_metadata {
        Some(merchant_account_id) => Ok(merchant_account_id),
        None => BraintreeAuthType::try_from(connector_config)?
            .merchant_account_id
            .ok_or_else(|| {
                IntegrationError::InvalidConnectorConfig {
                    config: "merchant_account_id",
                    context: domain_types::errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Set `merchant_account_id` in the Braintree connector configuration, \
                             or send it in the request `metadata` bag. Braintree scopes every 3D \
                             Secure lookup to a merchant account and will not infer one."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://graphql.braintreepayments.com/reference/#Input--PerformThreeDSecureLookupInput"
                                .to_string(),
                        ),
                        additional_context: Some(
                            "Braintree-hosted 3DS leg requested but no merchant_account_id is \
                             resolvable from the request metadata or the connector config"
                                .to_string(),
                        ),
                    },
                }
                .into()
            }),
    }
}

/// Reads the Braintree-hosted 3DS nonce out of a `connector_feature_data` blob.
///
/// `Some(_)` is the single discriminator that says "the trio ran": it is written by every leg of
/// the trio and by nothing else. The Authorize builder uses it both to pick the payment method it
/// charges and to suppress the external-MPI pass-through.
fn braintree_hosted_three_ds_nonce(
    feature_data: Option<&pii::SecretSerdeValue>,
) -> Option<Secret<String>> {
    feature_data
        .and_then(|value| {
            value
                .peek()
                .get(constants::BRAINTREE_THREE_DS_FEATURE_KEY)
                .and_then(|blob| blob.get(constants::BRAINTREE_THREE_DS_PAYMENT_METHOD_ID_KEY))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .map(Secret::new)
}

/// Builds the `braintree_three_ds` blob that carries every Braintree-specific 3DS signal with no
/// `AuthenticationData` slot from one leg to the next, and finally into Authorize.
///
/// `liability_shifted` / `liability_shift_possible` are emitted verbatim because they must be READ,
/// never inferred from the status: `DATA_ONLY_SUCCESSFUL` is a success with no liability shift,
/// and two different failures differ only in whether a retry could ever shift liability.
fn build_braintree_three_ds_feature_data(
    payment_method_id: &Secret<String>,
    client_token: Option<&Secret<String>>,
    authentication: Option<&BraintreeThreeDSecureAuthentication>,
    card: Option<&BraintreeCreditCardDetails>,
    lookup: Option<&BraintreeThreeDSecureLookupData>,
) -> pii::SecretSerdeValue {
    let mut blob = serde_json::Map::new();
    blob.insert(
        constants::BRAINTREE_THREE_DS_PAYMENT_METHOD_ID_KEY.to_string(),
        serde_json::Value::String(payment_method_id.peek().clone()),
    );
    if let Some(token) = client_token {
        // The client token is a credential; the whole blob is carried as a `Secret` and reaches
        // the caller as a masked string, which is why it is safe to place here and nowhere else.
        blob.insert(
            "client_token".to_string(),
            serde_json::Value::String(token.peek().clone()),
        );
    }
    if let Some(lookup) = lookup {
        if let Some(authentication_id) = lookup.authentication_id.as_ref() {
            blob.insert(
                "authentication_id".to_string(),
                serde_json::Value::String(authentication_id.clone()),
            );
        }
        if let Some(transaction_id) = lookup.transaction_id.as_ref() {
            blob.insert(
                "lookup_transaction_id".to_string(),
                serde_json::Value::String(transaction_id.clone()),
            );
        }
    }
    if let Some(authentication) = authentication {
        if let Some(liability_shifted) = authentication.liability_shifted {
            blob.insert(
                "liability_shifted".to_string(),
                serde_json::Value::Bool(liability_shifted),
            );
        }
        if let Some(liability_shift_possible) = authentication.liability_shift_possible {
            blob.insert(
                "liability_shift_possible".to_string(),
                serde_json::Value::Bool(liability_shift_possible),
            );
        }
        if let Some(card_enrolled) = authentication.card_enrolled {
            blob.insert(
                "card_enrolled".to_string(),
                serde_json::Value::String(card_enrolled.to_string()),
            );
        }
        if let Some(authentication_status) = authentication.authentication_status {
            blob.insert(
                "authentication_status".to_string(),
                serde_json::Value::String(authentication_status.to_string()),
            );
        }
        if let Some(pares_status) = authentication.pares_status {
            blob.insert(
                "pares_status".to_string(),
                serde_json::Value::String(pares_status.to_string()),
            );
        }
        if let Some(reason) = authentication.transaction_status_reason.as_ref() {
            blob.insert(
                "transaction_status_reason".to_string(),
                serde_json::Value::String(reason.clone()),
            );
        }
        if let Some(x_id) = authentication.x_id.as_ref() {
            blob.insert("x_id".to_string(), serde_json::Value::String(x_id.clone()));
        }
    }
    if let Some(card) = card {
        if let Some(bin) = card.bin.as_ref() {
            blob.insert("bin".to_string(), serde_json::Value::String(bin.clone()));
        }
        if let Some(last4) = card.last4.as_ref() {
            blob.insert(
                "last4".to_string(),
                serde_json::Value::String(last4.clone()),
            );
        }
        if let Some(brand_code) = card.brand_code.as_ref() {
            blob.insert(
                "brand_code".to_string(),
                serde_json::Value::String(brand_code.clone()),
            );
        }
    }

    Secret::new(serde_json::json!({
        constants::BRAINTREE_THREE_DS_FEATURE_KEY: serde_json::Value::Object(blob),
    }))
}

/// Maps Braintree's `ThreeDSecureAuthentication` onto the canonical UCS `AuthenticationData`.
///
/// Shared by legs 2 and 3 rather than cloned, so the two cannot drift: they deserialize the same
/// Braintree type from the same selection set. Every one of the 17 fields is written explicitly —
/// `AuthenticationData` derives no `Default`.
fn build_braintree_authentication_data(
    authentication: &BraintreeThreeDSecureAuthentication,
    lookup: Option<&BraintreeThreeDSecureLookupData>,
) -> router_request_types::AuthenticationData {
    router_request_types::AuthenticationData {
        // `transactionStatus` (3DS 2) rather than `paresStatus` (3DS 1): the latter was null on
        // every challenge capture and merely duplicated the former on the successes.
        trans_status: authentication
            .transaction_status
            .and_then(BraintreeThreeDSecureStatusIndicator::to_transaction_status),
        eci: authentication.eci_flag.clone(),
        cavv: authentication.cavv.clone(),
        // Braintree exposes no Mastercard UCAF member; deriving one from `eciFlag` would be a
        // fabricated claim.
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: authentication.three_d_secure_server_transaction_id.clone(),
        // A parse failure degrades to `None`. Braintree returns a full three-part semver
        // ("2.1.0" / "2.2.0"), but a two-part value must not abort the whole response.
        message_version: authentication
            .version
            .as_ref()
            .or_else(|| lookup.and_then(|lookup| lookup.version.as_ref()))
            .and_then(|version| version.parse::<common_utils::types::SemanticVersion>().ok()),
        ds_trans_id: authentication.directory_server_transaction_id.clone(),
        acs_transaction_id: authentication.acs_transaction_id.clone(),
        // The LOOKUP's transaction id, from the lookup-data block — distinct from `xId`, the
        // legacy Cardinal XID, which has no slot here and travels in `connector_feature_data`.
        transaction_id: lookup.and_then(|lookup| lookup.transaction_id.clone()),
        // Cartes Bancaires specific; Braintree returns none of its three members.
        network_params: None,
        // Braintree signals an applied SCA exemption through `authenticationStatus`
        // (`EXEMPTION_*_SUCCESSFUL`), not as a separate field. Deriving the enum from the status
        // would be UCS making a compliance claim it is not entitled to make.
        exemption_indicator: None,
        // No timestamp on the payload, and `now()` is not a substitute.
        created_at: None,
        // CRes / RReq challenge fields. Braintree exposes none of them on
        // `ThreeDSecureAuthentication` — `transactionStatusReason` is a Braintree-scoped reason
        // string, not a 3DS `challengeCode`, and goes to `connector_feature_data` instead.
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        // Decoupled authentication already reaches the caller through `trans_status`; a second,
        // weaker encoding would be redundant.
        authentication_type: None,
    }
}

// ---------------------------------------------------------------------------------------------
// Leg 1 — PreAuthenticate
// ---------------------------------------------------------------------------------------------

/// Variables for [`constants::PRE_AUTHENTICATE_MUTATION`]. The two members are the two root
/// mutation fields' inputs; they share no data, which is what makes the single document legal.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreePreAuthenticateVariables<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    card: InputData<T>,
    client_token: InputClientTokenData,
}

/// Two shapes, because `PreAuthenticate` can be reached with either a raw card or a payment method
/// the caller already tokenized.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum BraintreePreAuthenticateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    /// Raw card: mint the nonce leg 2 needs AND the client token, in one round trip.
    TokenizeAndBootstrap(GenericBraintreeRequest<BraintreePreAuthenticateVariables<T>>),
    /// Already tokenized: the caller holds the nonce, so only the client token is missing.
    BootstrapOnly(BraintreeClientTokenRequest),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreePreAuthenticateData {
    /// Absent on the `BootstrapOnly` shape — the document did not select it.
    tokenize_credit_card: Option<TokenizeCreditCardData>,
    create_client_token: ClientToken,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreePreAuthenticateSuccess {
    data: BraintreePreAuthenticateData,
}

/// `ErrorResponse` MUST be listed first and MUST require `errors`.
///
/// Braintree answers HTTP 200 for everything, and its error bodies carry BOTH a populated
/// `errors[]` and a `"data"` key (`{"data":{"createClientToken":null}}`), so an untagged enum that
/// tried the success variant first — or that discriminated on the presence of `data` — would
/// misclassify a hard error as a success with a null payload. Schema-validation errors carry no
/// `data` key at all, which is why the error variant must not require one.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreePreAuthenticateResponse {
    ErrorResponse(Box<ErrorResponse>),
    Success(Box<BraintreePreAuthenticateSuccess>),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                domain_types::connector_flow::PreAuthenticate,
                PaymentFlowData,
                connector_types::PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BraintreePreAuthenticateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                domain_types::connector_flow::PreAuthenticate,
                PaymentFlowData,
                connector_types::PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_account_id = resolve_merchant_account_id(
            item.router_data.request.metadata.as_ref(),
            &item.router_data.connector_config,
        )?;
        let client_token = InputClientTokenData {
            client_token: ClientTokenInput {
                merchant_account_id,
            },
        };

        match item.router_data.request.payment_method_data.as_ref() {
            Some(PaymentMethodData::Card(card_data)) => {
                Ok(Self::TokenizeAndBootstrap(GenericBraintreeRequest {
                    query: constants::PRE_AUTHENTICATE_MUTATION.to_string(),
                    variables: BraintreePreAuthenticateVariables {
                        card: InputData {
                            credit_card: CreditCardData {
                                number: card_data.card_number.clone(),
                                expiration_year: card_data.card_exp_year.clone(),
                                expiration_month: card_data.card_exp_month.clone(),
                                cvv: card_data.card_cvc.clone(),
                                cardholder_name: item
                                    .router_data
                                    .resource_common_data
                                    .get_optional_billing_full_name()
                                    .unwrap_or_else(|| Secret::new(String::new())),
                            },
                        },
                        client_token,
                    },
                }))
            }
            // The caller already exchanged the card for a Braintree single-use nonce, so the only
            // thing still missing is the client token.
            Some(PaymentMethodData::PaymentMethodToken(_)) => {
                Ok(Self::BootstrapOnly(BraintreeClientTokenRequest {
                    query: constants::CLIENT_TOKEN_MUTATION.to_string(),
                    variables: VariableClientTokenInput {
                        input: client_token,
                    },
                }))
            }
            Some(_) => Err(error_stack::report!(IntegrationError::NotSupported {
                message: utils::get_unimplemented_payment_method_error_message("braintree"),
                connector: "Braintree",
                context: Default::default(),
            })),
            // Fail closed: there is nothing to authenticate, and an empty client token would hand
            // the caller a bootstrap it can never complete.
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "payment_method_data",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the card (or a Braintree payment-method token) on the \
                         PreAuthenticate request. Braintree's 3D Secure lookup is keyed on a \
                         single-use payment method, which this leg mints from the card."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Mutation--tokenizeCreditCard"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "Braintree-hosted 3DS PreAuthenticate invoked with no payment method"
                            .to_string(),
                    ),
                },
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreePreAuthenticateResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PreAuthenticate,
        PaymentFlowData,
        connector_types::PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BraintreePreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            // No status is written on the error path. `get_error_response` leaves
            // `attempt_status: None` so the flow's own status stands, and this builder is shared
            // with Refund, Capture and RSync — hardcoding a failure here would report a terminal
            // refund failure for a transport error.
            BraintreePreAuthenticateResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreePreAuthenticateResponse::Success(success) => {
                let client_token = success.data.create_client_token.client_token.clone();
                if client_token.peek().is_empty() {
                    // `clientToken` is nullable in the SDL. Handing the caller an empty bootstrap
                    // is worse than failing, because it fails later and somewhere else.
                    return Err(utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree returned an empty clientToken on createClientToken; the 3D \
                         Secure device-data bootstrap cannot be built from it",
                    )
                    .into());
                }

                // Either the nonce this leg just minted, or the one the caller already held.
                let payment_method_id =
                    match success.data.tokenize_credit_card.as_ref() {
                        Some(tokenized) => tokenized.payment_method.id.clone(),
                        None => match item.router_data.request.payment_method_data.as_ref() {
                            Some(PaymentMethodData::PaymentMethodToken(token_data)) => {
                                token_data.token.clone()
                            }
                            _ => return Err(utils::unexpected_response_fail(
                                item.http_code,
                                "Braintree returned no tokenizeCreditCard payload and the request \
                                 carried no payment-method token, so the 3D Secure lookup has no \
                                 paymentMethodId to spend",
                            )
                            .into()),
                        },
                    };

                let connector_feature_data = build_braintree_three_ds_feature_data(
                    &payment_method_id,
                    Some(&client_token),
                    None,
                    None,
                    None,
                );

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        // Exactly what this leg produced: the credential a browser needs to
                        // collect device data. Non-terminal, so the composite loop advances to
                        // `Authenticate`.
                        status: enums::AttemptStatus::DeviceDataCollectionPending,
                        connector_feature_data: Some(connector_feature_data),
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                        resource_id: Some(ResponseId::ConnectorTransactionId(
                            payment_method_id.peek().clone(),
                        )),
                        // Nothing has been authenticated yet; populating this would be a
                        // fabricated claim about an authentication outcome.
                        authentication_data: None,
                        // Braintree's server-side lookup needs no browser round trip of its own:
                        // `dfReferenceId` is optional and `transactionInformation.browserInformation`
                        // satisfies the lookup on its own, so this leg hands back the bootstrap
                        // credential in `connector_feature_data` and lets the composite loop run
                        // `Authenticate` immediately. A caller that wants client-side device data
                        // uses the client token and returns the `dfReferenceId` on the next call.
                        redirection_data: None,
                        // `createClientToken` returns no merchant-visible reference, and
                        // `clientMutationId` is not selected. Do not synthesise one.
                        connector_response_reference_id: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Leg 2 — Authenticate (`performThreeDSecureLookup`)
// ---------------------------------------------------------------------------------------------

pub type BraintreeAuthenticateRequest =
    GenericBraintreeRequest<GenericVariableInput<PerformThreeDSecureLookupInput>>;

/// Braintree GraphQL `PerformThreeDSecureLookupInput`.
///
/// The three members the schema also defines and this connector deliberately does not send:
/// `dataOnlyRequested` and `cardAdd` are merchant policy with no UCS request member to carry them,
/// and `merchantInitiatedRequest` is the 3RI / prior-authentication tree, which is out of scope.
/// `requestAuthenticationChallenge` and `requestedExemptionType` are likewise omitted: forcing a
/// challenge defeats frictionless flow, and claiming an SCA exemption the merchant has not
/// asserted is a compliance statement UCS is not entitled to make.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformThreeDSecureLookupInput {
    /// `ID!` — the single-use nonce from leg 1.
    payment_method_id: Secret<String>,
    /// `Amount!` — a major-unit decimal string. Produced by the connector's own
    /// `amount_converter`, never by `MinorUnit::to_string()`: "1234" is itself a valid `Amount`,
    /// so that mistake is a silent 100x overstatement Braintree cannot detect.
    amount: StringMajorUnit,
    merchant_account_id: Secret<String>,
    /// Braintree distinguishes "absent" from "present and null", so every optional member is
    /// skipped rather than serialized as null.
    #[serde(skip_serializing_if = "Option::is_none")]
    df_reference_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transaction_information: Option<ThreeDSecureLookupTransactionInformationInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cardholder_information: Option<ThreeDSecureLookupCardholderInformationInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureLookupTransactionInformationInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    device_channel: Option<BraintreeThreeDSecureDeviceChannel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<pii::Email>,
    /// A sibling of `browserInformation`, one level up — the schema rejects it nested inside.
    #[serde(skip_serializing_if = "Option::is_none")]
    ip_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    browser_information: Option<ThreeDSecureLookupBrowserInformationInput>,
}

/// Braintree GraphQL `ThreeDSecureLookupBrowserInformationInput` — all nine members map 1:1 from
/// `BrowserInformation`. Note `javascriptEnabled`: one capital, unlike the UCS `java_script_enabled`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureLookupBrowserInformationInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    java_enabled: Option<bool>,
    #[serde(rename = "javascriptEnabled", skip_serializing_if = "Option::is_none")]
    java_script_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accept_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    screen_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    screen_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_zone: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_agent: Option<String>,
}

/// `ThreeDSecureLookupCardholderInformationInput` has exactly one member.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureLookupCardholderInformationInput {
    billing_address: ThreeDSecureLookupBillingAddressInput,
}

/// Braintree GraphQL `ThreeDSecureLookupBillingAddressInput`.
///
/// `countryCode` here is a plain `String`, NOT the versioned `CountryCode` scalar the Authorize
/// billing address uses, so the alpha-3/alpha-2 boundary at `Braintree-Version 2021-02-01` does
/// not apply on this leg. Send the country code exactly as UCS holds it and do not reuse the
/// Authorize alpha-3 conversion.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureLookupBillingAddressInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    given_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    surname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locality: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone_number: Option<Secret<String>>,
}

impl ThreeDSecureLookupBillingAddressInput {
    fn is_empty(&self) -> bool {
        self.given_name.is_none()
            && self.surname.is_none()
            && self.line1.is_none()
            && self.line2.is_none()
            && self.locality.is_none()
            && self.region.is_none()
            && self.postal_code.is_none()
            && self.country_code.is_none()
            && self.phone_number.is_none()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeThreeDSecureLookupPayload {
    three_d_secure_lookup_data: Option<BraintreeThreeDSecureLookupData>,
    payment_method: Option<BraintreeThreeDSecurePaymentMethod>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeAuthenticateData {
    perform_three_d_secure_lookup: Option<BraintreeThreeDSecureLookupPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeThreeDSecureLookupSuccess {
    data: BraintreeAuthenticateData,
}

/// `ErrorResponse` first, for the reason spelled out on [`BraintreePreAuthenticateResponse`].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeAuthenticateResponse {
    ErrorResponse(Box<ErrorResponse>),
    Success(Box<BraintreeThreeDSecureLookupSuccess>),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                domain_types::connector_flow::Authenticate,
                PaymentFlowData,
                connector_types::PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BraintreeAuthenticateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                domain_types::connector_flow::Authenticate,
                PaymentFlowData,
                connector_types::PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        // An incoming `authentication_data` means the caller already performed 3D Secure with its
        // own MPI and wants the Authorize pass-through, not a second, contradictory Braintree
        // authentication. Reject rather than silently authenticating twice.
        if request.authentication_data.is_some() {
            return Err(IntegrationError::NotSupported {
                message: "Braintree-hosted 3D Secure authentication with externally supplied \
                          authentication_data"
                    .to_string(),
                connector: "Braintree",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Braintree-hosted and external-MPI 3D Secure are mutually exclusive. Send \
                         an externally performed authentication on PaymentService/Authorize, where \
                         it is declared through options.threeDSecureAuthentication.passThrough; do \
                         not route it through the authentication legs."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Input--ThreeDSecurePassThroughInput"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "authentication_data present on a Braintree-hosted Authenticate leg"
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        // The nonce: from leg 1's `connector_feature_data` first (the composite path, where the
        // caller never sees a token), then from an explicitly tokenized payment method (the
        // granular `PaymentMethodAuthenticationService/Authenticate` path).
        //
        // A raw `Card` is NOT accepted: re-tokenizing here would mint a second nonce and make the
        // leg non-idempotent, and the lookup consumes whichever nonce it is given.
        let payment_method_id = braintree_hosted_three_ds_nonce(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        )
        .or_else(|| match request.payment_method_data.as_ref() {
            Some(PaymentMethodData::PaymentMethodToken(token_data)) => {
                Some(token_data.token.clone())
            }
            _ => None,
        })
        .ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "payment_method_data",
            context: domain_types::errors::IntegrationErrorContext {
                suggested_action: Some(
                    "Run the PreAuthenticate leg first and pass its connector_feature_data back \
                     on this request, or send an already-tokenized Braintree payment method. \
                     performThreeDSecureLookup requires a single-use paymentMethodId and will not \
                     accept a raw card."
                        .to_string(),
                ),
                doc_url: Some(
                    "https://graphql.braintreepayments.com/reference/#Input--PerformThreeDSecureLookupInput"
                        .to_string(),
                ),
                additional_context: Some(
                    "no Braintree payment-method nonce resolvable for the 3D Secure lookup"
                        .to_string(),
                ),
            },
        })?;

        let currency = request
            .currency
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the payment currency. Braintree's Amount scalar is a major-unit \
                         decimal string, so the minor-unit amount cannot be converted without it."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "currency absent on a Braintree 3D Secure lookup".to_string(),
                    ),
                },
            })?;
        let amount = item
            .connector
            .amount_converter
            .convert(request.amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let merchant_account_id = resolve_merchant_account_id(
            request.metadata.as_ref(),
            &item.router_data.connector_config,
        )?;

        // Device data, ingress 1 of 2 and the preferred one: the CardinalCommerce reference that
        // joins this lookup to device data already collected in the cardholder's browser by
        // `threeDSecure.prepareLookup`, which leg 1's client token bootstrapped.
        let df_reference_id = request
            .redirect_response
            .as_ref()
            .and_then(|redirect| redirect.payload.as_ref())
            .and_then(|payload| {
                let payload = payload.clone().expose();
                payload
                    .get("dfReferenceId")
                    .or_else(|| payload.get("df_reference_id"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .map(Secret::new);

        // Device data, ingress 2 of 2. Not a lesser option: the lookup is schema-valid and
        // succeeds on browser information alone.
        let browser_information = request.browser_info.as_ref().map(|browser| {
            ThreeDSecureLookupBrowserInformationInput {
                java_enabled: browser.java_enabled,
                java_script_enabled: browser.java_script_enabled,
                accept_header: browser.accept_header.clone(),
                language: browser.language.clone(),
                color_depth: browser.color_depth,
                screen_height: browser.screen_height,
                screen_width: browser.screen_width,
                time_zone: browser.time_zone,
                user_agent: browser.user_agent.clone(),
            }
        });

        if df_reference_id.is_none() && browser_information.is_none() {
            // Braintree would run a lookup with no device data at all, but the authentication
            // would be materially weaker and the liability-shift outcome misleading. Fail closed.
            return Err(IntegrationError::MissingRequiredField {
                field_name: "browser_info",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Send browser_info, or return the dfReferenceId produced by \
                         threeDSecure.prepareLookup in redirect_response.payload. Braintree's 3D \
                         Secure lookup accepts either, but a lookup with neither is materially \
                         weaker authentication."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Input--ThreeDSecureLookupBrowserInformationInput"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "neither dfReferenceId nor browser_info supplied to the Braintree 3D \
                         Secure lookup"
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        let device_channel = match request.device_channel {
            Some(connector_types::DeviceChannel::App) => {
                Some(BraintreeThreeDSecureDeviceChannel::Sdk)
            }
            Some(connector_types::DeviceChannel::Browser) => {
                Some(BraintreeThreeDSecureDeviceChannel::Browser)
            }
            // Do not assert a channel the request does not evidence.
            None => browser_information
                .as_ref()
                .map(|_| BraintreeThreeDSecureDeviceChannel::Browser),
        };

        let ip_address = request
            .browser_info
            .as_ref()
            .and_then(|browser| browser.ip_address)
            .map(|ip| Secret::new(ip.to_string()));

        let transaction_information = (device_channel.is_some()
            || request.email.is_some()
            || ip_address.is_some()
            || browser_information.is_some())
        .then_some(ThreeDSecureLookupTransactionInformationInput {
            device_channel,
            email: request.email.clone(),
            ip_address,
            browser_information,
        });

        let billing = item.router_data.resource_common_data.get_optional_billing();
        let billing_address_details = billing.and_then(|billing| billing.address.as_ref());
        let billing_address = ThreeDSecureLookupBillingAddressInput {
            given_name: billing_address_details.and_then(|a| a.first_name.clone()),
            surname: billing_address_details.and_then(|a| a.last_name.clone()),
            line1: billing_address_details.and_then(|a| a.line1.clone()),
            line2: billing_address_details.and_then(|a| a.line2.clone()),
            locality: billing_address_details.and_then(|a| a.city.clone()),
            region: billing_address_details.and_then(|a| a.state.clone()),
            postal_code: billing_address_details.and_then(|a| a.zip.clone()),
            country_code: billing_address_details
                .and_then(|a| a.country)
                .map(|country| country.to_string()),
            phone_number: billing
                .and_then(|billing| billing.phone.as_ref())
                .and_then(|phone| phone.number.clone()),
        };
        // Braintree distinguishes "absent" from "present and empty"; never emit `{}`.
        let cardholder_information = (!billing_address.is_empty())
            .then_some(ThreeDSecureLookupCardholderInformationInput { billing_address });

        Ok(Self {
            query: constants::AUTHENTICATE_MUTATION.to_string(),
            variables: GenericVariableInput {
                input: PerformThreeDSecureLookupInput {
                    payment_method_id,
                    amount,
                    merchant_account_id,
                    df_reference_id,
                    transaction_information,
                    cardholder_information,
                },
            },
        })
    }
}

impl BraintreeThreeDSecureLookupPayload {
    /// The challenge-vs-frictionless discriminator, and the single easiest way to get this flow
    /// wrong.
    ///
    /// `threeDSecureLookupData` comes back NON-NULL on every outcome, frictionless success and
    /// outright failure included; on those, `acsUrl` and `pareq` are null while `authenticationId`,
    /// `md`, `termUrl`, `transactionId` and `version` are still populated. So
    /// `three_d_secure_lookup_data.is_some()` would emit a redirect on every single outcome and
    /// strand the payment in a challenge that does not exist.
    ///
    /// Both halves are required: `CHALLENGE_REQUIRED` is the semantic signal, a non-null `acsUrl`
    /// the structural one — and a redirect form cannot be built without the latter anyway.
    fn is_challenge(&self) -> bool {
        let status_says_challenge = self
            .payment_method
            .as_ref()
            .and_then(BraintreeThreeDSecurePaymentMethod::authentication)
            .and_then(|authentication| authentication.authentication_status)
            == Some(BraintreeThreeDSecureAuthenticationStatus::ChallengeRequired);

        let acs_url_present = self
            .three_d_secure_lookup_data
            .as_ref()
            .and_then(|lookup| lookup.acs_url.as_ref())
            .is_some();

        status_says_challenge && acs_url_present
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreeAuthenticateResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::Authenticate,
        PaymentFlowData,
        connector_types::PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BraintreeAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeAuthenticateResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreeAuthenticateResponse::Success(success) => {
                let payload = success.data.perform_three_d_secure_lookup.ok_or_else(|| {
                    utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree returned a 200 with no performThreeDSecureLookup payload",
                    )
                })?;

                let payment_method = payload.payment_method.as_ref().ok_or_else(|| {
                    utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree's 3D Secure lookup returned no paymentMethod, so there is no \
                         nonce for the subsequent authorization to spend",
                    )
                })?;

                // The `... on CreditCardDetails` fragment yields `{}` rather than an error for a
                // non-card payment method, so an absent authentication block is detected here
                // explicitly and never inferred from a parse failure.
                let authentication = payment_method.authentication().ok_or_else(|| {
                    utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree's 3D Secure lookup returned a payment method with no \
                         threeDSecure.authentication block; this is not a frictionless success",
                    )
                })?;

                let is_challenge = payload.is_challenge();
                let lookup = payload.three_d_secure_lookup_data.as_ref();

                let status = authentication
                    .authentication_status
                    .map(BraintreeThreeDSecureAuthenticationStatus::to_attempt_status)
                    // Braintree returned the block but no status. Not a success, not a failure.
                    .unwrap_or(enums::AttemptStatus::Unspecified);

                let redirection_data = if is_challenge {
                    let lookup = lookup.ok_or_else(|| {
                        utils::unexpected_response_fail(
                            item.http_code,
                            "Braintree signalled CHALLENGE_REQUIRED with no threeDSecureLookupData",
                        )
                    })?;
                    let acs_url = lookup.acs_url.clone().ok_or_else(|| {
                        utils::unexpected_response_fail(
                            item.http_code,
                            "Braintree signalled CHALLENGE_REQUIRED with a null acsUrl; the \
                             challenge cannot be rendered and this is not a frictionless success",
                        )
                    })?;

                    let mut form_fields = std::collections::HashMap::new();
                    // `PaReq` / `MD` / `TermUrl` are Braintree's published field names for a
                    // server-side lookup and are what the ACS endpoint expects. `RedirectForm::Form`
                    // types `form_fields` as a plain map, so `pareq` and `term_url` — the latter a
                    // signed credential — are unwrapped only here, at the type boundary that
                    // demands it.
                    if let Some(pareq) = lookup.pareq.as_ref() {
                        form_fields.insert("PaReq".to_string(), pareq.peek().clone());
                    }
                    if let Some(md) = lookup.md.as_ref() {
                        form_fields.insert("MD".to_string(), md.clone());
                    }
                    if let Some(term_url) = lookup.term_url.as_ref() {
                        form_fields.insert("TermUrl".to_string(), term_url.peek().clone());
                    }

                    Some(Box::new(RedirectForm::Form {
                        endpoint: acs_url,
                        method: common_utils::request::Method::Post,
                        form_fields,
                    }))
                } else {
                    None
                };

                // The lookup ALWAYS consumes the input nonce and returns a different payment
                // method id; a replay of the old one answers "Nonce is already consumed". The
                // subsequent authorization must spend the new one, so it leaves by both the
                // prominent channel (`resource_id`) and the one the composite dispatcher forwards
                // into Authorize (`connector_feature_data`).
                let new_payment_method_id = payment_method.id.clone();
                let feature_data = build_braintree_three_ds_feature_data(
                    &new_payment_method_id,
                    None,
                    Some(authentication),
                    payment_method.card_details(),
                    lookup,
                );

                let authentication_id = lookup.and_then(|lookup| lookup.authentication_id.clone());

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        reference_id: authentication_id.clone(),
                        connector_feature_data: Some(feature_data.clone()),
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::AuthenticateResponse {
                        resource_id: Some(ResponseId::ConnectorTransactionId(
                            new_payment_method_id.peek().clone(),
                        )),
                        redirection_data,
                        // Populated on both paths: even on a challenge the payload legitimately
                        // carries the 3DS server / ACS / DS transaction ids and the ECI, which the
                        // caller needs to correlate the challenge. Only `cavv` and a settled
                        // `trans_status` are challenge-path absent, and both are optional.
                        authentication_data: Some(build_braintree_authentication_data(
                            authentication,
                            lookup,
                        )),
                        connector_feature_data: Some(feature_data.expose()),
                        connector_response_reference_id: authentication_id,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Leg 3 — PostAuthenticate (`node(id:)` readback)
// ---------------------------------------------------------------------------------------------

/// GraphQL `$id: ID!`. Every other Braintree flow sends `{ "input": ... }`; this one does not,
/// because `node(id:)` takes a bare id argument — so `GenericVariableInput<T>` must NOT be reused.
#[derive(Debug, Clone, Serialize)]
pub struct BraintreePostAuthenticateVariables {
    /// Secret because a bare Braintree payment-method id is all `chargeCreditCard` needs.
    id: Secret<String>,
}

pub type BraintreePostAuthenticateRequest =
    GenericBraintreeRequest<BraintreePostAuthenticateVariables>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeNodeData {
    node: Option<BraintreeThreeDSecurePaymentMethod>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeThreeDSecureResultSuccess {
    data: BraintreeNodeData,
}

/// `ErrorResponse` first. The `NOT_FOUND` body carries BOTH a populated `errors[]` and
/// `{"data":{"node":null}}`, so a success-first enum would read a hard error as a success with a
/// null node.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreePostAuthenticateResponse {
    ErrorResponse(Box<ErrorResponse>),
    Success(Box<BraintreeThreeDSecureResultSuccess>),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                domain_types::connector_flow::PostAuthenticate,
                PaymentFlowData,
                connector_types::PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BraintreePostAuthenticateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: BraintreeRouterData<
            RouterDataV2<
                domain_types::connector_flow::PostAuthenticate,
                PaymentFlowData,
                connector_types::PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // `connector_order_reference_id` is leg 2's `resource_id`, threaded here by the composite
        // dispatcher. `payment_method_data` is deliberately NOT a fallback: on the composite path
        // it still holds the ORIGINAL instrument, whose nonce the lookup already consumed, and
        // `node(id:)` would answer NOT_FOUND for it.
        //
        // `redirect_response` is deliberately not read either. The ACS posts its PaRes to
        // Braintree's own termUrl, Braintree records the outcome on the payment method and then
        // redirects the browser to a Braintree-owned page, so the browser comes back to the
        // merchant carrying nothing this leg needs. Calling `get_redirect_response_payload()` here
        // would raise MissingRequiredField on every real Braintree post-challenge request.
        let id = item
            .router_data
            .request
            .connector_order_reference_id
            .clone()
            .or_else(|| item.router_data.resource_common_data.reference_id.clone())
            .map(Secret::new)
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "connector_order_reference_id",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the connector_transaction_id returned by the Authenticate leg as \
                         connector_order_reference_id. Braintree records the 3D Secure outcome on \
                         the payment method that leg returned, and node(id:) is the only way to \
                         read it back."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Query--node".to_string(),
                    ),
                    additional_context: Some(
                        "PostAuthenticate invoked without the payment-method id produced by the \
                         Braintree 3D Secure lookup"
                            .to_string(),
                    ),
                },
            })?;

        Ok(Self {
            query: constants::POST_AUTHENTICATE_QUERY.to_string(),
            variables: BraintreePostAuthenticateVariables { id },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreePostAuthenticateResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PostAuthenticate,
        PaymentFlowData,
        connector_types::PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BraintreePostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreePostAuthenticateResponse::ErrorResponse(error_response) => Ok(Self {
                response: build_error_response(&error_response.errors, item.http_code)
                    .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreePostAuthenticateResponse::Success(success) => {
                let payment_method = success.data.node.ok_or_else(|| {
                    utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree's node(id:) readback resolved to no PaymentMethod",
                    )
                })?;

                // An absent block means the trio was skipped or the wrong id was threaded. It is
                // NOT a frictionless success and must never be reported as one.
                let authentication = payment_method.authentication().ok_or_else(|| {
                    utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree's node(id:) readback returned a payment method with no \
                         threeDSecure.authentication block; no 3D Secure lookup was ever run \
                         against it",
                    )
                })?;

                let status = authentication
                    .authentication_status
                    .map(BraintreeThreeDSecureAuthenticationStatus::to_attempt_status)
                    .unwrap_or(enums::AttemptStatus::Unspecified);

                if status == enums::AttemptStatus::AuthenticationSuccessful
                    && authentication.cavv.is_none()
                {
                    // Not an error: DATA_ONLY_SUCCESSFUL and the exemption statuses legitimately
                    // carry no cryptogram. Worth surfacing, because for AUTHENTICATE_SUCCESSFUL it
                    // is anomalous.
                    tracing::warn!(
                        target: "braintree_three_ds",
                        authentication_status = ?authentication.authentication_status,
                        "Braintree reported a successful 3D Secure authentication with no CAVV"
                    );
                }

                let payment_method_id = payment_method.id.clone();
                // `PostAuthenticateResponse` has no `resource_id`, so `connector_feature_data` on
                // the flow data is the only channel the 3DS-verified nonce can leave by — and the
                // composite dispatcher forwards it into the Authorize request, where the Authorize
                // builder reads it back as both the payment method to charge and the marker that
                // suppresses the external-MPI pass-through.
                let feature_data = build_braintree_three_ds_feature_data(
                    &payment_method_id,
                    None,
                    Some(authentication),
                    payment_method.card_details(),
                    None,
                );

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        reference_id: Some(payment_method_id.peek().clone()),
                        connector_feature_data: Some(feature_data),
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                        authentication_data: Some(build_braintree_authentication_data(
                            authentication,
                            None,
                        )),
                        // The variant has no `resource_id`; the id the caller needs next is the
                        // spendable one, so it goes here.
                        connector_response_reference_id: Some(payment_method_id.peek().clone()),
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}
