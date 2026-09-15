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
        Authenticate, Authorize, Capture, ClientAuthenticationToken, PSync, PaymentMethodToken,
        PostAuthenticate, PreAuthenticate, RSync, RepeatPayment, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        self, AmountInfo, ApplePayPaymentRequest, ApplePaySessionResponse,
        ApplepayClientAuthenticationResponse, ClientAuthenticationTokenData,
        ClientAuthenticationTokenRequestData, GooglePaySessionResponse,
        GpayAllowedMethodsParameters, GpayAllowedPaymentMethods, GpayClientAuthenticationResponse,
        GpayMerchantInfo, GpayShippingAddressParameters, GpayTokenParameters,
        GpayTokenizationSpecification, GpayTransactionInfo, MandateReference, NextActionCall,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentRequestMetadata, PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        PaypalClientAuthenticationResponse, PaypalTransactionInfo, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SdkNextAction,
        SecretInfoToInitiateSdk, SetupMandateRequestData, ThirdPartySdkSessionResponse,
    },
    errors::{ConnectorError, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types,
    router_response_types::RedirectForm,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::info;

pub const BRAINTREE_CONNECTOR_NAME: &str = "braintree";

pub mod constants {
    pub const CHANNEL_CODE: &str = "HyperSwitchBT_Ecom";
    pub const CLIENT_TOKEN_MUTATION: &str = "mutation createClientToken($input: CreateClientTokenInput!) { createClientToken(input: $input) { clientToken}}";
    pub const TOKENIZE_CREDIT_CARD: &str = "mutation  tokenizeCreditCard($input: TokenizeCreditCardInput!) { tokenizeCreditCard(input: $input) { clientMutationId paymentMethod { id } } }";
    // Braintree-hosted 3DS, leg 1 (PreAuthenticate). `tokenizeCreditCard` and `createClientToken`
    // take disjoint inputs, so neither needs the other's output and both can be root fields of a
    // single document (GraphQL executes root mutation fields serially, in document order). This
    // keeps the leg to one HTTP call and yields every member of `RedirectForm::Braintree`
    // (client token + nonce + BIN + return URL) without a new domain type or proto message.
    // Verified accepted by the Braintree sandbox under `Braintree-Version: 2019-01-01`.
    pub const PRE_AUTHENTICATE_MUTATION: &str = "mutation braintreeThreeDsPreAuthenticate($card: TokenizeCreditCardInput!, $clientToken: CreateClientTokenInput!) { tokenizeCreditCard(input: $card) { paymentMethod { id } } createClientToken(input: $clientToken) { clientToken } }";
    // Braintree-hosted 3DS, leg 2 (Authenticate): the server-side 3DS lookup.
    //
    // The selection set below was introspected AND exercised end-to-end against the Braintree
    // sandbox under `Braintree-Version: 2019-01-01` (four outcomes: CHALLENGE_REQUIRED,
    // AUTHENTICATE_SUCCESSFUL, AUTHENTICATE_FRICTIONLESS_FAILED,
    // AUTHENTICATE_UNABLE_TO_AUTHENTICATE).
    //
    // `threeDSecure` MUST be traversed through `.authentication`. At the pinned version the
    // served schema already types `CreditCardDetails.threeDSecure` as `ThreeDSecureDetails`
    // (the 2020-10-07 retype), whose flat scalars are all @deprecated; selecting them directly
    // is a hard GraphQL validation error. `Braintree-Version` gates the interpretation of
    // values, not the shape of the schema.
    //
    // `details` is a union (`PaymentMethodDetails`), so the inline fragment is mandatory.
    pub const AUTHENTICATE_MUTATION: &str = "mutation braintreeThreeDSecureLookup($input: PerformThreeDSecureLookupInput!) { performThreeDSecureLookup(input: $input) { threeDSecureLookupData { acsUrl authenticationId version pareq md termUrl transactionId } paymentMethod { id details { ... on CreditCardDetails { bin last4 brandCode threeDSecure { authentication { cavv eciFlag liabilityShifted liabilityShiftPossible cardEnrolled authenticationStatus version directoryServerTransactionId xId threeDSecureServerTransactionId acsTransactionId paresStatus transactionStatus transactionStatusReason } } } } } } }";
    // Braintree-hosted 3DS, leg 3 (PostAuthenticate): read the settled authentication back off
    // the payment method that leg 2 returned.
    //
    // This is a QUERY, not a mutation, and that is not a shortcut. The root Mutation type has
    // 112 fields and exactly one matches /3d|3ds|threeDSecure|challenge|authenticat/i —
    // `performThreeDSecureLookup`, which is leg 2. There is NO post-challenge completion
    // mutation, and the root Query type has no 3DS field either. The ACS posts its PaRes to
    // Braintree's own termUrl, Braintree records the outcome ON the payment method, and
    // PaymentMethod implements Node — so `node(id:)` is the only retrieval path that exists.
    // Introspected and exercised live at Braintree-Version 2019-01-01; the readback was
    // observed transitioning CHALLENGE_REQUIRED -> AUTHENTICATE_UNABLE_TO_AUTHENTICATE on the
    // same id after a PaRes reached that termUrl, which is what makes it a live view rather
    // than a snapshot.
    //
    // Pass the id VERBATIM. base64 and Relay-style "PaymentMethod:<id>" global ids both return
    // NOT_FOUND, as does the spent input nonce and an `authenticationId`.
    //
    // `node` returns the Node INTERFACE and `details` is the PaymentMethodDetails UNION, so both
    // inline fragments are mandatory. The `authentication` selection is character-for-character
    // the one in `AUTHENTICATE_MUTATION`: both legs deserialize the same Braintree type into the
    // same Rust struct, and a divergence between them would be a latent bug, not a saving.
    //
    // Do NOT add `createdAt`: on a single-use payment method it returns a partial error
    // ("Fetching `createdAt` on a single-use payment method is not supported from this
    // operation.", errorClass NOT_IMPLEMENTED) which would drag a successful readback into the
    // error arm. Do NOT add `authenticationInsight`: it requires an `input` argument and
    // selecting it bare is a hard validation error.
    pub const POST_AUTHENTICATE_QUERY: &str = "query braintreeThreeDSecureResult($id: ID!) { node(id: $id) { id ... on PaymentMethod { legacyId usage details { ... on CreditCardDetails { bin last4 brandCode threeDSecure { authentication { cavv eciFlag liabilityShifted liabilityShiftPossible cardEnrolled authenticationStatus version directoryServerTransactionId xId threeDSecureServerTransactionId acsTransactionId paresStatus transactionStatus transactionStatusReason } } } } } } }";
    /// The `connector_feature_data` key that marks a Braintree-HOSTED 3DS authentication.
    /// Written by legs 2 and 3; read by the Authorize builder to decide that the authentication
    /// already lives on the payment method and must NOT be re-sent as external-MPI
    /// pass-through. Producer and consumer share this constant so they cannot drift.
    pub const BRAINTREE_THREE_DS_FEATURE_KEY: &str = "braintree_three_ds";
    // Response selection set is kept in lock-step with `TransactionAuthChargeResponseBody`.
    // Every field below was verified to exist under the pinned `Braintree-Version: 2019-01-01`
    // via a sandbox introspection + live-mutation check (a selection the versioned schema does
    // not expose is a hard GraphQL validation error that would break every Authorize).
    // NOTE: `paymentMethod { id }` is deliberately selected ONLY on the *vault* mutations —
    // on a non-vault charge it would be the single-use token, which must not be reported as a
    // mandate reference.
    pub const CHARGE_CREDIT_CARD_MUTATION: &str = "mutation ChargeCreditCard($input: ChargeCreditCardInput!) { chargeCreditCard(input: $input) { transaction { id legacyId createdAt status orderId amount { value currencyCode } processorAuthorizationResponse { legacyCode message cvvResponse avsPostalCodeResponse avsStreetAddressResponse authorizationId additionalInformation } processorSettlementResponse { legacyCode message } statusHistory { terminal ... on ProcessorDeclinedEvent { declineType processorResponse { legacyCode message additionalInformation } networkResponse { code message } merchantAdviceCodeResponse { code message } } ... on GatewayRejectedEvent { gatewayRejectionReason processorResponse { legacyCode message } networkResponse { code message } merchantAdviceCodeResponse { code message } } ... on FailedEvent { processorResponse { legacyCode message } networkResponse { code message } merchantAdviceCodeResponse { code message } } } paymentMethodSnapshot { ... on CreditCardTransactionDetails { networkTransactionId } } } } }";
    pub const AUTHORIZE_CREDIT_CARD_MUTATION: &str = "mutation authorizeCreditCard($input: AuthorizeCreditCardInput!) { authorizeCreditCard(input: $input) { transaction { id legacyId createdAt status orderId amount { value currencyCode } processorAuthorizationResponse { legacyCode message cvvResponse avsPostalCodeResponse avsStreetAddressResponse authorizationId additionalInformation } processorSettlementResponse { legacyCode message } statusHistory { terminal ... on ProcessorDeclinedEvent { declineType processorResponse { legacyCode message additionalInformation } networkResponse { code message } merchantAdviceCodeResponse { code message } } ... on GatewayRejectedEvent { gatewayRejectionReason processorResponse { legacyCode message } networkResponse { code message } merchantAdviceCodeResponse { code message } } ... on FailedEvent { processorResponse { legacyCode message } networkResponse { code message } merchantAdviceCodeResponse { code message } } } paymentMethodSnapshot { ... on CreditCardTransactionDetails { networkTransactionId } } } } }";
    pub const CAPTURE_TRANSACTION_MUTATION: &str = "mutation captureTransaction($input: CaptureTransactionInput!) { captureTransaction(input: $input) { clientMutationId transaction { id legacyId amount { value currencyCode } status } } }";
    pub const VOID_TRANSACTION_MUTATION: &str = "mutation voidTransaction($input:  ReverseTransactionInput!) { reverseTransaction(input: $input) { clientMutationId reversal { ...  on Transaction { id legacyId amount { value currencyCode } status } } } }";
    pub const REFUND_TRANSACTION_MUTATION: &str = "mutation refundTransaction($input:  RefundTransactionInput!) { refundTransaction(input: $input) {clientMutationId refund { id legacyId amount { value currencyCode } status } } }";
    pub const AUTHORIZE_AND_VAULT_CREDIT_CARD_MUTATION: &str = "mutation authorizeCreditCard($input: AuthorizeCreditCardInput!) { authorizeCreditCard(input: $input) { transaction { id legacyId createdAt status orderId amount { value currencyCode } processorAuthorizationResponse { legacyCode message cvvResponse avsPostalCodeResponse avsStreetAddressResponse authorizationId additionalInformation } processorSettlementResponse { legacyCode message } statusHistory { terminal ... on ProcessorDeclinedEvent { declineType processorResponse { legacyCode message additionalInformation } networkResponse { code message } merchantAdviceCodeResponse { code message } } ... on GatewayRejectedEvent { gatewayRejectionReason processorResponse { legacyCode message } networkResponse { code message } merchantAdviceCodeResponse { code message } } ... on FailedEvent { processorResponse { legacyCode message } networkResponse { code message } merchantAdviceCodeResponse { code message } } } paymentMethodSnapshot { ... on CreditCardTransactionDetails { networkTransactionId } } paymentMethod { id } } } }";
    pub const CHARGE_AND_VAULT_TRANSACTION_MUTATION: &str = "mutation ChargeCreditCard($input: ChargeCreditCardInput!) { chargeCreditCard(input: $input) { transaction { id legacyId createdAt status orderId amount { value currencyCode } processorAuthorizationResponse { legacyCode message cvvResponse avsPostalCodeResponse avsStreetAddressResponse authorizationId additionalInformation } processorSettlementResponse { legacyCode message } statusHistory { terminal ... on ProcessorDeclinedEvent { declineType processorResponse { legacyCode message additionalInformation } networkResponse { code message } merchantAdviceCodeResponse { code message } } ... on GatewayRejectedEvent { gatewayRejectionReason processorResponse { legacyCode message } networkResponse { code message } merchantAdviceCodeResponse { code message } } ... on FailedEvent { processorResponse { legacyCode message } networkResponse { code message } merchantAdviceCodeResponse { code message } } } paymentMethodSnapshot { ... on CreditCardTransactionDetails { networkTransactionId } } paymentMethod { id } } } }";
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
pub type BraintreePreAuthenticateRequest<T> = GenericBraintreeRequest<PreAuthenticateVariables<T>>;
pub type BraintreeAuthenticateRequest =
    GenericBraintreeRequest<GenericVariableInput<PerformThreeDSecureLookupInput>>;

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
    /// Braintree's own de-duplication key: "If subsequent requests are made with the same
    /// `apiRequestKey`, and the first request resulted in a transaction being created (in any
    /// status), then the same transaction in its current status will be returned." It is
    /// populated from the caller's `merchant_request_id` and is never a freshly minted uuid — a
    /// per-call uuid would defeat the purpose and let a retry after a client timeout authorise
    /// twice.
    #[serde(skip_serializing_if = "Option::is_none")]
    api_request_key: Option<String>,
    transaction: TransactionBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<CreditCardTransactionOptions>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum BraintreePaymentsRequest {
    Card(CardPaymentRequest),
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
    /// Descriptor / L2-L3 / address enrichment. Flattened because Braintree carries every one
    /// of these as a direct member of `TransactionInput`, not as a nested object.
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
    /// A card authorize that also vaults the credential is the *first* transaction of a
    /// stored-credential series, so the network needs `RECURRING_FIRST` here. Without it the
    /// subsequent MIT (RepeatPayment, which sends `UNSCHEDULED`) has no CIT to chain to.
    payment_initiator: PaymentInitiatorType,
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

/// Subset of Braintree's `PaymentInitiator` enum that UCS actually emits. The full enum is
/// `ESTIMATED | ESTIMATED_MOTO | INSTALLMENT | INSTALLMENT_FIRST | MOTO | RECURRING |
/// RECURRING_FIRST | UNSCHEDULED`; the remaining members have no UCS source today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentInitiatorType {
    Unscheduled,
    RecurringFirst,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum TransactionBody {
    Regular(RegularTransactionBody),
    Vault(VaultTransactionBody),
    Mandate(MandateTransactionBody),
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

// ---------------------------------------------------------------------------
// Request-side enrichment on `TransactionInput` (descriptor, L2/L3, addresses)
//
// Placement note (Braintree splits these across two sibling objects):
//   * `input.options.billingAddress`      -> billing  (chargeCreditCard / authorizeCreditCard)
//   * `input.transaction.shipping.shippingAddress` -> shipping
// `chargePaymentMethod` / `authorizePaymentMethod` have no `options` member at all, which is why
// the wallet request body (`WalletPaymentInput`) carries neither options nor this enrichment.
// ---------------------------------------------------------------------------

/// `AddressInput`. Braintree exposes four alias pairs for the same concept
/// (`streetAddress`/`addressLine1`, `extendedAddress`/`addressLine2`, `locality`/`adminArea2`,
/// `region`/`adminArea1`) and does not define what happens when both members of a pair are sent —
/// so exactly one name per concept is emitted, consistently the PayPal-style one.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeAddressInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_line2: Option<Secret<String>>,
    /// City / town / village.
    #[serde(skip_serializing_if = "Option::is_none")]
    admin_area2: Option<Secret<String>>,
    /// State / province.
    #[serde(skip_serializing_if = "Option::is_none")]
    admin_area1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postal_code: Option<Secret<String>>,
    /// `scalar CountryCode` is version-sensitive: alpha-3 below `Braintree-Version 2021-02-01`,
    /// alpha-2 at or above it. This connector pins `2019-01-01`, so alpha-3 is the correct wire
    /// form. If `BRAINTREE_VERSION_VALUE` is ever raised past 2021-02-01 this must switch to
    /// `CountryAlpha2`.
    #[serde(skip_serializing_if = "Option::is_none")]
    country_code: Option<common_enums::CountryAlpha3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<BraintreePhoneInput>,
}

impl BraintreeAddressInput {
    fn is_empty(&self) -> bool {
        self.first_name.is_none()
            && self.last_name.is_none()
            && self.address_line1.is_none()
            && self.address_line2.is_none()
            && self.admin_area2.is_none()
            && self.admin_area1.is_none()
            && self.postal_code.is_none()
            && self.country_code.is_none()
            && self.phone.is_none()
    }
}

/// `PhoneInput` — both members are non-null in the schema, so the object is all-or-nothing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreePhoneInput {
    country_phone_code: String,
    phone_number: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDescriptorInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<Secret<String>>,
    /// No UCS source today — `BillingDescriptor` carries no url. Kept so the wire shape matches
    /// `TransactionDescriptorInput` (which does have `url`, ≤ 13 chars) and a future mapping is a
    /// one-line change rather than a schema change.
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionTaxInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    tax_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax_exempt: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
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

/// `TransactionLineItemType`. A sale line is always `DEBIT`; `CREDIT` lines on a sale are
/// rejected with validation code `97308`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionLineItemType {
    Debit,
}

#[derive(Debug, Clone, Serialize)]
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
    image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

/// Everything that decorates `TransactionInput` beyond `amount` / `orderId` / `merchantAccountId`.
/// Flattened into the transaction bodies because Braintree carries each of these as a direct
/// member of `TransactionInput`.
///
/// `purchaseOrderNumber` is deliberately absent: it is a real `TransactionInput` member and is
/// required for Level 2 qualification, but no UCS domain type carries a purchase-order number,
/// so there is nothing to map.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionEnrichment {
    #[serde(skip_serializing_if = "Option::is_none")]
    descriptor: Option<TransactionDescriptorInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax: Option<TransactionTaxInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    surcharge_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping: Option<TransactionShippingInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_items: Option<Vec<TransactionLineItemInput>>,
}

/// Characters Braintree permits on the Level 3 string fields (`lineItems[].name`,
/// `unitOfMeasure`, `productCode`, `commodityCode`): `a-z`, `A-Z`, `0-9`, `'`, `.`, `-` and
/// spaces. Merchant product names routinely contain `&`, `/`, `,` and accents, so sanitise first
/// and only then truncate — truncating first would leave a trailing illegal character in place.
fn sanitize_l3_text(value: &str, max_len: usize) -> String {
    let permitted = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '\'' | '.' | '-') {
                c
            } else {
                ' '
            }
        })
        .collect::<String>();
    // Dropping a character mid-word would otherwise leave a run of spaces behind.
    permitted
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_len)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// `descriptor.phone` must be 10-14 characters, of which **exactly ten are digits**, and may
/// otherwise contain only hyphens, parentheses and periods. Braintree rejects anything else with
/// validation code `92202` ("Phone must contain exactly 10 digits, and can only contain numbers,
/// dashes and parentheses") — confirmed against sandbox, which is stricter than the 10-14
/// character range the sale reference documents. A value that cannot be made to fit is omitted
/// rather than coerced: a wrong dynamic descriptor is a merchant-visible statement defect, and a
/// rejected one fails the whole authorize.
fn sanitize_descriptor_phone(value: &str) -> Option<String> {
    let cleaned = value
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '-' | '(' | ')' | '.'))
        .collect::<String>();
    let digits = cleaned.chars().filter(char::is_ascii_digit).count();
    (digits == 10 && (10..=14).contains(&cleaned.len())).then_some(cleaned)
}

/// §E.9 reconciliation, in minor units:
/// `amount = Σ lineItems.totalAmount + tax.taxAmount + shipping.shippingAmount
///           + shipping.shippingTaxAmount − discountAmount`
///
/// Braintree rejects (or silently drops to Level 1) a breakdown that does not balance, so when
/// line items are present and the sum does not match we send no L2/L3 block at all rather than an
/// unbalanced one. With no line items there is nothing to balance and the L2 fields stand alone.
fn l2_l3_breakdown_reconciles(
    amount: MinorUnit,
    line_item_totals: &[MinorUnit],
    tax_amount: Option<MinorUnit>,
    shipping_amount: Option<MinorUnit>,
    shipping_tax_amount: Option<MinorUnit>,
    discount_amount: Option<MinorUnit>,
) -> bool {
    if line_item_totals.is_empty() {
        return true;
    }
    let sum = line_item_totals
        .iter()
        .map(|total| total.get_amount_as_i64())
        .sum::<i64>()
        + tax_amount.map(MinorUnit::get_amount_as_i64).unwrap_or(0)
        + shipping_amount
            .map(MinorUnit::get_amount_as_i64)
            .unwrap_or(0)
        + shipping_tax_amount
            .map(MinorUnit::get_amount_as_i64)
            .unwrap_or(0)
        - discount_amount
            .map(MinorUnit::get_amount_as_i64)
            .unwrap_or(0);
    sum == amount.get_amount_as_i64()
}

fn build_braintree_address(
    address: Option<&domain_types::payment_address::Address>,
) -> Option<BraintreeAddressInput> {
    let address = address?;
    let details = address.address.as_ref();
    let phone = address.phone.as_ref().and_then(|phone| {
        // `PhoneInput` is all-or-nothing: both members are non-null in the schema.
        match (phone.number.clone(), phone.extract_country_code().ok()) {
            (Some(number), Some(country_phone_code)) => Some(BraintreePhoneInput {
                country_phone_code,
                phone_number: number,
            }),
            _ => None,
        }
    });
    let built = BraintreeAddressInput {
        first_name: details.and_then(|d| d.first_name.clone()),
        last_name: details.and_then(|d| d.last_name.clone()),
        address_line1: details.and_then(|d| d.line1.clone()),
        address_line2: details.and_then(|d| d.line2.clone()),
        admin_area2: details.and_then(|d| d.city.clone()),
        admin_area1: details.and_then(|d| d.state.clone()),
        postal_code: details.and_then(|d| d.zip.clone()),
        country_code: details
            .and_then(|d| d.country)
            .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
        phone,
    };
    // Braintree distinguishes "absent" from "present and empty"; never emit `{}`.
    (!built.is_empty()).then_some(built)
}

/// Builds the descriptor / L2-L3 / shipping block for a card Authorize.
///
/// Per-request fields win over the `l2_l3_data` bag (`PaymentsAuthorizeData.order_tax_amount`,
/// `.shipping_cost`, `.surcharge_amount`, `PaymentFlowData.order_details`); `l2_l3_data` is the
/// fallback. Every money field goes through the connector's `StringMajorUnit` converter —
/// Braintree wants major units, and `MinorUnit::to_string()` here would be a 100x overcharge that
/// Braintree cannot detect because `"1234"` is itself a valid `Amount`.
fn build_transaction_enrichment<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &BraintreeRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Result<TransactionEnrichment, Report<IntegrationError>> {
    let request = &item.router_data.request;
    let common = &item.router_data.resource_common_data;
    let l2_l3 = common.l2_l3_data.as_deref();
    let currency = request.currency;
    let to_major = |minor: MinorUnit| -> Result<StringMajorUnit, Report<IntegrationError>> {
        item.connector
            .amount_converter
            .convert(minor, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "failed to convert an L2/L3 amount (line item, tax, shipping, discount or \
                         surcharge) into Braintree major units"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "check that the order_details / l2_l3_data amounts and the request \
                         currency are consistent and within range"
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developer.paypal.com/braintree/docs/reference/general/level-2-and-3-processing/overview"
                            .to_string(),
                    ),
                },
            })
    };
    // Braintree treats 0 as "present and zero", which neither qualifies for L2/L3 nor conveys
    // anything; omit it instead.
    let non_zero = |minor: Option<MinorUnit>| minor.filter(|m| m.get_amount_as_i64() != 0);

    // --- dynamic descriptor (§E.6) -----------------------------------------------------------
    let descriptor = request.billing_descriptor.as_ref().and_then(|billing| {
        let name = billing.name.as_ref().map(|name| {
            // Company/DBA section is limited to 15 characters, letters and numbers only; the
            // full `<company>*<product>` form is capped at 22.
            utils::truncate_secret_string(name, 22)
        });
        let phone = billing
            .phone
            .clone()
            .and_then(|phone| sanitize_descriptor_phone(&phone.expose()))
            .map(Secret::new);
        (name.is_some() || phone.is_some()).then_some(TransactionDescriptorInput {
            name,
            phone,
            url: None,
        })
    });

    // --- amounts (§E.8) ------------------------------------------------------------------------
    let tax_amount = non_zero(
        request
            .order_tax_amount
            .or_else(|| l2_l3.and_then(|data| data.get_order_tax_amount())),
    );
    let tax_exempt = l2_l3
        .and_then(|data| data.get_tax_status())
        .map(|status| matches!(status, common_enums::TaxStatus::Exempt));
    let discount_amount = non_zero(l2_l3.and_then(|data| data.get_discount_amount()));
    let shipping_amount = non_zero(
        request
            .shipping_cost
            .or_else(|| l2_l3.and_then(|data| data.get_shipping_cost())),
    );
    let shipping_tax_amount = non_zero(l2_l3.and_then(|data| data.get_shipping_amount_tax()));
    let surcharge_amount = non_zero(request.surcharge_amount.as_ref().map(|money| money.amount));

    // --- line items (§E.7) ---------------------------------------------------------------------
    let order_details = common
        .order_details
        .clone()
        .or_else(|| l2_l3.and_then(|data| data.get_order_details()))
        .unwrap_or_default();
    let mut line_items = Vec::with_capacity(order_details.len());
    let mut line_item_totals = Vec::with_capacity(order_details.len());
    // `TransactionInput.lineItems` accepts at most 249 entries.
    for detail in order_details.iter().take(249) {
        let total = detail.total_amount.unwrap_or_else(|| {
            MinorUnit::new(detail.amount.get_amount_as_i64() * i64::from(detail.quantity))
        });
        line_item_totals.push(total);
        line_items.push(TransactionLineItemInput {
            name: sanitize_l3_text(&detail.product_name, 35),
            kind: TransactionLineItemType::Debit,
            quantity: detail.quantity.to_string(),
            unit_amount: to_major(detail.amount)?,
            total_amount: to_major(total)?,
            tax_amount: non_zero(detail.total_tax_amount)
                .map(to_major)
                .transpose()?,
            discount_amount: non_zero(detail.unit_discount_amount)
                .map(to_major)
                .transpose()?,
            unit_of_measure: detail
                .unit_of_measure
                .as_deref()
                .map(|value| sanitize_l3_text(value, 12)),
            product_code: detail
                .product_id
                .as_deref()
                .or(detail.sku.as_deref())
                .map(|value| sanitize_l3_text(value, 12)),
            commodity_code: detail
                .commodity_code
                .as_deref()
                .map(|value| sanitize_l3_text(value, 12)),
            description: detail
                .description
                .as_deref()
                .map(|value| value.chars().take(127).collect::<String>()),
            image_url: detail.product_img_link.clone(),
            url: detail.product_link.clone(),
        });
    }

    let balances = l2_l3_breakdown_reconciles(
        request.minor_amount,
        &line_item_totals,
        tax_amount,
        shipping_amount,
        shipping_tax_amount,
        discount_amount,
    );
    if !balances {
        info!(
            "BRAINTREE: L2/L3 breakdown does not reconcile with the transaction amount; omitting the L2/L3 block"
        );
    }

    // --- shipping (§E.4) -----------------------------------------------------------------------
    let shipping_address = build_braintree_address(common.get_optional_shipping());
    let ships_from_postal_code = common
        .get_optional_shipping()
        .and_then(|address| address.address.as_ref())
        .and_then(|details| details.origin_zip.clone())
        .or_else(|| l2_l3.and_then(|data| data.get_shipping_origin_zip()));
    let shipping = {
        let amount = balances.then_some(shipping_amount).flatten();
        let tax = balances.then_some(shipping_tax_amount).flatten();
        let has_any = shipping_address.is_some()
            || amount.is_some()
            || tax.is_some()
            || ships_from_postal_code.is_some();
        has_any
            .then(
                || -> Result<TransactionShippingInput, Report<IntegrationError>> {
                    Ok(TransactionShippingInput {
                        shipping_address,
                        shipping_amount: amount.map(to_major).transpose()?,
                        shipping_tax_amount: tax.map(to_major).transpose()?,
                        ships_from_postal_code,
                    })
                },
            )
            .transpose()?
    };

    let tax = balances
        .then_some(())
        .and_then(|()| {
            (tax_amount.is_some() || tax_exempt.is_some()).then_some((tax_amount, tax_exempt))
        })
        .map(
            |(amount, exempt)| -> Result<TransactionTaxInput, Report<IntegrationError>> {
                Ok(TransactionTaxInput {
                    tax_amount: amount.map(to_major).transpose()?,
                    tax_exempt: exempt,
                })
            },
        )
        .transpose()?;

    Ok(TransactionEnrichment {
        descriptor,
        tax,
        discount_amount: balances
            .then_some(discount_amount)
            .flatten()
            .map(to_major)
            .transpose()?,
        // `surchargeAmount` is not part of the reconciliation formula (it is called out
        // separately, for the Visa Rent Discount Program), so it is not gated on it.
        surcharge_amount: surcharge_amount.map(to_major).transpose()?,
        shipping,
        line_items: (balances && !line_items.is_empty()).then_some(line_items),
    })
}

/// Which vault holds the credential a Braintree MIT (`RepeatPayment`) is charging.
///
/// RULE NT-1: `options.externalVault` is emitted **if and only if** the credential is vaulted
/// OUTSIDE Braintree. The rule is encoded in this type rather than at the call sites because
/// Braintree's gateway does not enforce it — the sandbox accepts `externalVault` on a
/// Braintree-vaulted token, accepts it with no NTID, and accepts a syntactically invalid NTID,
/// returning `SUBMITTED_FOR_SETTLEMENT` in all three cases. There is therefore no API-observable
/// difference between a correctly chained NTID and a silently ignored one, and no live test can
/// catch a regime error: the split has to hold by construction. `external_vault()` returns `None`
/// for the Braintree-vaulted arm for every possible input, and that arm has no field that could
/// make it return anything else.
#[derive(Debug, Clone)]
pub enum BraintreeMitVaultRegime {
    /// Regime A — the credential is a Braintree **multi-use** (vaulted) payment method, minted by
    /// the CIT's `vaultPaymentMethodAfterTransacting` and replayed here as `connector_mandate_id`.
    /// Braintree holds the credential and replays the stored-credential chain on the merchant's
    /// behalf, so the entire MIT signal is `transaction.paymentInitiator = UNSCHEDULED` and
    /// `options.externalVault` MUST be omitted — Braintree's schema explicitly forbids it for
    /// multi-use payment methods. This is the only RepeatPayment path the connector has ever
    /// exercised, and its request body is unchanged.
    BraintreeVaulted { connector_mandate_id: String },
    /// Regime B — the credential is vaulted outside Braintree and presented as a freshly minted
    /// **single-use** token (Braintree never accepts a raw PAN on a transaction mutation, so the
    /// caller must have run `PaymentMethodService/Tokenize` first). UCS owns the
    /// stored-credential chain, so the CIT's scheme NTID is replayed as
    /// `verifyingNetworkTransactionId`.
    ExternallyVaulted {
        single_use_token: Secret<String>,
        /// Optional on purpose. Braintree accepts `{ status: VAULTED }` with no NTID, and a MIT
        /// whose mandate reference carries no NTID must degrade to "no NTID" rather than fail.
        network_transaction_id: Option<Secret<String>>,
    },
}

impl BraintreeMitVaultRegime {
    /// `input.paymentMethodId` — a Braintree vault token in Regime A, a single-use token in
    /// Regime B. Both are `ID!` on the same mutations, which is why one builder serves both.
    fn payment_method_id(&self) -> Secret<String> {
        match self {
            Self::BraintreeVaulted {
                connector_mandate_id,
            } => Secret::new(connector_mandate_id.clone()),
            Self::ExternallyVaulted {
                single_use_token, ..
            } => single_use_token.clone(),
        }
    }

    /// RULE NT-1, the whole of it. Regime A can never produce `Some`.
    fn external_vault(&self) -> Option<TransactionExternalVaultOptions> {
        match self {
            Self::BraintreeVaulted { .. } => None,
            Self::ExternallyVaulted {
                network_transaction_id,
                ..
            } => Some(TransactionExternalVaultOptions::Vaulted {
                verifying_network_transaction_id: network_transaction_id.clone(),
            }),
        }
    }
}

/// Recovers the CIT's scheme network transaction id for an externally-vaulted (Regime B) MIT.
///
/// Two carriers are checked, in order of authority:
///  1. `MandateReferenceId::NetworkMandateId` — the PSP-agnostic slot, and the one the external
///     vault regime is normally driven from.
///  2. `ConnectorMandateReferenceId.mandate_metadata`, decoded as `BraintreeMandateMetadata` —
///     the CIT -> MIT hand-off this connector writes on every card transaction response, used
///     when the caller replayed the CIT's own mandate reference.
///
/// This function never fails. A missing, empty or unparsable carrier degrades to `None`, which
/// downgrades the MIT to `{ status: VAULTED }` with no NTID — accepted by Braintree. Erroring
/// instead would turn a recoverable gap into a declined payment.
fn braintree_mit_network_transaction_id<T: PaymentMethodDataTypes>(
    request: &RepeatPaymentData<T>,
) -> Option<Secret<String>> {
    request
        .get_network_mandate_id()
        .or_else(|| braintree_ntid_from_mandate_metadata(&request.mandate_reference))
        .filter(|network_transaction_id| !network_transaction_id.is_empty())
        .map(Secret::new)
}

/// The `mandate_metadata` half of `braintree_mit_network_transaction_id`: decodes the
/// `BraintreeMandateMetadata` this connector wrote on the CIT response.
///
/// Every failure mode — a non-`ConnectorMandateId` reference, absent metadata, metadata that is
/// not a `BraintreeMandateMetadata`, or one whose `network_transaction_id` is null — yields
/// `None`. It must never surface an error: a MIT that cannot find its NTID is still a valid MIT.
fn braintree_ntid_from_mandate_metadata(
    mandate_reference: &connector_types::MandateReferenceId,
) -> Option<String> {
    match mandate_reference {
        connector_types::MandateReferenceId::ConnectorMandateId(connector_mandate_id) => {
            connector_mandate_id
                .get_mandate_metadata()
                .and_then(|metadata| {
                    serde_json::from_value::<BraintreeMandateMetadata>(metadata.expose()).ok()
                })
                .and_then(|metadata| metadata.network_transaction_id)
        }
        connector_types::MandateReferenceId::NetworkMandateId(_)
        | connector_types::MandateReferenceId::NetworkTokenWithNTI(_) => None,
    }
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
        BraintreeMitVaultRegime,
        BraintreeMeta,
    )> for MandatePaymentRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        (item, vault_regime, metadata): (
            BraintreeRouterData<
                RouterDataV2<
                    RepeatPayment,
                    PaymentFlowData,
                    RepeatPaymentData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            BraintreeMitVaultRegime,
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
        // RULE NT-1 lives entirely inside `BraintreeMitVaultRegime::external_vault()`: it is
        // `None` for every Braintree-vaulted (Regime A) request, so `options` collapses to `None`
        // below and the Regime A request body is byte-for-byte what it was before NTID support.
        let options = CreditCardTransactionOptions {
            // 3DS pass-through and the billing address belong to the CIT, never to a
            // merchant-initiated repeat — the cardholder is not present.
            three_d_secure_authentication: None,
            billing_address: None,
            external_vault: vault_regime.external_vault(),
        };
        // Braintree distinguishes "absent" from "present and empty"; never emit `options: {}`.
        let options = (!options.is_empty()).then_some(options);
        Ok(Self {
            query,
            variables: VariablePaymentInput {
                input: PaymentInput {
                    payment_method_id: vault_regime.payment_method_id(),
                    api_request_key: item
                        .router_data
                        .resource_common_data
                        .get_merchant_request_id()
                        .ok(),
                    transaction: transaction_body,
                    options,
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
            // Braintree never accepts raw PAN on a transaction mutation: `chargeCreditCard` /
            // `authorizeCreditCard` take a `paymentMethodId: ID!`. The caller must tokenize
            // first (PaymentMethodService/Tokenize, or any Braintree client-side nonce) and send
            // the result as `payment_method.token`.
            PaymentMethodData::Card(_) => Err(raw_card_not_tokenized_error()),
            PaymentMethodData::PaymentMethodToken(_) => {
                // NARROWED, not deleted. Braintree-hosted 3DS now has all three legs
                // (PreAuthenticate / Authenticate / PostAuthenticate), so a payment that ran the
                // trio arrives here carrying the hosted-3DS marker and must be charged. But
                // Braintree-hosted 3DS still cannot happen INSIDE one Authorize call, so a
                // DIRECT `PaymentService/Authorize` that never ran the trio and carries neither
                // the marker nor an external authentication must still fail closed rather than
                // silently charge unauthenticated.
                //
                // The `!is_braintree_hosted_three_ds` clause carries one real case: a
                // Braintree-hosted authentication that legitimately produced no
                // `AuthenticationData` but did produce the marker. Without it such a payment
                // would be rejected after the trio had already run to completion.
                let three_ds_mode = braintree_authorize_three_ds_mode(
                    item.router_data.resource_common_data.is_three_ds(),
                    item.router_data.request.authentication_data.as_ref(),
                    item.router_data.request.connector_feature_data.as_ref(),
                );
                if three_ds_mode == BraintreeAuthorizeThreeDsMode::Unauthenticated {
                    Err(error_stack::report!(IntegrationError::FlowNotSupported {
                        flow: "Braintree-hosted 3D Secure authorize".to_string(),
                        connector: "Braintree".to_string(),
                        context: domain_types::errors::IntegrationErrorContext {
                            additional_context: Some(
                                "A 3D Secure payment reached Authorize with neither a completed \
                                 Braintree-hosted authentication nor an external one. Braintree \
                                 cannot perform 3D Secure inside a single Authorize call: the \
                                 hosted flow runs as PreAuthenticate -> Authenticate -> \
                                 PostAuthenticate before the charge."
                                    .to_string(),
                            ),
                            suggested_action: Some(
                                "Route this payment through CompositeAuthorize so the three \
                                 authentication legs run and the verified nonce reaches \
                                 Authorize, or perform 3D Secure with an external MPI and send \
                                 the result in `authentication_data`."
                                    .to_string(),
                            ),
                            doc_url: Some(
                                "https://developer.paypal.com/braintree/docs/guides/3d-secure/overview"
                                    .to_string(),
                            ),
                        },
                    }))
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

/// A raw `Card` reached a builder that can only send a `paymentMethodId`. This is a caller
/// sequencing problem, not a missing field on the request, so it must not surface as
/// `MissingRequiredField("payment_method_token")` — the caller did send a card.
fn raw_card_not_tokenized_error() -> Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: "raw card data on Braintree Authorize".to_string(),
        connector: "Braintree",
        context: domain_types::errors::IntegrationErrorContext {
            additional_context: Some(
                "Braintree's chargeCreditCard / authorizeCreditCard mutations take a \
                 `paymentMethodId`, so a card must be exchanged for a token before it can be \
                 authorized."
                    .to_string(),
            ),
            suggested_action: Some(
                "Call PaymentMethodService/Tokenize with the card first, then send the returned \
                 token on Authorize as `payment_method.token`."
                    .to_string(),
            ),
            doc_url: Some(
                "https://graphql.braintreepayments.com/guides/making_api_calls/#tokenizing-a-payment-method"
                    .to_string(),
            ),
        },
    })
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAuthChargeResponseBody {
    id: String,
    status: BraintreePaymentStatus,
    payment_method: Option<PaymentMethodInfo>,
    /// Authorization-time processor result. Replaces the deprecated `Transaction.processorResponse`
    /// and is the only place AVS / CVV check results live — they are decided at authorization
    /// time, never at capture.
    #[serde(default)]
    processor_authorization_response: Option<TransactionProcessorResponse>,
    /// Settlement-time (4000-class) processor result. Null until settlement is attempted.
    #[serde(default)]
    processor_settlement_response: Option<TransactionSettlementProcessorResponse>,
    /// Reverse-chronological, most recent event first. Decline type, Mastercard merchant advice
    /// code, raw network response and gateway-rejection reason are reachable ONLY through these
    /// typed events — none of them hangs off `Transaction` directly.
    #[serde(default)]
    status_history: Option<Vec<BraintreeStatusEvent>>,
    /// `Transaction.paymentMethodSnapshot` — a GraphQL UNION (`PaymentMethodSnapshot`) with nine
    /// members. Only `CreditCardTransactionDetails` carries `networkTransactionId`; every other
    /// member (`CreditCardDetails`, `PayPalTransactionDetails`, ...) must deserialize to an empty
    /// object rather than fail, which is why the selection set asks for exactly one inline
    /// fragment and both levels here are optional.
    ///
    /// `Transaction.networkTransactionId` does NOT exist at the pinned version: selecting it is a
    /// document-level GraphQL validation error that would reject every Authorize before
    /// execution. The snapshot union is the only path to the value.
    #[serde(default)]
    payment_method_snapshot: Option<CreditCardTransactionSnapshot>,
}

/// `CreditCardTransactionDetails` — the one `PaymentMethodSnapshot` union member that carries the
/// scheme network transaction id (NTID).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardTransactionSnapshot {
    /// Nullable even on `CreditCardTransactionDetails`. Braintree does not document the format
    /// (observed as a 15-digit numeric string); it is opaque and must never be parsed, padded or
    /// validated.
    #[serde(default)]
    pub network_transaction_id: Option<String>,
}

impl TransactionAuthChargeResponseBody {
    /// `paymentMethodSnapshot` -> `... on CreditCardTransactionDetails` -> `networkTransactionId`.
    ///
    /// Yields `None` whenever the snapshot resolved to a non-card union member (the inline
    /// fragment then matches nothing and the object arrives empty). Deliberately NOT gated on the
    /// transaction status: the NTID is assigned at authorization time and is present on
    /// `PROCESSOR_DECLINED` as well as on the success states.
    fn network_transaction_id(&self) -> Option<String> {
        self.payment_method_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.network_transaction_id.clone())
    }
}

/// Braintree CIT -> MIT hand-off.
///
/// `MandateReferenceId` is an enum, so a Braintree MIT — which may need the Braintree vault token
/// AND the scheme network transaction id at the same time — cannot obtain both from it:
/// `ConnectorMandateId` carries the token and `NetworkMandateId` carries the NTID, never both.
/// The NTID therefore rides in `ConnectorMandateReferenceId.mandate_metadata` alongside
/// `connector_mandate_id`. Precedent in this repo: `PaysafeMandateMetadata`
/// (`paysafe/transformers.rs`).
///
/// This value is advisory. A MIT that arrives without it (or with an unparsable one) MUST
/// degrade to "no NTID" and MUST NOT error — the Braintree-vaulted MIT (Regime A, see RULE NT-1)
/// never puts an NTID on the wire anyway, so its absence is a no-op there.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BraintreeMandateMetadata {
    pub network_transaction_id: Option<String>,
}

/// Builds the `mandate_metadata` payload handed back on a CIT so a later MIT can see the NTID.
///
/// Returns `None` when there is no NTID to carry, so the wire/DB shape is unchanged for every
/// response that has no snapshot (wallets, `tokenizeCreditCard`), and never fails: a
/// serialization error degrades to `None` rather than turning a successful payment into an error.
fn build_braintree_mandate_metadata(
    network_transaction_id: Option<String>,
) -> Option<pii::SecretSerdeValue> {
    let network_transaction_id = network_transaction_id?;
    serde_json::to_value(BraintreeMandateMetadata {
        network_transaction_id: Some(network_transaction_id),
    })
    .ok()
    .map(Secret::new)
}

/// `AvsCvvResponseCode` — one enum shared by `cvvResponse`, `avsPostalCodeResponse` and
/// `avsStreetAddressResponse`. `Unknown` keeps an unrecognised value from failing the whole
/// response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum AvsCvvResponseCode {
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

impl AvsCvvResponseCode {
    /// The single-letter REST/server-SDK representation, which is what downstream AVS/CVV
    /// consumers expect. `Unknown` has no letter.
    pub fn as_legacy_code(self) -> Option<&'static str> {
        match self {
            Self::Matches => Some("M"),
            Self::DoesNotMatch => Some("N"),
            Self::NotVerified => Some("U"),
            Self::NotProvided => Some("I"),
            Self::IssuerDoesNotParticipate => Some("S"),
            Self::SystemError => Some("E"),
            Self::NotApplicable => Some("A"),
            Self::Bypass => Some("B"),
            Self::Unknown => None,
        }
    }
}

/// `TransactionAuthorizationProcessorResponse`. Note the GraphQL names: `legacyCode` (not
/// `processorResponseCode`), `message` (not `processorResponseText`) and no `Code` suffix on the
/// three AVS/CVV members.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionProcessorResponse {
    pub legacy_code: Option<String>,
    pub message: Option<String>,
    pub cvv_response: Option<AvsCvvResponseCode>,
    pub avs_postal_code_response: Option<AvsCvvResponseCode>,
    pub avs_street_address_response: Option<AvsCvvResponseCode>,
    #[serde(default)]
    pub authorization_id: Option<String>,
    #[serde(default)]
    pub additional_information: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSettlementProcessorResponse {
    pub legacy_code: Option<String>,
    pub message: Option<String>,
}

/// `MerchantAdviceCodeResponse` / `PaymentNetworkResponse` share this shape. Both `code`s are
/// bare nullable `String`s in the schema — Braintree constrains neither, and a network response
/// code is only interpretable together with the card brand, so they are carried verbatim and
/// never mapped onto a UCS enum.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeCodeMessage {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// `ProcessorDeclineType` — the authoritative "is this decline retryable?" discriminator. It is
/// on the status event, not on the processor-response object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessorDeclineType {
    Hard,
    Soft,
    #[serde(other)]
    Unknown,
}

/// `GatewayRejectionReason` — 14 members in the SDL, five of which appear on no prose page.
/// A gateway rejection is not a decline: it is blocked by the merchant's own gateway settings,
/// and if the transaction had already authorized the gateway has *already* voided it, so it must
/// never be followed by a Void.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum GatewayRejectionReason {
    ApplicationIncomplete,
    Avs,
    AvsAndCvv,
    Cvv,
    Duplicate,
    ExcessiveRetry,
    Fraud,
    ManualTransactionsDisabled,
    PaymentMethodBlocked,
    RiskThreshold,
    ThreeDSecure,
    TokenIssuance,
    TooManyConfirmationAttempts,
    UnionPayEnrollmentRequired,
    #[serde(other)]
    Unknown,
}

/// One entry of `Transaction.statusHistory`. Only the members the inline fragments in the
/// mutation actually select are modelled; every one of them is absent on event types that do not
/// carry it, hence all-optional.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeStatusEvent {
    #[serde(default)]
    pub terminal: Option<bool>,
    #[serde(default)]
    pub decline_type: Option<ProcessorDeclineType>,
    #[serde(default)]
    pub gateway_rejection_reason: Option<GatewayRejectionReason>,
    #[serde(default)]
    pub processor_response: Option<TransactionProcessorResponse>,
    #[serde(default)]
    pub network_response: Option<BraintreeCodeMessage>,
    #[serde(default)]
    pub merchant_advice_code_response: Option<BraintreeCodeMessage>,
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
                let connector_response = build_card_connector_response(&transaction_data);
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_transaction_failure_error_response(
                        &transaction_data,
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
                                // The CIT -> MIT hand-off of the scheme NTID. The vault token
                                // and the NTID cannot both travel in `MandateReferenceId`, so
                                // the NTID rides here. See `BraintreeMandateMetadata`.
                                mandate_metadata: build_braintree_mandate_metadata(
                                    transaction_data.network_transaction_id(),
                                ),
                            })
                        }),
                        connector_metadata: None,
                        // Read off `paymentMethodSnapshot { ... on CreditCardTransactionDetails
                        // { networkTransactionId } }`. `network_txn_link_id` stays `None`: that
                        // slot is the Mastercard Transaction Link Identifier, a different
                        // identifier that Braintree does not expose at the pinned version.
                        network_txn_id: transaction_data.network_transaction_id(),
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
                            client_token_data
                                .data
                                .create_client_token
                                .client_token
                                .clone(),
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

/// Builds the `ErrorResponse` for a card transaction Braintree declined or rejected on a 200.
///
/// Braintree reports declines (`PROCESSOR_DECLINED` / `FAILED`) and gateway rejections
/// (`GATEWAY_REJECTED`) as a *successful* GraphQL response with no `errors[]` entry, so the
/// detail has to be dug out of `processorAuthorizationResponse` and the typed `statusHistory`
/// events. Populating `network_advice_code` / `network_decline_code` / `network_error_message`
/// here is what lets a smart-retry layer key off the Mastercard merchant advice code and the raw
/// network response instead of guessing from the gateway status.
fn create_transaction_failure_error_response(
    transaction: &TransactionAuthChargeResponseBody,
    http_code: u16,
) -> domain_types::router_data::ErrorResponse {
    let status_string = transaction.status.to_string();
    // `statusHistory` is reverse-chronological, so the first event carrying decline detail is the
    // one that produced the current status.
    let event = transaction.status_history.as_ref().and_then(|events| {
        events.iter().find(|event| {
            event.decline_type.is_some()
                || event.gateway_rejection_reason.is_some()
                || event.merchant_advice_code_response.is_some()
                || event.network_response.is_some()
        })
    });
    let processor = transaction
        .processor_authorization_response
        .as_ref()
        .or_else(|| event.and_then(|event| event.processor_response.as_ref()));
    let merchant_advice = event.and_then(|event| event.merchant_advice_code_response.as_ref());
    let network = event.and_then(|event| event.network_response.as_ref());

    let code = processor
        .and_then(|processor| processor.legacy_code.clone())
        .or_else(|| {
            event
                .and_then(|event| event.gateway_rejection_reason)
                .map(|reason| reason.to_string())
        })
        .unwrap_or_else(|| status_string.clone());
    let message = processor
        .and_then(|processor| processor.message.clone())
        .or_else(|| {
            event
                .and_then(|event| event.gateway_rejection_reason)
                .map(|reason| reason.to_string())
        })
        .unwrap_or_else(|| status_string.clone());
    let mut reason_parts = vec![status_string];
    if let Some(decline_type) = event.and_then(|event| event.decline_type) {
        reason_parts.push(format!("decline_type={decline_type}"));
    }
    if let Some(reason) = event.and_then(|event| event.gateway_rejection_reason) {
        reason_parts.push(format!("gateway_rejection_reason={reason}"));
    }
    if let Some(info) = processor.and_then(|processor| processor.additional_information.clone()) {
        reason_parts.push(info);
    }
    if let Some(advice) = merchant_advice.and_then(|advice| advice.message.clone()) {
        reason_parts.push(advice);
    }

    domain_types::router_data::ErrorResponse {
        code,
        message,
        reason: Some(reason_parts.join("; ")),
        // Left `None` deliberately: the caller sets `PaymentFlowData.status` from the Braintree
        // status map on this same path, so the terminal state is already reported there.
        attempt_status: None,
        connector_transaction_id: Some(transaction.id.clone()),
        status_code: http_code,
        network_advice_code: merchant_advice.and_then(|advice| advice.code.clone()),
        network_decline_code: network.and_then(|network| network.code.clone()),
        network_error_message: network.and_then(|network| network.message.clone()),
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Surfaces the AVS / CVV check results and the acquirer authorization code on the success path.
/// `PaymentsResponseData::TransactionResponse` has no slot for them, so they ride on
/// `PaymentFlowData.connector_response`, which is where every other connector puts payment checks.
fn build_card_connector_response(
    transaction: &TransactionAuthChargeResponseBody,
) -> Option<domain_types::router_data::ConnectorResponseData> {
    let processor = transaction.processor_authorization_response.as_ref()?;
    let letter =
        |code: Option<AvsCvvResponseCode>| code.and_then(AvsCvvResponseCode::as_legacy_code);
    let payment_checks = Some(serde_json::json!({
        "avs_street_address_response_code": letter(processor.avs_street_address_response),
        "avs_postal_code_response_code": letter(processor.avs_postal_code_response),
        "cvv_response_code": letter(processor.cvv_response),
        "avs_street_address_response": processor.avs_street_address_response.map(|code| code.to_string()),
        "avs_postal_code_response": processor.avs_postal_code_response.map(|code| code.to_string()),
        "cvv_response": processor.cvv_response.map(|code| code.to_string()),
        "processor_response_code": processor.legacy_code,
        "processor_response_text": processor.message,
    }));
    Some(
        domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks,
                card_network: None,
                domestic_network: None,
                auth_code: processor.authorization_id.clone(),
            },
        ),
    )
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
    /// Braintree adds transaction statuses over time. Deserializing an unrecognised one into a
    /// catch-all keeps a single new status from failing the whole response parse; it maps to
    /// `AttemptStatus::Unspecified` so the caller applies its own previous-status fallback
    /// rather than UCS inventing a Pending or a Failure it cannot substantiate.
    #[serde(other)]
    Unknown,
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
            BraintreePaymentStatus::Unknown => Self::Unspecified,
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
                let connector_response = build_card_connector_response(&transaction_data);
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_transaction_failure_error_response(
                        &transaction_data,
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
                                // The CIT -> MIT hand-off of the scheme NTID. The vault token
                                // and the NTID cannot both travel in `MandateReferenceId`, so
                                // the NTID rides here. See `BraintreeMandateMetadata`.
                                mandate_metadata: build_braintree_mandate_metadata(
                                    transaction_data.network_transaction_id(),
                                ),
                            })
                        }),
                        connector_metadata: None,
                        // Read off `paymentMethodSnapshot { ... on CreditCardTransactionDetails
                        // { networkTransactionId } }`. `network_txn_link_id` stays `None`: that
                        // slot is the Mastercard Transaction Link Identifier, a different
                        // identifier that Braintree does not expose at the pinned version.
                        network_txn_id: transaction_data.network_transaction_id(),
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
                            client_token_data
                                .data
                                .create_client_token
                                .client_token
                                .clone(),
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
        if let Some(currency) = rsync_currency(
            item.router_data.request.refund_money,
            &item.router_data.request.refund_connector_metadata,
        ) {
            validate_currency(currency, Some(metadata.merchant_config_currency))?;
        }
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
    /// Taken from the card itself, never from the billing name: the two are distinct fields and
    /// conflating them misreports the cardholder to the issuer. Omitted when absent rather than
    /// sent as an empty string, which Braintree would store verbatim.
    #[serde(skip_serializing_if = "Option::is_none")]
    cardholder_name: Option<Secret<String>>,
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
                            cardholder_name: card_data.card_holder_name.clone(),
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
        // Braintree's chargeCreditCard / authorizeCreditCard mutations take a `paymentMethodId`,
        // never raw card data, so the only payment-method shape this builder can serve is a
        // token obtained from PaymentMethodService/Tokenize (or any Braintree single-use nonce).
        let payment_method_id = match &item.router_data.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(token) => token.token.clone(),
            _ => return Err(raw_card_not_tokenized_error()),
        };

        // External (merchant-performed / MPI) 3DS pass-through. `passThrough` is only legal on
        // `options`, which only the credit-card mutations have — and only when an ECI is present.
        //
        // It is sent ONLY for an externally performed authentication. A Braintree-HOSTED one
        // (legs PreAuthenticate / Authenticate / PostAuthenticate) reaches this builder carrying
        // `authentication_data` too, because the composite dispatcher copies PostAuthenticate's
        // into the Authorize request — but Braintree already holds that authentication ON the
        // payment method and attaches it to the transaction by itself. Re-declaring it through
        // `threeDSecurePassThru` would assert an externally performed authentication for one
        // Braintree performed, which the two topologies forbid.
        let three_ds_data = (braintree_authorize_three_ds_mode(
            item.router_data.resource_common_data.is_three_ds(),
            item.router_data.request.authentication_data.as_ref(),
            item.router_data.request.connector_feature_data.as_ref(),
        ) == BraintreeAuthorizeThreeDsMode::ExternalPassThrough)
            .then_some(item.router_data.request.authentication_data.as_ref())
            .flatten()
            .and_then(convert_external_three_ds_data)
            .map(|pass_through| ThreeDSecureAuthenticationInput {
                pass_through: Some(pass_through),
            });

        let options = CreditCardTransactionOptions {
            three_d_secure_authentication: three_ds_data,
            billing_address: build_braintree_address(
                item.router_data
                    .resource_common_data
                    .get_optional_payment_billing()
                    .or_else(|| item.router_data.resource_common_data.get_optional_billing()),
            ),
            // Authorize is never an external-vault MIT: the external-vault regime only exists on
            // RepeatPayment, where `BraintreeMitVaultRegime` decides it. See RULE NT-1.
            external_vault: None,
        };
        // Braintree distinguishes "absent" from "present and empty"; never emit `options: {}`.
        let options = (!options.is_empty()).then_some(options);

        // The merchant-facing reference. `merchant_order_id` is the merchant's own order id when
        // the caller supplied one; otherwise fall back to the request reference id, which is what
        // this connector has always sent. PSync anchors on the Braintree transaction id (not on
        // `orderId`), and Authorize returns that same id, so the two flows stay consistent.
        let order_id = item
            .router_data
            .request
            .merchant_order_id
            .clone()
            .or_else(|| {
                item.router_data
                    .resource_common_data
                    .l2_l3_data
                    .as_deref()
                    .and_then(|data| data.get_merchant_order_reference_id())
            })
            .unwrap_or_else(|| {
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });
        let enrichment = build_transaction_enrichment(&item)?;
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
                    payment_initiator: PaymentInitiatorType::RecurringFirst,
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
                    payment_method_id,
                    api_request_key: item
                        .router_data
                        .resource_common_data
                        .get_merchant_request_id()
                        .ok(),
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
    client_token: Secret<String>,
    payment_method_token: Secret<String>,
    card_details: PaymentMethodData<T>,
    complete_authorize_url: String,
) -> Result<RedirectForm, Report<ConnectorError>> {
    Ok(RedirectForm::Braintree {
        client_token: client_token.expose(),
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

/// Resolves the currency RSync validates against, in precedence order.
///
/// RSync searches by refund id and sends neither an amount nor a currency to Braintree,
/// so this guard can only ever reject a request that would otherwise have succeeded.
/// It reads the first-class `refund_money` the caller supplies
/// (`RefundServiceGetRequest.refund_amount`) first, falls back to the optional metadata
/// blob for callers that still put `currency` there, and returns `None` when neither is
/// present so the caller skips the guard rather than failing the sync.
///
/// Requiring the metadata copy made RSync unreachable for any caller that did not
/// hand-populate it — Hyperswitch does not — so a settled refund could never be read
/// back. An unvalidated sync is strictly better than an unsyncable one.
fn rsync_currency(
    refund_money: Option<common_utils::types::Money>,
    refund_connector_metadata: &Option<pii::SecretSerdeValue>,
) -> Option<enums::Currency> {
    refund_money
        .map(|money| money.currency)
        .or_else(|| extract_metadata_field(refund_connector_metadata, "currency").ok())
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

// ---------------------------------------------------------------------------
// Incoming webhooks (§W of the Braintree technical specification)
//
// Braintree POSTs `application/x-www-form-urlencoded` with exactly two fields,
// `bt_signature` and `bt_payload`; `bt_payload` is line-wrapped base64 of the
// notification XML. EVERY notification, for EVERY kind, nests its entity one level
// down, under `<subject>`:
//
//   <notification>
//     <timestamp type="datetime"/><kind/><source-merchant-id/>?
//     <subject><dispute>..</dispute> | <transaction>..</transaction></subject>
//   </notification>
//
// The un-wrapped form does not exist. Element names are kebab-case on the wire; the
// snake_case spellings that appear throughout Braintree's own SDK source are a
// post-parse rewrite (`braintree_python/.../xml_util.py` `__underscored`,
// `braintree_ruby/.../xml/parser.rb` `tr("-", "_")`), never the wire form.
//
// None of these structs may carry `deny_unknown_fields`: Braintree annotates typed
// elements (`type="datetime"`, `type="array"`, `nil="true"`), and quick-xml surfaces
// attributes as `@<attr>` fields.
// ---------------------------------------------------------------------------

/// Braintree emits absent optional elements as `<foo nil="true"/>`. quick-xml deserializes
/// an empty element as an empty *value*, not as a missing field, so a typed target
/// (`Currency`, an amount) sees `""` and fails — and a failed parse fails the WHOLE
/// notification, not just the field. That is a lost dispute notification with a reply-by
/// deadline attached, so every typed optional goes through this helper: empty or
/// whitespace-only content becomes `None` before the real deserializer ever runs.
fn deserialize_nil_aware<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        // Routed through `serde_json::Value::String` rather than serde's own `StrDeserializer`:
        // the latter forwards `deserialize_newtype_struct` to `visit_str`, which a newtype
        // amount type such as `StringMajorUnit` rejects ("expected tuple struct").
        Some(value) => T::deserialize(serde_json::Value::String(value.to_string()))
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Deserialize)]
pub struct BraintreeWebhookResponse {
    pub bt_signature: String,
    pub bt_payload: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Notification {
    /// The notification kind. Deliberately `String`, not an enum: Braintree adds kinds over
    /// time and an unrecognised kind must degrade to
    /// `EventType::IncomingWebhookEventUnspecified`, never fail the parse.
    pub kind: String,
    pub timestamp: String,
    /// Present when the webhook was delivered to a partner/marketplace parent account on
    /// behalf of a sub-merchant. Not used for routing today; captured so the raw resource
    /// object surfaced by `get_webhook_resource_object` is complete.
    #[serde(default)]
    pub source_merchant_id: Option<String>,
    /// The entity wrapper. Optional only so that a kind whose subject UCS does not model
    /// (subscription, disbursement, partner-merchant) still parses and reaches the
    /// `IncomingWebhookEventUnspecified` arm instead of `WebhookBodyDecodingFailed`.
    #[serde(default)]
    pub subject: Option<NotificationSubject>,
}

impl Notification {
    /// The `<subject><dispute>` entity, if this notification carries one.
    pub(super) fn dispute(&self) -> Option<&BraintreeDisputeData> {
        self.subject.as_ref().and_then(|s| s.dispute.as_ref())
    }

    /// The `<subject><transaction>` entity, if this notification carries one. Shared by the
    /// payment kinds and by `refund_failed` — Braintree uses one element for both.
    pub(super) fn transaction(&self) -> Option<&BraintreeWebhookTransaction> {
        self.subject.as_ref().and_then(|s| s.transaction.as_ref())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NotificationSubject {
    #[serde(default)]
    pub dispute: Option<BraintreeDisputeData>,
    /// Carries BOTH the payment-lifecycle entity (`transaction_settled`,
    /// `transaction_settlement_declined`) and the refund entity (`refund_failed`).
    /// Braintree uses one `<transaction>` element for both; the refund reading is a
    /// projection of this same struct, not a second wire type.
    #[serde(default)]
    pub transaction: Option<BraintreeWebhookTransaction>,
}

/// The `<transaction>` entity: the subject of `transaction_settled`,
/// `transaction_settlement_declined` and `refund_failed`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BraintreeWebhookTransaction {
    /// For a payment kind: the sale's own id. For `refund_failed`: the REFUND's own id
    /// (NOT the parent sale — that is `refunded_transaction_id`).
    pub id: String,
    pub status: BraintreeWebhookTransactionStatus,
    /// Decimal MAJOR units on the wire, e.g. `10.00`.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub amount: Option<StringMajorUnit>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub currency_iso_code: Option<enums::Currency>,
    /// The merchant-assigned reference UCS sent on the Authorize leg.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub order_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub merchant_account_id: Option<String>,
    /// `credit_card`, `us_bank_account`, `paypal_account`, `apple_pay_card`, … . The
    /// discriminator that proves a `transaction_settled` payload is an ACH/SEPA event and
    /// not a card one.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub payment_instrument_type: Option<String>,
    /// `refund_failed` only: the id of the SALE this refund was taken against. This is the
    /// payment-lookup key. Braintree's Ruby sample emits `<refunded-transaction-id>` and both
    /// the Python and Ruby `Transaction` models declare `refunded_transaction_id`.
    ///
    /// `refunded-transaction-fk` is a stale artifact present only in the Python/Node/PHP/Java
    /// *sample generators*; no SDK ever parses it. It is accepted here as an alias purely so a
    /// payload produced by one of those generators in a test harness still parses. Production
    /// logic must never be keyed on the `-fk` spelling.
    #[serde(
        default,
        alias = "refunded-transaction-fk",
        deserialize_with = "deserialize_nil_aware"
    )]
    pub refunded_transaction_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub processor_response_code: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub processor_response_text: Option<String>,
    /// `Option<String>` rather than a datetime: these are informational only, and a
    /// `nil="true"` element must not be able to fail the parse.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub updated_at: Option<String>,
}

impl BraintreeWebhookTransaction {
    /// `refund_failed`: this entity's own id IS the refund id.
    pub(super) fn connector_refund_id(&self) -> String {
        self.id.clone()
    }

    /// `refund_failed`: the parent sale. `None` is possible on a malformed payload and must
    /// degrade to a `None` reference slot, never to reusing `self.id` — putting the refund id
    /// in the payment slot makes the caller's lookup hit the wrong row.
    pub(super) fn parent_transaction_id(&self) -> Option<String> {
        self.refunded_transaction_id.clone()
    }
}

/// Braintree transaction status as it appears in webhook XML: the REST spelling, lowercase
/// snake_case (`<status>processor_declined</status>`).
///
/// This CANNOT be `BraintreePaymentStatus`, which is `SCREAMING_SNAKE_CASE` because it
/// deserializes the GraphQL response. Feeding webhook XML into that enum sends every real
/// status into its `#[serde(other)] Unknown` arm — a silent, total misreading that no test on
/// the GraphQL path can catch. Same value set, different encoding, so the two are bridged by
/// the `From` impl below rather than duplicating the `-> AttemptStatus` mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BraintreeWebhookTransactionStatus {
    Authorized,
    Authorizing,
    /// GraphQL spells this `AUTHORIZED_EXPIRED`; the REST/webhook surface spells it
    /// `authorization_expired`. Both accepted.
    #[serde(alias = "authorized_expired")]
    AuthorizationExpired,
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
    /// Braintree adds statuses over time. An unrecognised one must not fail the parse; it
    /// maps to `AttemptStatus::Unspecified` so the caller applies its own previous-status
    /// fallback rather than UCS inventing a Pending or a Failure it cannot substantiate.
    #[serde(other)]
    Unknown,
}

/// Bridge to the GraphQL-side enum so there is exactly ONE `-> enums::AttemptStatus` mapping
/// for Braintree and the webhook path cannot drift from the PSync path. Both matches are
/// exhaustive with no `_` arm, so adding a variant to either enum is a compile error rather
/// than a silent mismapping.
impl From<BraintreeWebhookTransactionStatus> for BraintreePaymentStatus {
    fn from(item: BraintreeWebhookTransactionStatus) -> Self {
        match item {
            BraintreeWebhookTransactionStatus::Authorized => Self::Authorized,
            BraintreeWebhookTransactionStatus::Authorizing => Self::Authorizing,
            BraintreeWebhookTransactionStatus::AuthorizationExpired => Self::AuthorizedExpired,
            BraintreeWebhookTransactionStatus::Failed => Self::Failed,
            BraintreeWebhookTransactionStatus::ProcessorDeclined => Self::ProcessorDeclined,
            BraintreeWebhookTransactionStatus::GatewayRejected => Self::GatewayRejected,
            BraintreeWebhookTransactionStatus::Voided => Self::Voided,
            BraintreeWebhookTransactionStatus::Settling => Self::Settling,
            BraintreeWebhookTransactionStatus::Settled => Self::Settled,
            BraintreeWebhookTransactionStatus::SettlementPending => Self::SettlementPending,
            BraintreeWebhookTransactionStatus::SettlementDeclined => Self::SettlementDeclined,
            BraintreeWebhookTransactionStatus::SettlementConfirmed => Self::SettlementConfirmed,
            BraintreeWebhookTransactionStatus::SubmittedForSettlement => {
                Self::SubmittedForSettlement
            }
            BraintreeWebhookTransactionStatus::Unknown => Self::Unknown,
        }
    }
}

/// Maps the webhook transaction status onto a refund status. `refund_failed` is the only
/// refund kind Braintree emits, so only the failure arm is reachable today; the success arm
/// is written out so that if Braintree ever adds a refund-success kind the mapping is already
/// correct and the compiler enforces completeness. Exhaustive, no `_` arm.
///
/// `RefundStatus::Unknown` — not `Pending` — is the honest target for a status that says
/// nothing about the refund: UCS must not invent a Pending it cannot substantiate.
impl From<BraintreeWebhookTransactionStatus> for enums::RefundStatus {
    fn from(item: BraintreeWebhookTransactionStatus) -> Self {
        match item {
            BraintreeWebhookTransactionStatus::ProcessorDeclined
            | BraintreeWebhookTransactionStatus::GatewayRejected
            | BraintreeWebhookTransactionStatus::Failed
            | BraintreeWebhookTransactionStatus::SettlementDeclined => Self::Failure,
            BraintreeWebhookTransactionStatus::Settled
            | BraintreeWebhookTransactionStatus::Settling
            | BraintreeWebhookTransactionStatus::SettlementConfirmed
            | BraintreeWebhookTransactionStatus::SubmittedForSettlement
            | BraintreeWebhookTransactionStatus::SettlementPending => Self::Success,
            BraintreeWebhookTransactionStatus::Authorized
            | BraintreeWebhookTransactionStatus::Authorizing
            | BraintreeWebhookTransactionStatus::AuthorizationExpired
            | BraintreeWebhookTransactionStatus::Voided
            | BraintreeWebhookTransactionStatus::Unknown => Self::Unknown,
        }
    }
}

/// The `<subject><dispute>` entity.
///
/// `rename_all = "kebab-case"` is load-bearing: without it every multi-word field is looked
/// up under a name the wire never uses (`amount_disputed` vs `amount-disputed`) and the
/// struct cannot deserialize a single real dispute payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BraintreeDisputeData {
    /// Braintree's dispute id. Non-optional: every dispute payload carries it, and without it
    /// there is no dispute to report.
    pub id: String,
    /// Decimal MAJOR units on the wire, e.g. `10.00` — NOT minor units.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub amount_disputed: Option<StringMajorUnit>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub amount_won: Option<StringMajorUnit>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub currency_iso_code: Option<enums::Currency>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub case_number: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub chargeback_protection_level: Option<String>,
    /// The dispute STAGE — `chargeback` | `pre_arbitration` | `retrieval`. Distinct from the
    /// notification `kind`, and case-variable on the wire.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub kind: Option<String>,
    /// `open` | `won` | `lost` | `accepted` | `expired` | `disputed` | `under_review`.
    /// Informational only: UCS derives `DisputeStatus` from the notification kind, not from
    /// here.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reason: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reason_code: Option<String>,
    /// Set when this dispute is a pre-arbitration escalation of an earlier one.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub original_dispute_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub merchant_account_id: Option<String>,
    /// `Option<String>`, not `Option<PrimitiveDateTime>`: Braintree emits these as
    /// `nil="true"` empty elements when unset, which a datetime deserializer cannot survive.
    /// Parse downstream if a typed value is ever needed.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub updated_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reply_by_date: Option<String>,
    /// Not consumed by any mapping; it exists for the raw resource object. Braintree emits
    /// this as a `type="array"` wrapper on some payloads — an unrecognised nested element is
    /// simply ignored (no `deny_unknown_fields` anywhere in this module), so the array form
    /// degrades to all-`None` rather than failing the notification.
    #[serde(default)]
    pub evidence: Option<DisputeEvidence>,
    /// Non-optional: the dispute cannot be attributed to a payment without it, and
    /// `get_webhook_reference` uses `transaction.id` as the payment-lookup key.
    pub transaction: DisputeTransaction,
    /// `type="array"` on the wire. Not consumed today; declared so it is visible in the raw
    /// resource object and so a future pass does not have to rediscover the element name.
    #[serde(default)]
    pub status_history: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DisputeTransaction {
    /// The disputed SALE's id. This is the payment-lookup key for the dispute reference.
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub amount: Option<StringMajorUnit>,
    /// The merchant-assigned reference from the original Authorize. Populates
    /// `DisputeWebhookDetailsResponse::connector_response_reference_id`.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub order_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub payment_instrument_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub merchant_account_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub created_at: Option<String>,
}

/// One `<evidence>` record. Every field is optional: `comment` and `url` are routinely
/// `nil="true"`, and `url::Url` cannot parse the `""` that a nil element yields — which
/// would fail the whole notification. Kept for the raw resource object; no mapping reads it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DisputeEvidence {
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub id: Option<Secret<String>>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub comment: Option<String>,
    /// `Option<String>`, not `url::Url`: a `nil="true"` element yields `""`, which is not a
    /// valid URL and would fail the whole notification parse.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub category: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub sequence_number: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub sent_to_processor_at: Option<String>,
}

// Maps the Braintree notification `kind` to the prism webhook event type.
//
// A `&str` match with a `_` arm, deliberately, rather than a `#[serde]` enum on
// `Notification::kind`: an unknown kind must REACH the fallback arm, not fail
// deserialization.
//
// `process_webhook_event` fans out to process_payment/refund/dispute_webhook purely from
// `EventType::is_payment_event()` / `is_refund_event()` / `is_dispute_event()`. Anything
// matching none of the three — `IncomingWebhookEventUnspecified` included — falls through to
// the PAYMENT handler, not to an error, so a wrongly classed kind produces a wrong-shaped
// response rather than a failure.
pub(super) fn get_status(status: &str) -> connector_types::EventType {
    match status {
        // --- dispute family: the 8 kinds Braintree emits, all reachable on a Card run ---
        "dispute_opened" => connector_types::EventType::DisputeOpened,
        "dispute_accepted" | "dispute_auto_accepted" => connector_types::EventType::DisputeAccepted,
        // `dispute_under_review` fires after evidence is submitted while the issuer reviews
        // it. UCS has no "under review" variant; `DisputeChallenged` is exactly "evidence
        // submitted, outcome pending", keeps the dispute non-terminal and is idempotent with
        // the `dispute_disputed` that preceded it. Leaving it unmapped would send a dispute
        // payload to `process_payment_webhook` via the misc fall-through.
        "dispute_disputed" | "dispute_under_review" => {
            connector_types::EventType::DisputeChallenged
        }
        "dispute_expired" => connector_types::EventType::DisputeExpired,
        "dispute_won" => connector_types::EventType::DisputeWon,
        "dispute_lost" => connector_types::EventType::DisputeLost,

        // --- transaction family: these two are the ENTIRE transaction webhook family ---
        // Braintree documents them as available for ACH and SEPA Direct Debit only, and every
        // SDK sample hard-codes `<payment-instrument-type>us_bank_account</...>`. They cannot
        // fire for a card payment. There is no `transaction_authorized`, no
        // `transaction_voided`, no `transaction_processor_declined`, no
        // `transaction_gateway_rejected` — those are transaction STATUSES, not webhook kinds.
        // A Braintree card payment produces no payment-lifecycle webhook, ever.
        "transaction_settled" => connector_types::EventType::PaymentIntentSuccess,
        "transaction_settlement_declined" => connector_types::EventType::PaymentIntentFailure,

        // --- refund family: `refund_failed` is the ONLY refund kind ---
        // There is no `refund_settled` and no `refund_succeeded`; a successful refund is
        // observable only through RSync.
        "refund_failed" => connector_types::EventType::RefundFailure,

        // --- everything else ---
        // Subscription, marketplace/sub-merchant, partner-merchant, disbursement, Local
        // Payment Method, Account Updater, grant/revoke, Fraud Protection (`transaction_reviewed`,
        // whose subject is `<transaction-review>`, a fourth entity this module does not model),
        // the deprecated `transaction_disbursed`, and every kind Braintree adds after this was
        // written.
        //
        // MUST be `IncomingWebhookEventUnspecified`. Never a substituted
        // PaymentIntentProcessing, never a Failure, never a Dispute*: UCS has no basis for any
        // of those and an invented status propagates to the merchant's ledger.
        _ => connector_types::EventType::IncomingWebhookEventUnspecified,
    }
}

// Maps the Braintree NOTIFICATION `kind` to the prism dispute status.
//
// `DisputeStatus` has no `Unknown` variant and `DisputeWebhookDetailsResponse::status` is not
// an `Option`, so a total function is forced. `DisputeOpened` is the least-harmful default
// (it is also the enum's `#[default]`) and is unreachable in practice: this is only entered
// once `get_status` has already returned a dispute `EventType`, which only the kinds below
// can produce. The arm is kept anyway — the two functions must not be coupled by an invariant
// no type enforces. `DisputeCancelled` has no Braintree kind and is never produced.
pub(super) fn get_dispute_status(status: &str) -> enums::DisputeStatus {
    match status {
        "dispute_opened" => enums::DisputeStatus::DisputeOpened,
        "dispute_lost" => enums::DisputeStatus::DisputeLost,
        "dispute_won" => enums::DisputeStatus::DisputeWon,
        "dispute_accepted" | "dispute_auto_accepted" => enums::DisputeStatus::DisputeAccepted,
        "dispute_expired" => enums::DisputeStatus::DisputeExpired,
        "dispute_disputed" | "dispute_under_review" => enums::DisputeStatus::DisputeChallenged,
        _ => enums::DisputeStatus::DisputeOpened,
    }
}

/// Maps the DISPUTE's own `<kind>` (the STAGE — distinct from the notification kind, and
/// confusingly spelled identically) to the prism dispute stage.
///
/// Case-insensitive: Braintree's legacy sample generator emits `CHARGEBACK` and the modern
/// one emits `chargeback`, and both are valid on the wire. Returns a value rather than a
/// `Result`: an unrecognised stage must not discard a dispute notification that has a
/// reply-by deadline attached — the error path was itself the bug.
pub(super) fn get_dispute_stage(code: Option<&str>) -> enums::DisputeStage {
    match code.map(str::trim).map(str::to_ascii_uppercase).as_deref() {
        Some("CHARGEBACK") => enums::DisputeStage::Dispute,
        Some("PRE_ARBITRATION") => enums::DisputeStage::PreArbitration,
        Some("RETRIEVAL") => enums::DisputeStage::PreDispute,
        other => {
            tracing::warn!(
                target: "braintree_webhook",
                stage = ?other,
                "unrecognised Braintree dispute stage; defaulting to Dispute"
            );
            enums::DisputeStage::Dispute
        }
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
// newline-stripped `bt_payload`. Braintree's base64 output is line-wrapped, so the newlines
// must go before the base64 step — but `verify_webhook_source` signs the RAW, newline-
// inclusive `bt_payload`. The asymmetry is deliberate; normalising both to one string breaks
// verification.
pub(super) fn decode_from_request(
    request: &connector_types::RequestDetails,
) -> Result<Notification, Report<domain_types::errors::WebhookError>> {
    let notif = get_webhook_object_from_body(&request.body)?;
    decode_webhook_payload(notif.bt_payload.replace('\n', "").as_bytes())
}

// Builds the typed webhook resource reference for the ParseEvent phase.
//
// `Ok(None)` is the documented, legal answer for "no actionable reference" and is what an
// unmodelled kind (subscription, disbursement, partner-merchant) must get — returning
// `WebhookReferenceIdNotFound` there turns a webhook UCS simply does not model into a hard
// ParseEvent error. `WebhookReferenceIdNotFound` is reserved for a payload whose kind IS
// modelled but whose identifying element is genuinely missing.
//
// One id per slot: the caller matches on which field is populated, so duplicating an id "to
// be safe" makes a refund webhook resolve as a payment.
pub(super) fn get_webhook_reference(
    notification: &Notification,
) -> Result<
    Option<connector_types::WebhookResourceReference>,
    Report<domain_types::errors::WebhookError>,
> {
    let event_type = get_status(notification.kind.as_str());

    if event_type.is_dispute_event() {
        let dispute_data = notification.dispute().ok_or_else(|| {
            error_stack::report!(domain_types::errors::WebhookError::WebhookReferenceIdNotFound)
        })?;
        // HS emits `PaymentId(ConnectorTransactionId(transaction.id))`. The shadow normaliser
        // maps a prism Dispute reference via `connector_dispute_id.or(connector_transaction_id)`,
        // preferring connector_dispute_id, so it MUST be `None` here to match HS byte-for-byte.
        // The dispute's own id is still reported, on `DisputeWebhookDetailsResponse::dispute_id`.
        return Ok(Some(connector_types::WebhookResourceReference::Dispute(
            connector_types::DisputeWebhookReference {
                connector_dispute_id: None,
                connector_transaction_id: Some(dispute_data.transaction.id.clone()),
            },
        )));
    }

    if event_type.is_refund_event() {
        let transaction = notification.transaction().ok_or_else(|| {
            error_stack::report!(domain_types::errors::WebhookError::WebhookReferenceIdNotFound)
        })?;
        return Ok(Some(connector_types::WebhookResourceReference::Refund(
            connector_types::RefundWebhookReference {
                // `<id>` on a `refund_failed` payload is the REFUND's own id; the parent sale
                // is `<refunded-transaction-id>`. Swapping these updates the wrong row.
                connector_refund_id: Some(transaction.connector_refund_id()),
                merchant_refund_id: None,
                connector_transaction_id: transaction.parent_transaction_id(),
                merchant_transaction_id: transaction.order_id.clone(),
            },
        )));
    }

    if event_type.is_payment_event() {
        let transaction = notification.transaction().ok_or_else(|| {
            error_stack::report!(domain_types::errors::WebhookError::WebhookReferenceIdNotFound)
        })?;
        return Ok(Some(connector_types::WebhookResourceReference::Payment(
            connector_types::PaymentWebhookReference {
                connector_transaction_id: Some(transaction.id.clone()),
                merchant_transaction_id: transaction.order_id.clone(),
            },
        )));
    }

    // Unmodelled kind: no actionable reference, not an error.
    Ok(None)
}

// Builds the payment webhook response.
//
// Reachable for `transaction_settled` / `transaction_settlement_declined` and — via the
// misc-event fall-through in `process_webhook_event` — for every unmodelled kind. The
// unmodelled case has no `<transaction>` at all and must degrade to `Unspecified`, not error,
// which is why the transaction is optional here.
//
// NOTE: both transaction kinds are ACH / SEPA Direct Debit ONLY. Braintree emits no
// payment-lifecycle webhook for a card transaction, so this handler cannot fire for a card
// payment. It is implemented anyway because it is the same code the ACH/SEPA payment methods
// will need, and because the trait default is a hard `WebhooksNotImplemented` error that the
// misc fall-through would surface for every unmodelled kind.
pub(super) fn build_webhook_payment_response(
    notification: &Notification,
    raw_body: &[u8],
) -> Result<connector_types::WebhookDetailsResponse, Report<domain_types::errors::WebhookError>> {
    let transaction = notification.transaction();

    if let Some(instrument) = transaction.and_then(|t| t.payment_instrument_type.as_deref()) {
        if instrument != "us_bank_account" && instrument != "sepa_debit_account" {
            // Not rejected: a webhook must never be dropped on a shape assumption. Logged so
            // the anomaly is visible if Braintree ever extends the family to other instruments.
            tracing::warn!(
                target: "braintree_webhook",
                payment_instrument_type = instrument,
                kind = notification.kind.as_str(),
                "Braintree transaction webhook on a non-bank instrument"
            );
        }
    }

    // Derived from `<status>`, never from the kind, and routed through the GraphQL-side enum
    // so the single `From<BraintreePaymentStatus> for AttemptStatus` mapping is reused.
    let status = transaction
        .map(|t| enums::AttemptStatus::from(BraintreePaymentStatus::from(t.status)))
        .unwrap_or(enums::AttemptStatus::Unspecified);

    // Processor codes are only meaningful on the declined kind.
    let is_failure = domain_types::utils::is_payment_failure(status);
    let error_code = transaction
        .filter(|_| is_failure)
        .and_then(|t| t.processor_response_code.clone());
    let error_message = transaction
        .filter(|_| is_failure)
        .and_then(|t| t.processor_response_text.clone());

    Ok(connector_types::WebhookDetailsResponse {
        resource_id: transaction.map(|t| ResponseId::ConnectorTransactionId(t.id.clone())),
        status,
        connector_response_reference_id: transaction.and_then(|t| t.order_id.clone()),
        // Braintree echoes no separate request reference in the webhook payload.
        connector_request_reference_id: None,
        // Braintree has no mandate webhook kind.
        mandate_reference: None,
        error_code,
        error_message,
        error_reason: None,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        // `transaction_settled` reports a SETTLEMENT. Equating a settled amount with a
        // captured amount is an inference a webhook handler should not make when PSync
        // reports the captured amount directly.
        amount_captured: None,
        minor_amount_captured: None,
        // Not carried in the webhook payload.
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

// Builds the refund webhook response for `refund_failed`, the only refund kind Braintree
// emits. The payload is a `<transaction>` whose `<id>` is the REFUND's own id and whose
// `<refunded-transaction-id>` is the parent sale.
pub(super) fn build_webhook_refund_response(
    notification: &Notification,
    raw_body: &[u8],
) -> Result<connector_types::RefundWebhookDetailsResponse, Report<domain_types::errors::WebhookError>>
{
    let transaction = notification.transaction().ok_or_else(|| {
        error_stack::report!(domain_types::errors::WebhookError::WebhookResourceObjectNotFound)
    })?;

    Ok(connector_types::RefundWebhookDetailsResponse {
        connector_refund_id: Some(transaction.connector_refund_id()),
        merchant_transaction_id: transaction.order_id.clone(),
        status: enums::RefundStatus::from(transaction.status),
        // The PARENT sale, never this entity's own id.
        connector_response_reference_id: transaction.parent_transaction_id(),
        error_code: transaction.processor_response_code.clone(),
        error_message: transaction.processor_response_text.clone(),
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}

// Builds the dispute webhook response, including the webhook amount conversion.
//
// `amount` and `currency` on `DisputeWebhookDetailsResponse` are NOT `Option`, so a missing
// `amount-disputed` or `currency-iso-code` is a typed error — never a substituted zero.
pub(super) fn build_webhook_dispute_response(
    notification: &Notification,
    raw_body: &[u8],
) -> Result<
    connector_types::DisputeWebhookDetailsResponse,
    Report<domain_types::errors::WebhookError>,
> {
    let dispute_data = notification.dispute().ok_or_else(|| {
        error_stack::report!(domain_types::errors::WebhookError::WebhookResourceObjectNotFound)
    })?;

    let currency = dispute_data.currency_iso_code.ok_or_else(|| {
        error_stack::report!(
            domain_types::errors::WebhookError::WebhookMissingRequiredField {
                field: "currency-iso-code",
            }
        )
    })?;

    // Braintree puts money on the wire as a decimal MAJOR-unit string (`10.00`), so the value
    // is parsed back to minor units before being handed to the connector's configured webhook
    // converter (`amount_converter_webhooks: StringMinorUnit`).
    let amount_disputed = dispute_data.amount_disputed.clone().ok_or_else(|| {
        error_stack::report!(
            domain_types::errors::WebhookError::WebhookMissingRequiredField {
                field: "amount-disputed",
            }
        )
    })?;
    let minor_amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
        &common_utils::types::StringMajorUnitForConnector,
        amount_disputed,
        currency,
    )?;

    Ok(connector_types::DisputeWebhookDetailsResponse {
        amount: domain_types::utils::convert_amount_for_webhook(
            &common_utils::types::StringMinorUnitForConnector,
            minor_amount,
            currency,
        )?,
        currency,
        dispute_id: dispute_data.id.clone(),
        // Status comes from the NOTIFICATION kind; stage from the DISPUTE's own `<kind>`.
        // Both elements are literally named `kind`, which is why this is easy to get wrong.
        status: get_dispute_status(notification.kind.as_str()),
        stage: get_dispute_stage(dispute_data.kind.as_deref()),
        connector_response_reference_id: dispute_data.transaction.order_id.clone(),
        dispute_message: dispute_data.reason.clone(),
        connector_reason_code: dispute_data.reason_code.clone(),
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
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
        // The two MIT regimes are selected here and nowhere else — see RULE NT-1 on
        // `BraintreeMitVaultRegime`. The selector is the payment-method shape, because that is
        // what says WHICH vault holds the credential: a Braintree vault token arrives as
        // `MandatePayment` + `connector_mandate_id`, an externally vaulted credential arrives as
        // a single-use `PaymentMethodToken` minted by `PaymentMethodService/Tokenize`.
        match item.router_data.request.payment_method_data.clone() {
            // Regime A — Braintree-vaulted. `options.externalVault` is never emitted.
            PaymentMethodData::MandatePayment => {
                let connector_mandate_id = item.router_data.request.connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    },
                )?;
                Ok(Self::Mandate(MandatePaymentRequest::try_from((
                    item,
                    BraintreeMitVaultRegime::BraintreeVaulted {
                        connector_mandate_id,
                    },
                    metadata,
                ))?))
            }
            // Regime B — externally vaulted. The caller already exchanged the card for a
            // single-use Braintree token (Braintree never accepts a raw PAN on a transaction
            // mutation), so UCS is the vault of record and must replay the CIT's NTID.
            PaymentMethodData::PaymentMethodToken(token) => {
                let network_transaction_id =
                    braintree_mit_network_transaction_id(&item.router_data.request);
                Ok(Self::Mandate(MandatePaymentRequest::try_from((
                    item,
                    BraintreeMitVaultRegime::ExternallyVaulted {
                        single_use_token: token.token,
                        network_transaction_id,
                    },
                    metadata,
                ))?))
            }
            // A raw card cannot reach a `paymentMethodId` mutation; it must be tokenized first.
            PaymentMethodData::Card(_) => Err(raw_card_not_tokenized_error()),
            PaymentMethodData::Wallet(_)
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
            | PaymentMethodData::NetworkToken(_)
            // Both of these carry a network transaction id with RAW CARD / decrypted wallet data
            // attached. Braintree's transaction mutations take a `paymentMethodId: ID!` and have
            // no PAN-bearing input at all, so neither is representable here — and adding a
            // tokenize leg inside the connector is deliberately out of scope.
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

/// `CreditCardTransactionOptionsInput` — `input.options`, a sibling of `input.transaction`.
/// Only `chargeCreditCard` / `authorizeCreditCard` accept it; `chargePaymentMethod` /
/// `authorizePaymentMethod` (the wallet and vaulted-credential mutations) have no `options`
/// member at all, which is why the billing address and 3DS pass-through are unreachable there.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardTransactionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_authentication: Option<ThreeDSecureAuthenticationInput>,
    /// Billing address lives on `options`, NOT on `transaction` — the shipping address is the
    /// other way round. This is the split that AVS depends on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<BraintreeAddressInput>,
    /// `TransactionExternalVaultOptionsInput`. Emitted **only** when the credential being charged
    /// is vaulted OUTSIDE Braintree (RULE NT-1, see `BraintreeMitVaultRegime`). It MUST stay
    /// `None` for a Braintree multi-use (vaulted) token: Braintree's own schema documentation
    /// forbids it there, and — crucially — the gateway does NOT reject the violation, so this is
    /// a by-construction invariant rather than a testable one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_vault: Option<TransactionExternalVaultOptions>,
}

impl CreditCardTransactionOptions {
    fn is_empty(&self) -> bool {
        self.three_d_secure_authentication.is_none()
            && self.billing_address.is_none()
            && self.external_vault.is_none()
    }
}

/// `TransactionExternalVaultOptionsInput` — `input.options.externalVault`, a sibling of
/// `input.transaction` (it is NOT a member of `TransactionInput`).
///
/// Modelled as an internally tagged enum so that the one combination Braintree rejects —
/// `status: WILL_VAULT` together with a `verifyingNetworkTransactionId` — is unrepresentable.
/// `status` is `ExternalVaultStatus!` in the schema (NON-NULL, exactly two members `VAULTED` and
/// `WILL_VAULT`), so it is always emitted and is never a free-form string.
///
/// The NTID field is `verifyingNetworkTransactionId`. `previousNetworkTransactionId` is NOT a
/// field of this type and is rejected at variable coercion.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum TransactionExternalVaultOptions {
    /// The credential is not externally vaulted yet, but will be if this transaction succeeds.
    /// Braintree forbids sending an NTID with this status, hence the unit variant.
    #[serde(rename = "WILL_VAULT")]
    WillVault,
    /// The credential is already held in a non-Braintree vault. The NTID is optional: Braintree
    /// accepts `{ status: VAULTED }` on its own, which is the correct degradation when the CIT
    /// did not hand one over.
    #[serde(rename = "VAULTED", rename_all = "camelCase")]
    Vaulted {
        #[serde(skip_serializing_if = "Option::is_none")]
        verifying_network_transaction_id: Option<Secret<String>>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureAuthenticationInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass_through: Option<ThreeDSecurePassThroughInput>,
}

/// `ThreeDSecureCavvAlgorithm` — an open scalar, but Braintree documents exactly two values:
/// `2` (CVV with ATN) and `3` (Mastercard SPA). Any other algorithm code must be omitted rather
/// than coerced, so it is modelled as an enum rather than a passthrough string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ThreeDSecureCavvAlgorithm {
    #[serde(rename = "2")]
    CvvWithAtn,
    #[serde(rename = "3")]
    MastercardSpa,
}

/// `ThreeDSecurePassThroughNetwork` — a real GraphQL enum with exactly three members, so an
/// unlisted value is a hard coercion error, not a silently ignored one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecurePassThroughNetwork {
    Eftpos,
    Mastercard,
    Visa,
}

/// `ThreeDSecurePassThroughInput` — results of a **merchant-performed** (external MPI) 3D Secure
/// authentication. Nine members; `eciFlag` is the only non-null one, which is why the whole
/// object is only built when an ECI is present.
///
/// Field names are the SDL names and several of them are not the obvious ones:
/// `xId` (capital I), `version` (not `threeDSecureVersion`), `directoryServerResponse` (not
/// `authenticationResponse`) and `directoryServerTransactionId`. A field literally named
/// `dsTransactionId` does exist in the schema but belongs to
/// `ThreeDSecurePriorAuthenticationDetailsInput` (the 3RI / prior-authentication tree under
/// `performThreeDSecureLookup`) — it must not be cross-wired here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecurePassThroughInput {
    /// `ECommerceIndicator!` — non-null. A card-brand-specific two-digit string whose leading
    /// zero is significant (`"02"` != `2`); passed through from the MPI verbatim and never
    /// derived from the card network.
    pub eci_flag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    /// 3DS **1.x** only — the SDL says it is no longer used in 3DS 2 authentications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_server_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory_server_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv_algorithm: Option<ThreeDSecureCavvAlgorithm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory_server_transaction_id: Option<String>,
    /// Dual-network (eftpos) pinning. No UCS source today; kept so the wire shape matches the SDL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<ThreeDSecurePassThroughNetwork>,
}

/// What the Authorize builder must do about 3D Secure on a tokenized card.
///
/// After the Braintree-hosted 3DS trio shipped, an Authorize request can carry
/// `authentication_data` from **two mutually exclusive ingresses**, and they need opposite
/// treatment:
///
/// * **External MPI** — the caller performed 3DS themselves and sent the result on the original
///   Authorize request. Braintree must be told about it, via
///   `options.threeDSecureAuthentication.passThrough`.
/// * **Braintree-hosted** (PreAuthenticate / Authenticate / PostAuthenticate) — the composite
///   dispatcher copies PostAuthenticate's `authentication_data` into the Authorize request
///   (hyperswitch does the same on its `CompleteAuthorize`). Braintree must be told **nothing**:
///   the authentication already lives ON the payment method and Braintree attaches it to the
///   transaction itself. Live-proven — a 3DS-verified nonce charged with NO `threeDSecurePassThru`
///   came back `SUBMITTED_FOR_SETTLEMENT` with a populated
///   `paymentMethodSnapshot.threeDSecure.authentication` and `liabilityShifted: true`.
///
/// Branching on `authentication_data.is_some()` alone would therefore cross-wire a
/// Braintree-PERFORMED authentication into the external-MPI pass-through — UCS asserting an
/// externally performed authentication for one Braintree performed itself. The two topologies
/// must not merge, which is what this enum exists to keep apart.
///
/// The discriminator is the marker legs 2 and 3 plant in `connector_feature_data`, which the
/// composite dispatcher forwards into the Authorize request. Rejected alternatives, recorded so
/// they are not re-proposed: sniffing the token shape (`tokencc_` prefix vs bare UUID) is an
/// undocumented string heuristic on a credential; `is_three_ds()` alone is true for external MPI
/// too; and a new `PaymentsAuthorizeData` member would be a cross-crate change for something an
/// already-forwarded channel expresses exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BraintreeAuthorizeThreeDsMode {
    /// Nothing to assert: not a 3D Secure payment and no authentication supplied.
    None,
    /// Braintree-hosted. Charge the verified nonce and send NO pass-through.
    Hosted,
    /// Externally (MPI) performed. Send `options.threeDSecureAuthentication.passThrough`.
    ExternalPassThrough,
    /// A 3D Secure payment that reached Authorize with neither a completed Braintree-hosted
    /// authentication nor an external one. Must fail closed rather than charge unauthenticated.
    Unauthenticated,
}

fn braintree_authorize_three_ds_mode(
    is_three_ds: bool,
    authentication_data: Option<&router_request_types::AuthenticationData>,
    connector_feature_data: Option<&pii::SecretSerdeValue>,
) -> BraintreeAuthorizeThreeDsMode {
    let is_braintree_hosted = connector_feature_data
        .and_then(|feature_data| {
            feature_data
                .peek()
                .get(constants::BRAINTREE_THREE_DS_FEATURE_KEY)
        })
        .is_some();

    // Hosted wins over `authentication_data`: on the composite path BOTH are present, and the
    // hosted authentication is the one that actually happened.
    if is_braintree_hosted {
        BraintreeAuthorizeThreeDsMode::Hosted
    } else if authentication_data.is_some() {
        BraintreeAuthorizeThreeDsMode::ExternalPassThrough
    } else if is_three_ds {
        BraintreeAuthorizeThreeDsMode::Unauthenticated
    } else {
        BraintreeAuthorizeThreeDsMode::None
    }
}

/// Returns `None` when the authentication carries no ECI: `eciFlag` is non-null, so a
/// pass-through object without one cannot be sent, and sending a placeholder ECI would be a
/// claim about the authentication outcome that the MPI never made.
fn convert_external_three_ds_data(
    auth_data: &router_request_types::AuthenticationData,
) -> Option<ThreeDSecurePassThroughInput> {
    let eci_flag = auth_data.eci.clone()?;
    let version = auth_data
        .message_version
        .as_ref()
        .map(|semantic_version| semantic_version.to_string());
    // The XID is a 3DS 1.x artefact; suppress it on 3DS 2 authentications.
    let is_three_ds_one = auth_data
        .message_version
        .as_ref()
        .is_some_and(|semantic_version| semantic_version.get_major() < 2);
    Some(ThreeDSecurePassThroughInput {
        eci_flag,
        cavv: auth_data.cavv.clone(),
        x_id: is_three_ds_one
            .then(|| auth_data.transaction_id.clone())
            .flatten(),
        three_d_secure_server_transaction_id: auth_data.threeds_server_transaction_id.clone(),
        version,
        directory_server_response: auth_data
            .trans_status
            .as_ref()
            .map(map_transaction_status_to_code),
        cavv_algorithm: match auth_data.get_cavv_algorithm() {
            Some("2") => Some(ThreeDSecureCavvAlgorithm::CvvWithAtn),
            Some("3") => Some(ThreeDSecureCavvAlgorithm::MastercardSpa),
            // Braintree documents only `2` and `3`; anything else is omitted, not coerced.
            _ => None,
        },
        directory_server_transaction_id: auth_data.ds_trans_id.clone(),
        network: None,
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
                        resource_id: ResponseId::ConnectorTransactionId(
                            transaction_data.id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: transaction_data.payment_method.as_ref().map(|pm| {
                            Box::new(MandateReference {
                                connector_mandate_id: Some(pm.id.clone().expose()),
                                payment_method_id: None,
                                connector_mandate_request_reference_id: None,
                                // The CIT -> MIT hand-off of the scheme NTID. The vault token
                                // and the NTID cannot both travel in `MandateReferenceId`, so
                                // the NTID rides here. See `BraintreeMandateMetadata`.
                                mandate_metadata: build_braintree_mandate_metadata(
                                    transaction_data.network_transaction_id(),
                                ),
                            })
                        }),
                        connector_metadata: None,
                        // Read off `paymentMethodSnapshot { ... on CreditCardTransactionDetails
                        // { networkTransactionId } }`. `network_txn_link_id` stays `None`: that
                        // slot is the Mastercard Transaction Link Identifier, a different
                        // identifier that Braintree does not expose at the pinned version.
                        network_txn_id: transaction_data.network_transaction_id(),
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
                    | BraintreePaymentStatus::SubmittedForSettlement
                    // An unrecognised status is not evidence that the reversal failed; stay
                    // non-terminal so the caller re-syncs rather than reporting a void failure
                    // that never happened.
                    | BraintreePaymentStatus::Unknown => {
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
                            cardholder_name: card_data.card_holder_name.clone(),
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

// -------------------------------------------------------------------------------------------
// Braintree-hosted 3D Secure, leg 1: `PreAuthenticate` (device data collection setup).
//
// Braintree has no server-side "DDC setup" endpoint. Its documented bootstrap for 3DS is the
// client token: the cardholder's browser runs `braintree.threeDSecure.create({authorization:
// clientToken})` and then `threeDSecure.prepareLookup({nonce, bin})` to produce the
// `dfReferenceId` that the later `Authenticate` leg (`performThreeDSecureLookup`) requires.
// This leg therefore mints that client token and hands the caller the three things
// `prepareLookup` needs — client token, single-use nonce and card BIN — plus the caller's own
// completion URL, as `RedirectForm::Braintree`.
//
// Both mutations go out in ONE HTTP call as two root fields of a single document
// (`constants::PRE_AUTHENTICATE_MUTATION`); they take disjoint inputs so neither depends on the
// other's result.
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreAuthenticateVariables<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    /// `$card: TokenizeCreditCardInput!`
    card: InputData<T>,
    /// `$clientToken: CreateClientTokenInput!`
    client_token: InputClientTokenData,
}

/// Resolve the Braintree merchant account id for a flow whose request carries only the generic
/// `metadata` bag rather than a dedicated `merchant_account_id` member.
///
/// The per-request value wins over the connector-config copy, mirroring the precedence the
/// Authorize builder applies to `PaymentsAuthorizeData::merchant_account_id`.
fn resolve_merchant_account_id(
    request_metadata: &Option<pii::SecretSerdeValue>,
    connector_config: &ConnectorSpecificConfig,
) -> Result<Secret<String>, Report<IntegrationError>> {
    if let Ok(merchant_account_id) =
        extract_metadata_string_field(request_metadata, "merchant_account_id")
    {
        info!("BRAINTREE: Picking merchant_account_id from the per-request metadata");
        return Ok(merchant_account_id);
    }
    BraintreeAuthType::try_from(connector_config)?
        .merchant_account_id
        .ok_or_else(|| {
            IntegrationError::InvalidConnectorConfig {
                config: "merchant_account_id",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Braintree's `createClientToken` is merchant-account scoped and this \
                         connector treats `merchantAccountId` as mandatory on every flow."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Send `merchant_account_id` in the request metadata, or configure it on \
                         the Braintree connector account."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Input--ClientTokenInput"
                            .to_string(),
                    ),
                },
            }
            .into()
        })
}

fn pre_authenticate_payment_method_error(detail: &str) -> Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: "given payment method on Braintree PreAuthenticate".to_string(),
        connector: "Braintree",
        context: domain_types::errors::IntegrationErrorContext {
            additional_context: Some(detail.to_string()),
            suggested_action: Some(
                "Send the raw card in `payment_method.card` on PreAuthenticate. Braintree's \
                 `threeDSecure.prepareLookup` needs the card BIN alongside the nonce, and a BIN \
                 cannot be recovered from an already-issued token."
                    .to_string(),
            ),
            doc_url: Some(
                "https://developer.paypal.com/braintree/docs/guides/3d-secure/step-by-step-integration/javascript/v3"
                    .to_string(),
            ),
        },
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
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
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let merchant_account_id = resolve_merchant_account_id(
            &item.router_data.request.metadata,
            &item.router_data.connector_config,
        )?;

        // No currency validation on this leg: `createClientToken` moves no money and
        // `ClientTokenInput` has no amount member, so `merchant_config_currency` is not consulted.
        let payment_method_data = item
            .router_data
            .request
            .payment_method_data
            .clone()
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_data",
                    context: domain_types::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "PreAuthenticate has nothing to authenticate without a card."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Send the card in `payment_method.card` on the PreAuthenticate request."
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                })
            })?;

        match payment_method_data {
            PaymentMethodData::Card(card_data) => Ok(Self {
                query: constants::PRE_AUTHENTICATE_MUTATION.to_string(),
                variables: PreAuthenticateVariables {
                    card: InputData {
                        credit_card: CreditCardData {
                            number: card_data.card_number,
                            expiration_year: card_data.card_exp_year,
                            expiration_month: card_data.card_exp_month,
                            cvv: card_data.card_cvc,
                            cardholder_name: card_data.card_holder_name.clone(),
                        },
                    },
                    client_token: InputClientTokenData {
                        client_token: ClientTokenInput {
                            merchant_account_id,
                        },
                    },
                },
            }),
            // An already-issued nonce carries no BIN (`PaymentMethodData::PaymentMethodToken` is
            // `{ token, token_payment_method_type }`), and `threeDSecure.prepareLookup` requires
            // one. Reject rather than emit a `RedirectForm` with an empty `bin`.
            PaymentMethodData::PaymentMethodToken(_) => Err(pre_authenticate_payment_method_error(
                "Braintree's 3D Secure device-data-collection bootstrap needs the card BIN, which \
                 an already-tokenized payment method does not carry.",
            )),
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
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(pre_authenticate_payment_method_error(
                    "Braintree-hosted 3D Secure is card-only on this connector.",
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreAuthenticateData {
    tokenize_credit_card: TokenizeCreditCardData,
    create_client_token: ClientToken,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PreAuthenticateSuccessResponse {
    data: PreAuthenticateData,
}

/// Braintree answers HTTP 200 for everything, so success and failure are separated by body shape.
///
/// `ErrorResponse` is listed **first** deliberately: on a GraphQL partial success the body carries
/// both a populated `errors[]` and a `data` object, and the errors must win. `ErrorResponse`
/// requires an `errors` member, so a clean success can never match it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreePreAuthenticateResponse {
    ErrorResponse(Box<ErrorResponse>),
    PreAuthenticateResponse(Box<PreAuthenticateSuccessResponse>),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreePreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreePreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreePreAuthenticateResponse::ErrorResponse(error_response) => Ok(Self {
                // No status write on the error path: `AttemptStatus` for a failed authentication
                // setup belongs to the caller's state machine, and the shared Braintree error
                // builder is flow-agnostic.
                response: build_error_response::<PaymentsResponseData>(
                    error_response.errors.as_ref(),
                    item.http_code,
                )
                .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreePreAuthenticateResponse::PreAuthenticateResponse(success) => {
                let client_token = success.data.create_client_token.client_token.clone();
                if client_token.peek().is_empty() {
                    // `CreateClientTokenPayload.clientToken` is nullable in the SDL. An empty
                    // token would produce a `RedirectForm` the browser cannot bootstrap from.
                    return Err(utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree returned an empty clientToken on createClientToken",
                    )
                    .into());
                }

                let card_details = item
                    .router_data
                    .request
                    .payment_method_data
                    .clone()
                    .ok_or_else(|| {
                        utils::unexpected_response_fail(
                            item.http_code,
                            "PreAuthenticate response reached the transformer without the card it \
                             was built from",
                        )
                    })?;

                // Echoed back to the caller so the browser knows where to post the DDC outcome.
                // `RedirectForm::Braintree::acs_url` is a misnomer inherited from hyperswitch: it
                // is the merchant's own completion URL, not an ACS URL.
                let ddc_return_url = item
                    .router_data
                    .request
                    .continue_redirection_url
                    .as_ref()
                    .or(item.router_data.request.router_return_url.as_ref())
                    .map(ToString::to_string)
                    .ok_or_else(|| {
                        utils::unexpected_response_fail(
                            item.http_code,
                            "PreAuthenticate needs `continue_redirection_url` or \
                             `router_return_url` to tell the browser where to return after device \
                             data collection",
                        )
                    })?;

                let redirection_data = get_braintree_redirect_form(
                    client_token,
                    success.data.tokenize_credit_card.payment_method.id.clone(),
                    card_details,
                    ddc_return_url,
                )?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        // Fixed, not mapped: `createClientToken` returns no status of any kind,
                        // and the next thing that must happen is device data collection.
                        status: enums::AttemptStatus::DeviceDataCollectionPending,
                        ..item.router_data.resource_common_data.clone()
                    },
                    response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                        // A client token is a credential, not a Braintree object: `node(id:)`
                        // cannot resolve it, so any id here would be unsyncable.
                        resource_id: None,
                        // Nothing has been authenticated yet — no CAVV, no ECI, no status.
                        authentication_data: None,
                        redirection_data: Some(Box::new(redirection_data)),
                        connector_response_reference_id: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

// -------------------------------------------------------------------------------------------
// Braintree-hosted 3D Secure, leg 2: `Authenticate` (the server-side 3DS lookup).
//
// One call, one HTTP request: `mutation performThreeDSecureLookup`. It consumes the single-use
// nonce that leg 1 (`PreAuthenticate` / `PaymentMethodToken`) minted, runs the enrolment lookup,
// and answers either a challenge (ACS URL + CReq, surfaced as a `RedirectForm::Form`) or a
// settled frictionless outcome (CAVV + ECI, surfaced as `AuthenticationData`).
//
// The whole selection set and the four response shapes below were exercised live against the
// Braintree sandbox under `Braintree-Version: 2019-01-01`; see `constants::AUTHENTICATE_MUTATION`.
// -------------------------------------------------------------------------------------------

/// `PerformThreeDSecureLookupInput`, as served at the pinned `Braintree-Version`.
///
/// The served input has 13 members. The three not modelled here are deliberate omissions with no
/// UCS ingress: `dataOnlyRequested` and `cardAdd` are merchant policy, and
/// `merchantInitiatedRequest` is the 3RI / prior-authentication tree, which stays out of scope.
/// `clientMutationId` is not sent (it is an echo, never an idempotency key) and
/// `clientInformation` is not sent because UCS's `SdkInformation` describes a 3DS SDK rather than
/// the Braintree JS SDK, so the mapping would be a type confusion.
///
/// Every optional member is `skip_serializing_if = "Option::is_none"`: Braintree distinguishes
/// "absent" from "present and null", and an explicit `null` is not the same as an omission.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformThreeDSecureLookupInput {
    /// `ID!` — the single-use nonce from the `PaymentMethodToken` / `PreAuthenticate` leg.
    payment_method_id: Secret<String>,
    /// `Amount!` — a major-unit decimal string ("10.00"), via the connector's `amount_converter`.
    amount: StringMajorUnit,
    merchant_account_id: Secret<String>,
    /// Device-data ingress 1 of 2, and the preferred one: CardinalCommerce's join key for the
    /// device data the browser already collected via `threeDSecure.prepareLookup`.
    #[serde(skip_serializing_if = "Option::is_none")]
    df_reference_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transaction_information: Option<ThreeDSecureLookupTransactionInformationInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cardholder_information: Option<ThreeDSecureLookupCardholderInformationInput>,
}

/// `ThreeDSecureLookupTransactionInformationInput` — 4 of its 46 members are mapped; the other 42
/// (shipping, installments, recurring, order description, …) have no `PaymentsAuthenticateData`
/// ingress.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureLookupTransactionInformationInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    device_channel: Option<ThreeDSecureDeviceChannel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<pii::Email>,
    /// NOTE: `ipAddress` is a sibling of `browserInformation`, one level up — the schema rejects
    /// it nested inside `browserInformation`.
    #[serde(skip_serializing_if = "Option::is_none")]
    ip_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    browser_information: Option<ThreeDSecureLookupBrowserInformationInput>,
}

impl ThreeDSecureLookupTransactionInformationInput {
    fn is_empty(&self) -> bool {
        self.device_channel.is_none()
            && self.email.is_none()
            && self.ip_address.is_none()
            && self.browser_information.is_none()
    }
}

/// `ThreeDSecureLookupBrowserInformationInput` — all nine members map 1:1 from
/// `BrowserInformation`. Device-data ingress 2 of 2.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureLookupBrowserInformationInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    java_enabled: Option<bool>,
    /// Braintree spells this with ONE capital (`javascriptEnabled`), unlike the UCS
    /// `java_script_enabled`. A camelCase rename of the UCS name would be rejected.
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
    /// `Int`, minutes of UTC offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    time_zone: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_agent: Option<String>,
}

impl ThreeDSecureLookupBrowserInformationInput {
    fn is_empty(&self) -> bool {
        self.java_enabled.is_none()
            && self.java_script_enabled.is_none()
            && self.accept_header.is_none()
            && self.language.is_none()
            && self.color_depth.is_none()
            && self.screen_height.is_none()
            && self.screen_width.is_none()
            && self.time_zone.is_none()
            && self.user_agent.is_none()
    }
}

/// `ThreeDSecureLookupCardholderInformationInput` — it has exactly one member.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureLookupCardholderInformationInput {
    billing_address: ThreeDSecureLookupBillingAddressInput,
}

/// `ThreeDSecureLookupBillingAddressInput`. `line3` is not sent (no UCS ingress).
///
/// NOTE: `countryCode` here is a plain `String`, **not** the versioned `CountryCode` scalar the
/// Authorize address input uses — so the alpha-3/alpha-2 boundary at `Braintree-Version
/// 2021-02-01` does not apply on this leg. Send the country as UCS holds it; do NOT reuse the
/// Authorize `from_alpha2_to_alpha3` conversion.
#[derive(Debug, Clone, Serialize)]
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
    country_code: Option<common_enums::CountryAlpha2>,
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

/// `ThreeDSecureDeviceChannel`. `THREE_R_I` exists on the wire but is never sent — 3RI is out of
/// scope on this connector.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureDeviceChannel {
    Browser,
    Sdk,
}

impl From<connector_types::DeviceChannel> for ThreeDSecureDeviceChannel {
    fn from(channel: connector_types::DeviceChannel) -> Self {
        match channel {
            connector_types::DeviceChannel::Browser => Self::Browser,
            connector_types::DeviceChannel::App => Self::Sdk,
        }
    }
}

fn authenticate_payment_method_error(detail: &str) -> Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: "given payment method on Braintree Authenticate".to_string(),
        connector: "Braintree",
        context: domain_types::errors::IntegrationErrorContext {
            additional_context: Some(detail.to_string()),
            suggested_action: Some(
                "Run the PaymentMethodToken / PreAuthenticate leg first and send the resulting \
                 single-use nonce in `payment_method.token` on Authenticate."
                    .to_string(),
            ),
            doc_url: Some(
                "https://graphql.braintreepayments.com/reference/#Mutation--performThreeDSecureLookup"
                    .to_string(),
            ),
        },
    })
}

/// Pull `dfReferenceId` out of `redirect_response.payload`.
///
/// The gRPC conversion builds `payload` as a flat `{string: string}` JSON object from the
/// redirection response's string map, so both the Braintree spelling (`dfReferenceId`) and a
/// snake_case alias are accepted — callers echo back whatever key their browser integration used.
fn extract_df_reference_id(
    redirect_response: Option<&connector_types::ContinueRedirectionResponse>,
) -> Option<Secret<String>> {
    let payload = redirect_response?.payload.as_ref()?.clone().expose();
    ["dfReferenceId", "df_reference_id"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(|value| value.as_str()))
        .map(|value| Secret::new(value.to_string()))
}

fn build_lookup_browser_information(
    browser_info: Option<&router_request_types::BrowserInformation>,
) -> Option<ThreeDSecureLookupBrowserInformationInput> {
    let browser_info = browser_info?;
    let built = ThreeDSecureLookupBrowserInformationInput {
        java_enabled: browser_info.java_enabled,
        java_script_enabled: browser_info.java_script_enabled,
        accept_header: browser_info.accept_header.clone(),
        language: browser_info.language.clone(),
        color_depth: browser_info.color_depth,
        screen_height: browser_info.screen_height,
        screen_width: browser_info.screen_width,
        time_zone: browser_info.time_zone,
        user_agent: browser_info.user_agent.clone(),
    };
    // Braintree distinguishes "absent" from "present and empty"; never emit `{}`.
    (!built.is_empty()).then_some(built)
}

fn build_lookup_billing_address(
    address: Option<&domain_types::payment_address::Address>,
) -> Option<ThreeDSecureLookupCardholderInformationInput> {
    let address = address?;
    let details = address.address.as_ref();
    let built = ThreeDSecureLookupBillingAddressInput {
        // No `billing_full_name` fallback: `givenName` / `surname` are mapped explicitly and each
        // is omitted when absent, rather than splitting a full name on whitespace.
        given_name: details.and_then(|d| d.first_name.clone()),
        surname: details.and_then(|d| d.last_name.clone()),
        line1: details.and_then(|d| d.line1.clone()),
        line2: details.and_then(|d| d.line2.clone()),
        locality: details.and_then(|d| d.city.clone()),
        region: details.and_then(|d| d.state.clone()),
        postal_code: details.and_then(|d| d.zip.clone()),
        country_code: details.and_then(|d| d.country),
        phone_number: address.get_phone_with_country_code().ok(),
    };
    (!built.is_empty()).then_some(ThreeDSecureLookupCardholderInformationInput {
        billing_address: built,
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
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
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        // Per-request metadata wins over the connector-config copy. Reused verbatim from the
        // PreAuthenticate leg — the helper is deliberately flow-agnostic.
        let merchant_account_id =
            resolve_merchant_account_id(&request.metadata, &item.router_data.connector_config)?;

        // An externally (MPI) performed authentication and a Braintree-hosted lookup are mutually
        // exclusive topologies. `authentication_data` means the caller already holds a CAVV and
        // wants the `threeDSecurePassThru` path on Authorize; running a second, contradictory
        // authentication here would silently discard theirs.
        if request.authentication_data.is_some() {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "externally authenticated 3DS data on Braintree Authenticate".to_string(),
                connector: "Braintree",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "`authentication_data` carries a merchant-performed (MPI) authentication. \
                         Braintree-hosted 3DS and external 3DS pass-through are mutually exclusive."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Send the external CAVV/ECI on Authorize (it is applied as \
                         `threeDSecurePassThru`) instead of calling Authenticate."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Input--ThreeDSecurePassThroughInput"
                            .to_string(),
                    ),
                },
            }));
        }

        let payment_method_data = request.payment_method_data.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "payment_method_data",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Authenticate has nothing to look up without a tokenized instrument."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Send the single-use nonce in `payment_method.token` on the Authenticate \
                         request."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })
        })?;

        let payment_method_id = match payment_method_data {
            PaymentMethodData::PaymentMethodToken(token) => token.token.clone(),
            // Raw card is rejected on purpose: `performThreeDSecureLookup` takes a tokenized
            // instrument, and re-tokenizing here would mint a second nonce and make the leg
            // non-idempotent (the lookup consumes whichever nonce it is given).
            _ => {
                return Err(authenticate_payment_method_error(
                    "Braintree's 3D Secure lookup takes an already-tokenized payment method; \
                     tokenizing inside this leg would mint a second nonce.",
                ))
            }
        };

        let currency = request.currency.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "The Braintree `Amount` scalar is a major-unit decimal string, so the \
                         minor amount cannot be converted without a currency."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Send `amount.currency` on the Authenticate request.".to_string(),
                    ),
                    doc_url: None,
                },
            })
        })?;

        // Major units through the connector's own converter. `MinorUnit::to_string()` here would
        // be a silent 100x overstatement that Braintree cannot detect, because "1000" is itself a
        // valid `Amount`.
        let amount = item
            .connector
            .amount_converter
            .convert(request.amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "failed to convert the Authenticate amount into Braintree major units"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "check that the request amount and currency are consistent and in range"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })?;

        // Device data has TWO accepted ingresses and `dfReferenceId` is NOT mandatory — every
        // live sandbox lookup omitted it and succeeded on `browserInformation` alone. Send both
        // when both are available: `dfReferenceId` is the authoritative join key and
        // `browserInformation` is corroborating detail.
        let df_reference_id = extract_df_reference_id(request.redirect_response.as_ref());
        let browser_information = build_lookup_browser_information(request.browser_info.as_ref());

        if df_reference_id.is_none() && browser_information.is_none() {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "browser_info",
                    context: domain_types::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "A 3D Secure lookup with no device data at all authenticates far more \
                             weakly and makes the liability-shift outcome misleading."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Send either `redirection_response.payload.dfReferenceId` (the \
                             output of the browser's `threeDSecure.prepareLookup`, preferred) or \
                             `browser_info`."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://developer.paypal.com/braintree/docs/guides/3d-secure/server-side"
                                .to_string(),
                        ),
                    },
                }
            ));
        }

        let transaction_information = ThreeDSecureLookupTransactionInformationInput {
            // When the caller does not state a channel, only assert BROWSER if browser data
            // actually evidences one; otherwise omit rather than claim a channel.
            device_channel: request
                .device_channel
                .map(ThreeDSecureDeviceChannel::from)
                .or_else(|| {
                    browser_information
                        .is_some()
                        .then_some(ThreeDSecureDeviceChannel::Browser)
                }),
            email: request.email.clone(),
            ip_address: request
                .browser_info
                .as_ref()
                .and_then(|info| info.ip_address)
                .map(|ip| Secret::new(ip.to_string())),
            browser_information,
        };
        let transaction_information =
            (!transaction_information.is_empty()).then_some(transaction_information);

        let cardholder_information = build_lookup_billing_address(
            item.router_data
                .resource_common_data
                .get_optional_payment_billing()
                .or_else(|| item.router_data.resource_common_data.get_optional_billing()),
        );

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

// ---- response ------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureLookupData {
    /// Non-null ONLY on a challenge. See `AuthenticateOutcome`.
    acs_url: Option<String>,
    authentication_id: Option<String>,
    version: Option<String>,
    /// Named `pareq` for 3DS1 legacy reasons; under 3DS2 it carries a base64 CReq. Passed
    /// through opaquely — never decoded, re-encoded or re-derived.
    pareq: Option<String>,
    /// Always exactly `authenticationId` in every observed response. Echoed, never synthesised:
    /// if Braintree ever diverges the two, an echo is still correct and a synthesis is not.
    md: Option<String>,
    /// SENSITIVE: the URL embeds an `authorization_fingerprint` JWT, so the whole string is a
    /// credential. It has to travel in `RedirectForm::Form::form_fields`, a plain
    /// `HashMap<String, String>` with no masking — keep it out of `tracing` output.
    term_url: Option<Secret<String>>,
    transaction_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureAuthenticationDetails {
    /// The 3DS cryptogram. `Secret` on arrival and for its whole life.
    cavv: Option<Secret<String>>,
    /// Two-character ECI, network-scoped (Visa 05/06/07, Mastercard 02/01/00). Kept a `String`
    /// precisely so nothing reinterprets or normalises it across networks.
    eci_flag: Option<String>,
    /// READ this; never infer it from the status. `DATA_ONLY_SUCCESSFUL` is a success with no
    /// liability shift, and the two failure statuses differ only in `liabilityShiftPossible`.
    liability_shifted: Option<bool>,
    liability_shift_possible: Option<bool>,
    card_enrolled: Option<ThreeDSecureCardEnrolled>,
    authentication_status: Option<ThreeDSecureAuthenticationStatus>,
    version: Option<String>,
    directory_server_transaction_id: Option<String>,
    x_id: Option<String>,
    three_d_secure_server_transaction_id: Option<String>,
    acs_transaction_id: Option<String>,
    pares_status: Option<ThreeDSecureAuthenticationStatusIndicator>,
    transaction_status: Option<ThreeDSecureAuthenticationStatusIndicator>,
    transaction_status_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSecureDetails {
    authentication: Option<ThreeDSecureAuthenticationDetails>,
}

/// The `... on CreditCardDetails` fragment. A non-`CreditCardDetails` union member yields `{}`
/// rather than an error, so every member is optional and an absent `threeDSecure` has to be
/// detected explicitly.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupPaymentMethodDetails {
    bin: Option<String>,
    last4: Option<String>,
    brand_code: Option<String>,
    three_d_secure: Option<ThreeDSecureDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupPaymentMethod {
    /// The NEW single-use nonce. See the FINDING 3 comment on the response transformer.
    id: Secret<String>,
    details: Option<LookupPaymentMethodDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformThreeDSecureLookupPayload {
    /// Returned NON-NULL on every outcome, frictionless success included — which is exactly why
    /// it must never be used as the challenge discriminator.
    three_d_secure_lookup_data: Option<ThreeDSecureLookupData>,
    payment_method: Option<LookupPaymentMethod>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformThreeDSecureLookupData {
    perform_three_d_secure_lookup: Option<PerformThreeDSecureLookupPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeThreeDSecureLookupSuccess {
    data: PerformThreeDSecureLookupData,
}

/// Braintree answers HTTP 200 for everything, so success and failure are separated by body shape.
///
/// `ErrorResponse` MUST be listed first, and MUST require `errors`. Both real Braintree error
/// bodies carry a `data` key (`{"data":{"performThreeDSecureLookup":null}}`), so an untagged enum
/// that tries the success variant first — or that discriminates on the presence of `data` —
/// misclassifies a hard error as a success with a null payload. Schema-validation errors carry no
/// `data` key at all, which is why the error variant must not require one.
///
/// `GenericBraintreeResponse<T>` is deliberately NOT reused here: it lists `SuccessResponse`
/// first, which is safe for the flows that use it and wrong for this one.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreeAuthenticateResponse {
    ErrorResponse(Box<ErrorResponse>),
    AuthenticateResponse(Box<BraintreeThreeDSecureLookupSuccess>),
}

/// `ThreeDSecureCardEnrolled`. Informational only — it reaches the caller untranslated in
/// `connector_feature_data` and no status is ever derived from it.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
// `Display` must match the serde spelling: these values reach the caller verbatim in
// `connector_feature_data` as "the raw Braintree string", and a PascalCase Rust variant
// name there would be a value Braintree never emitted.
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureCardEnrolled {
    Bypass,
    Error,
    No,
    Unavailable,
    Yes,
    #[serde(other)]
    Unknown,
}

/// `ThreeDSecureAuthenticationStatusIndicator` — the 3DS `transStatus` letter codes, spelled out.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
// `Display` must match the serde spelling: these values reach the caller verbatim in
// `connector_feature_data` as "the raw Braintree string", and a PascalCase Rust variant
// name there would be a value Braintree never emitted.
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureAuthenticationStatusIndicator {
    SuccessfulAuthentication,
    FailedAuthentication,
    UnableToCompleteAuthentication,
    SuccessfulAttemptsTransaction,
    AuthenticationRejected,
    ChallengeRequiredForAuthentication,
    ChallengeRequiredDecoupledAuthentication,
    InformationalChallengePreferenceAcknowledged,
    /// Maps to `None`, never to `TransactionStatus::Failure`. `TransactionStatus` derives
    /// `Default = Failure`, so anything that reaches for a default on this path silently reports
    /// a failed authentication.
    #[serde(other)]
    Unknown,
}

impl ThreeDSecureAuthenticationStatusIndicator {
    /// Exact 8-to-8 mapping onto `common_enums::TransactionStatus`, so no information is lost.
    fn to_trans_status(self) -> Option<common_enums::TransactionStatus> {
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

/// `ThreeDSecureAuthenticationStatus`.
///
/// The served schema at the pinned `Braintree-Version` exposes 25 values. The four arms marked
/// REMOVED below were `@deprecated` in the master SDL and are no longer served at all; their arms
/// are kept because this enum is deserialize-only on this flow (none of these values is ever
/// *sent*), so an arm for a value the server never emits is simply dead — and if Braintree ever
/// restores one, the parse still works. They are **unreachable today**.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
// `Display` must match the serde spelling: these values reach the caller verbatim in
// `connector_feature_data` as "the raw Braintree string", and a PascalCase Rust variant
// name there would be a value Braintree never emitted.
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDSecureAuthenticationStatus {
    AuthenticateSuccessful,
    AuthenticateAttemptSuccessful,
    AuthenticateFrictionlessFailed,
    AuthenticateFailed,
    AuthenticateFailedAcsError,
    AuthenticateRejected,
    AuthenticateError,
    AuthenticateUnableToAuthenticate,
    ChallengeRequired,
    DataOnlySuccessful,
    ExemptionLowValueSuccessful,
    ExemptionTraSuccessful,
    LookupNotEnrolled,
    LookupBypassed,
    SkippedDueToRule,
    SkippedDueToAdaptiveAuthentication,
    AuthenticationUnavailable,
    LookupError,
    LookupCardError,
    LookupServerError,
    LookupFailedAcsError,
    MpiServerError,
    UnsupportedCard,
    UnsupportedAccountType,
    #[serde(rename = "UNSUPPORTED_THREE_D_SECURE_VERSION")]
    UnsupportedThreeDSecureVersion,
    // --- REMOVED from the served schema; arms kept, unreachable today -----------------------
    AuthenticateSignatureVerificationFailed,
    AuthenticateSuccessfulIssuerNotParticipating,
    AuthenticationBypassed,
    LookupEnrolled,
    /// Braintree adds 3DS authentication statuses over time (4 arrived in 2022-09-30, 4 more in
    /// 2023-05-23). Deserializing an unrecognised one into a catch-all keeps a single new status
    /// from failing the whole response parse; it maps to `AttemptStatus::Unspecified` so the
    /// caller applies its own previous-status fallback rather than UCS inventing a Pending or a
    /// Failure it cannot substantiate.
    #[serde(other)]
    Unknown,
}

impl From<ThreeDSecureAuthenticationStatus> for enums::AttemptStatus {
    fn from(status: ThreeDSecureAuthenticationStatus) -> Self {
        match status {
            // Advanceable: authentication is settled and Braintree's Authorize can spend the
            // returned nonce. Liability shift is reported separately and must be READ, not
            // inferred from any of these — `DATA_ONLY_SUCCESSFUL` shifts nothing.
            ThreeDSecureAuthenticationStatus::AuthenticateSuccessful
            | ThreeDSecureAuthenticationStatus::AuthenticateAttemptSuccessful
            | ThreeDSecureAuthenticationStatus::DataOnlySuccessful
            | ThreeDSecureAuthenticationStatus::ExemptionLowValueSuccessful
            | ThreeDSecureAuthenticationStatus::ExemptionTraSuccessful
            | ThreeDSecureAuthenticationStatus::LookupNotEnrolled
            | ThreeDSecureAuthenticationStatus::LookupBypassed
            | ThreeDSecureAuthenticationStatus::SkippedDueToRule
            | ThreeDSecureAuthenticationStatus::SkippedDueToAdaptiveAuthentication
            | ThreeDSecureAuthenticationStatus::AuthenticateSuccessfulIssuerNotParticipating
            | ThreeDSecureAuthenticationStatus::AuthenticationBypassed => {
                Self::AuthenticationSuccessful
            }
            // Non-terminal, and the ONLY value that means "render the ACS challenge". It must
            // stay non-terminal or the challenge can never complete.
            ThreeDSecureAuthenticationStatus::ChallengeRequired
            | ThreeDSecureAuthenticationStatus::LookupEnrolled => Self::AuthenticationPending,
            // Terminal for this attempt. `AttemptStatus::is_terminal_status()` reports
            // `AuthenticationFailed` terminal, so nothing here polls forever.
            ThreeDSecureAuthenticationStatus::AuthenticateFrictionlessFailed
            | ThreeDSecureAuthenticationStatus::AuthenticateFailed
            | ThreeDSecureAuthenticationStatus::AuthenticateFailedAcsError
            | ThreeDSecureAuthenticationStatus::AuthenticateRejected
            | ThreeDSecureAuthenticationStatus::AuthenticateError
            | ThreeDSecureAuthenticationStatus::AuthenticateUnableToAuthenticate
            | ThreeDSecureAuthenticationStatus::AuthenticationUnavailable
            | ThreeDSecureAuthenticationStatus::LookupError
            | ThreeDSecureAuthenticationStatus::LookupCardError
            | ThreeDSecureAuthenticationStatus::LookupServerError
            | ThreeDSecureAuthenticationStatus::LookupFailedAcsError
            | ThreeDSecureAuthenticationStatus::MpiServerError
            | ThreeDSecureAuthenticationStatus::UnsupportedCard
            | ThreeDSecureAuthenticationStatus::UnsupportedAccountType
            | ThreeDSecureAuthenticationStatus::UnsupportedThreeDSecureVersion
            | ThreeDSecureAuthenticationStatus::AuthenticateSignatureVerificationFailed => {
                Self::AuthenticationFailed
            }
            ThreeDSecureAuthenticationStatus::Unknown => Self::Unspecified,
        }
    }
}

/// Which of the two branches the lookup landed on.
///
/// FINDING 2, and the single most likely way to get this flow wrong: `threeDSecureLookupData` is
/// returned NON-NULL on every outcome — frictionless success and outright failure included. On
/// those outcomes `acsUrl` and `pareq` are null while `authenticationId`, `md`, `termUrl`,
/// `transactionId` and `version` are still populated. Discriminating on
/// `three_d_secure_lookup_data.is_some()` would therefore emit a redirect on EVERY outcome and
/// strand a frictionless payment in a challenge that does not exist.
///
/// The discriminator is belt-and-braces: `authenticationStatus == CHALLENGE_REQUIRED` is the
/// semantic signal, `acsUrl.is_some()` the structural one, and a `RedirectForm` cannot be built
/// without a non-null `acsUrl` anyway. A `CHALLENGE_REQUIRED` carrying a null `acsUrl` was never
/// observed; if it ever happens it is an `UnexpectedResponseError`, never a silent frictionless
/// success.
fn is_braintree_challenge(
    authentication_status: Option<ThreeDSecureAuthenticationStatus>,
    acs_url: Option<&String>,
) -> bool {
    matches!(
        authentication_status,
        Some(ThreeDSecureAuthenticationStatus::ChallengeRequired)
    ) && acs_url.is_some()
}

/// Build the ACS step-up form for a `CHALLENGE_REQUIRED` outcome.
///
/// `RedirectForm::Form` is used rather than a new variant: the `Authenticate` response mapper
/// (`domain_types::types`) accepts exactly seven `RedirectForm` variants and turns everything else
/// — `RedirectForm::Braintree`, which leg 1 uses, included — into an `UnexpectedResponseError`.
/// `Form` carries this payload exactly, so the blast radius outside the connector is zero: no
/// change to `types.rs`, `router_response_types.rs` or `proto/`.
///
/// `PaReq` / `MD` / `TermUrl` are Braintree's published field names for a server-side-lookup
/// step-up form. `pareq` is named for 3DS1 legacy reasons but carries a base64 3DS2 CReq under
/// 3DS2; it is passed through opaquely — never decoded, re-encoded or re-derived.
///
/// SENSITIVE: `termUrl` embeds a signed `authorization_fingerprint` JWT, so the whole URL is a
/// credential. `form_fields` is an unmasked `HashMap<String, String>`, so this value WILL appear
/// in any raw request/response log. That is a known, accepted limitation of the variant — do not
/// log it deliberately, and do not widen it.
fn build_challenge_redirect_form(
    lookup: Option<&ThreeDSecureLookupData>,
    http_code: u16,
) -> Result<RedirectForm, Report<ConnectorError>> {
    let missing = |field: &'static str| {
        utils::unexpected_response_fail(
            http_code,
            format!("Braintree reported CHALLENGE_REQUIRED with a null {field}"),
        )
    };
    let lookup = lookup.ok_or_else(|| missing("threeDSecureLookupData"))?;
    Ok(RedirectForm::Form {
        endpoint: lookup.acs_url.clone().ok_or_else(|| missing("acsUrl"))?,
        method: common_utils::Method::Post,
        form_fields: std::collections::HashMap::from([
            (
                "PaReq".to_string(),
                lookup.pareq.clone().ok_or_else(|| missing("pareq"))?,
            ),
            // Echoed, not synthesised from `authenticationId`, even though the two have always
            // been equal in every observed response.
            (
                "MD".to_string(),
                lookup.md.clone().ok_or_else(|| missing("md"))?,
            ),
            (
                "TermUrl".to_string(),
                lookup
                    .term_url
                    .clone()
                    .ok_or_else(|| missing("termUrl"))?
                    .expose(),
            ),
        ]),
    })
}

/// `ThreeDSecureAuthentication` -> `router_request_types::AuthenticationData`.
///
/// Shared by leg 2 (`Authenticate`, the `performThreeDSecureLookup` payload) and leg 3
/// (`PostAuthenticate`, the `node(id:)` readback): both legs read the SAME Braintree type out of
/// the same `CreditCardDetails.threeDSecure.authentication` block, so they map it with the same
/// function rather than with two copies that can drift apart.
///
/// `lookup` is the leg-2 `threeDSecureLookupData` block and is `None` on leg 3 — `node(id:)` does
/// not return one. It contributes only the lookup's own `transactionId` and a `version` fallback.
fn build_three_ds_authentication_data(
    authentication: &ThreeDSecureAuthenticationDetails,
    lookup: Option<&ThreeDSecureLookupData>,
) -> router_request_types::AuthenticationData {
    router_request_types::AuthenticationData {
        // From `transactionStatus`, not `paresStatus`: the latter was null on the
        // challenge capture and merely duplicates the former on the successes.
        // Never defaulted — `TransactionStatus` derives `Default = Failure`.
        trans_status: authentication
            .transaction_status
            .and_then(ThreeDSecureAuthenticationStatusIndicator::to_trans_status),
        eci: authentication.eci_flag.clone(),
        // `None` on a challenge is correct and expected, not an error.
        cavv: authentication.cavv.clone(),
        // Braintree exposes no Mastercard UCAF member; do not derive one from the ECI.
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: authentication.three_d_secure_server_transaction_id.clone(),
        // Braintree returns a full three-part semver ("2.1.0"). A parse failure
        // degrades to `None` — it must never abort the whole response.
        message_version: authentication
            .version
            .clone()
            .or_else(|| lookup.and_then(|data| data.version.clone()))
            .and_then(|version| {
                <common_utils::types::SemanticVersion as std::str::FromStr>::from_str(&version).ok()
            }),
        // NEVER cross-wire this into `ThreeDSecurePassThroughInput.dsTransactionId` —
        // that is the external-MPI path and the two topologies must not meet.
        ds_trans_id: authentication.directory_server_transaction_id.clone(),
        acs_transaction_id: authentication.acs_transaction_id.clone(),
        // The LOOKUP's transaction id, from the lookup-data block — distinct from
        // `xId`, the legacy Cardinal XID, which goes to `connector_feature_data`.
        transaction_id: lookup.and_then(|data| data.transaction_id.clone()),
        // Cartes Bancaires specific; Braintree returns none of the three members.
        network_params: None,
        // Braintree signals an applied exemption through `authenticationStatus`
        // (`EXEMPTION_*_SUCCESSFUL`), not as a separate field. Deriving the enum from
        // the status would be a compliance *claim* UCS is not entitled to make.
        exemption_indicator: None,
        // No timestamp on the payload; `now()` would be a fabrication.
        created_at: None,
        // CRes/RReq fields from a completed challenge. `None` on BOTH legs: on leg 2 the
        // challenge has not happened yet, and on leg 3 Braintree exposes no CRes/RReq member
        // anywhere on `ThreeDSecureAuthentication` (all 14 of its members are modelled above).
        // `transactionStatusReason` is the closest thing and it is a Braintree-scoped reason
        // string, not a 3DS `challengeCode` — mapping it here would be a fabricated claim, so
        // it goes to `connector_feature_data` instead.
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        // Decoupled authentication already reaches the caller through `trans_status`
        // (`ChallengeRequiredDecoupledAuthentication`); a second, weaker encoding
        // would be redundant.
        authentication_type: None,
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreeAuthenticateResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreeAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreeAuthenticateResponse::ErrorResponse(error_response) => Ok(Self {
                // No status write on the error path: the shared Braintree error builder is
                // flow-agnostic (Refund, RSync and Capture route through it too), so an
                // `AttemptStatus` written here would be read as a terminal refund failure on a
                // refund transport error.
                response: build_error_response::<PaymentsResponseData>(
                    error_response.errors.as_ref(),
                    item.http_code,
                )
                .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreeAuthenticateResponse::AuthenticateResponse(success) => {
                let payload = success.data.perform_three_d_secure_lookup.ok_or_else(|| {
                    utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree returned a null performThreeDSecureLookup payload with no \
                             errors",
                    )
                })?;

                let payment_method = payload.payment_method.ok_or_else(|| {
                    utils::unexpected_response_fail(
                        item.http_code,
                        "Braintree returned a 3D Secure lookup with no paymentMethod",
                    )
                })?;

                // The `... on CreditCardDetails` fragment yields `{}` on a non-credit-card union
                // member rather than failing, so an absent authentication block has to be
                // detected explicitly instead of being inferred from a parse failure.
                let authentication = payment_method
                    .details
                    .as_ref()
                    .and_then(|details| details.three_d_secure.as_ref())
                    .and_then(|three_ds| three_ds.authentication.as_ref())
                    .ok_or_else(|| {
                        utils::unexpected_response_fail(
                            item.http_code,
                            "Braintree returned a 3D Secure lookup with no CreditCardDetails \
                             authentication block — the instrument is not a credit card",
                        )
                    })?;

                let lookup = payload.three_d_secure_lookup_data;
                let acs_url = lookup.as_ref().and_then(|data| data.acs_url.as_ref());
                let is_challenge =
                    is_braintree_challenge(authentication.authentication_status, acs_url);

                let redirection_data = is_challenge
                    .then(|| build_challenge_redirect_form(lookup.as_ref(), item.http_code))
                    .transpose()?
                    .map(Box::new);

                let authentication_id = lookup
                    .as_ref()
                    .and_then(|data| data.authentication_id.clone());
                let lookup_transaction_id =
                    lookup.as_ref().and_then(|data| data.transaction_id.clone());

                let authentication_data =
                    build_three_ds_authentication_data(authentication, lookup.as_ref());

                // FINDING 3, live-confirmed on all four captured outcomes: the lookup ALWAYS
                // consumes the input nonce and returns a DIFFERENT `paymentMethod.id`
                // (`tokencc_bh_…` in, a bare UUID out). Replaying the old nonce returns "Nonce is
                // already consumed". The subsequent charge must spend the NEW one, so it leaves
                // this flow by the most prominent channel — `resource_id` — and is mirrored into
                // `connector_feature_data` so an orchestrator that treats `resource_id` strictly
                // as a transaction id still does not lose it.
                let new_payment_method_id = payment_method.id.expose();

                // The marker key is shared with leg 3 (PostAuthenticate) and with the Authorize
                // builder's Braintree-hosted-vs-external-MPI discriminator, so all three read it
                // from one constant and cannot drift apart.
                let connector_feature_data = serde_json::Value::Object(
                    [(
                        constants::BRAINTREE_THREE_DS_FEATURE_KEY.to_string(),
                        serde_json::json!({
                        "payment_method_id": new_payment_method_id,
                        "authentication_id": authentication_id,
                        "lookup_transaction_id": lookup_transaction_id,
                        "liability_shifted": authentication.liability_shifted,
                        "liability_shift_possible": authentication.liability_shift_possible,
                        "card_enrolled": authentication.card_enrolled.map(|value| value.to_string()),
                        "authentication_status": authentication
                            .authentication_status
                            .map(|value| value.to_string()),
                        "pares_status": authentication.pares_status.map(|value| value.to_string()),
                        "transaction_status_reason": authentication.transaction_status_reason,
                        "x_id": authentication.x_id,
                        "bin": payment_method.details.as_ref().and_then(|d| d.bin.clone()),
                        "last4": payment_method.details.as_ref().and_then(|d| d.last4.clone()),
                        "brand_code": payment_method
                            .details
                            .as_ref()
                            .and_then(|d| d.brand_code.clone()),
                        }),
                    )]
                    .into_iter()
                    .collect(),
                );

                let status = authentication
                    .authentication_status
                    .map(enums::AttemptStatus::from)
                    .unwrap_or(enums::AttemptStatus::Unspecified);

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        // The same value as `connector_response_reference_id`, so a later flow can
                        // find it without unpacking `connector_feature_data`.
                        reference_id: authentication_id.clone(),
                        ..item.router_data.resource_common_data.clone()
                    },
                    response: Ok(PaymentsResponseData::AuthenticateResponse {
                        resource_id: Some(ResponseId::ConnectorTransactionId(
                            new_payment_method_id,
                        )),
                        redirection_data,
                        authentication_data: Some(authentication_data),
                        connector_feature_data: Some(connector_feature_data),
                        // Braintree's own reference for this authentication — what a support
                        // ticket or a PostAuthenticate read quotes. Never synthesised.
                        connector_response_reference_id: authentication_id,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

// ==============================================================================================
// Braintree-hosted 3D Secure, leg 3: PostAuthenticate — the `node(id:)` readback
// ==============================================================================================

/// GraphQL `$id: ID!`.
///
/// Every other Braintree flow sends `{ "variables": { "input": … } }`; this one does not, because
/// `node(id:)` takes a bare id argument. `GenericVariableInput<T>` is therefore deliberately NOT
/// reused — it would wrap the id in an `input` object the document never declares.
#[derive(Debug, Clone, Serialize)]
pub struct BraintreePostAuthenticateVariables {
    /// `Secret` because a Braintree payment-method id is a spendable payment credential: a bare
    /// `paymentMethodId` is all `chargeCreditCard` needs.
    id: Secret<String>,
}

pub type BraintreePostAuthenticateRequest =
    GenericBraintreeRequest<BraintreePostAuthenticateVariables>;

/// The one member of `PaymentsPostAuthenticateData` this leg reads.
fn post_authenticate_payment_method_id<T: PaymentMethodDataTypes>(
    request: &PaymentsPostAuthenticateData<T>,
) -> Result<Secret<String>, Report<IntegrationError>> {
    // This leg reads EXACTLY ONE member of `PaymentsPostAuthenticateData`, and that is not an
    // oversight:
    //
    // * `redirect_response` is deliberately NOT read, and `get_redirect_response_payload()`
    //   is deliberately NOT called. The ACS posts its PaRes to BRAINTREE's own `termUrl`
    //   (leg 2 put it in the step-up form), not to UCS, and Braintree then 302s the browser
    //   to a page on `assets.braintreegateway.com`. The browser therefore comes back to the
    //   caller carrying nothing UCS needs. `get_redirect_response_payload()` raises
    //   `MissingRequiredField` on `None`, so calling it would fail every Braintree
    //   post-challenge request. This is a real divergence from most PostAuthenticate
    //   implementations, which ingest a CRes — do not "fix" it.
    // * `payment_method_data` is NOT read: on the composite path it still holds the ORIGINAL
    //   instrument, and leg 2 consumed that nonce. Reading it would send a spent nonce and
    //   `node(id:)` would answer NOT_FOUND.
    // * `amount` / `currency` are NOT read and `amount_converter` is NOT invoked: no money
    //   moves on a readback and `node(id: ID!)` takes one argument.
    // * No `merchant_account_id` is resolved: `node(id:)` has no merchant-scoped argument —
    //   the Basic-auth credential pair IS the merchant scope.
    request
        .connector_order_reference_id
        .clone()
        .map(Secret::new)
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_order_reference_id",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Braintree's PostAuthenticate reads the settled 3D Secure authentication \
                     back off the payment method that the Authenticate leg returned, so it \
                     needs that payment-method id and nothing else."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Send the `connector_transaction_id` returned by \
                     PaymentMethodAuthenticationService/Authenticate as \
                     `connector_order_reference_id`. Do not send the nonce that went INTO \
                     the lookup — the lookup consumes it."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Query--node".to_string(),
                    ),
                },
            })
        })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BraintreeRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
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
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method_id = post_authenticate_payment_method_id(&item.router_data.request)?;

        Ok(Self {
            query: constants::POST_AUTHENTICATE_QUERY.to_string(),
            // Verbatim: base64 and Relay-style "PaymentMethod:<id>" global ids both answer
            // NOT_FOUND. There is no encoding step here and there must never be one.
            variables: BraintreePostAuthenticateVariables {
                id: payment_method_id,
            },
        })
    }
}

// ---- response ------------------------------------------------------------------------------

/// `PaymentMethodUsage`. A single-use nonce is spent by the first charge; a multi-use one is a
/// vaulted instrument. Informational on this leg — it is surfaced in the typed connector
/// response and nothing branches on it.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreePaymentMethodUsage {
    SingleUse,
    MultiUse,
    #[serde(other)]
    Unknown,
}

/// The `... on PaymentMethod` fragment of the `node(id:)` readback.
///
/// `node` returns the `Node` INTERFACE, which has **30** possible types. If the id belongs to one
/// of the other 29 (a Transaction, a Refund, a Customer, …) the inline fragment simply
/// contributes nothing rather than failing, so every member below is optional and a wrong-type
/// node has to be detected explicitly.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeNodeReadback {
    /// Echoed back verbatim — the same spendable payment-method id that was sent.
    id: Option<Secret<String>>,
    legacy_id: Option<String>,
    usage: Option<BraintreePaymentMethodUsage>,
    /// The `PaymentMethodDetails` union. Reused from leg 2: both legs select the identical
    /// `... on CreditCardDetails` fragment, so they share one struct.
    details: Option<LookupPaymentMethodDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeThreeDSecureResultData {
    /// Nullable: a NOT_FOUND answer is `{"data":{"node":null},"errors":[…]}`.
    node: Option<BraintreeNodeReadback>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeThreeDSecureResultSuccess {
    data: BraintreeThreeDSecureResultData,
}

/// Braintree answers HTTP 200 for everything, so success and failure are separated by body shape.
///
/// `ErrorResponse` MUST be listed first, and MUST require `errors`. The live NOT_FOUND body
/// carries BOTH a populated `errors[]` and a `data` key
/// (`{"data":{"node":null},"errors":[{"message":"An object with this ID was not found.",…}]}`),
/// so an untagged enum that tried the success variant first — or that discriminated on the
/// presence of `data` — would read a hard error as a success with a null node. Schema-validation
/// errors carry no `data` key at all, which is why the error variant must not require one.
///
/// `GenericBraintreeResponse<T>` is deliberately NOT reused here: it lists `SuccessResponse`
/// first, which is safe for the flows that use it and wrong for this one. Same call as leg 2.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BraintreePostAuthenticateResponse {
    ErrorResponse(Box<ErrorResponse>),
    PostAuthenticateResponse(Box<BraintreeThreeDSecureResultSuccess>),
}

/// The `braintree_three_ds` blob the verified nonce and the Braintree-only 3DS signals leave by.
///
/// `PostAuthenticateResponse` has no `resource_id` and no `connector_feature_data` member, but the
/// gRPC response builder sources `connector_feature_data` from `resource_common_data`
/// independently of the response variant — so writing it there surfaces the blob to the caller
/// with no `types.rs` and no `proto/` change. The composite dispatcher then forwards it into the
/// Authorize request, where the marker key is ALSO the Braintree-hosted-vs-external-MPI
/// discriminator (`braintree_authorize_three_ds_mode`) — do not rename it without updating that.
///
/// Leg 2's `authentication_id` and `lookup_transaction_id` are deliberately not re-emitted:
/// `node(id:)` returns neither, and the dispatcher replaces `connector_feature_data` wholesale
/// rather than merging. Nothing reads them on the Authorize path, so the drop is accepted; if they
/// are ever needed the fix is to merge, and it belongs in the connector, not the dispatcher.
fn build_post_authenticate_feature_data(
    payment_method_id: Option<&String>,
    authentication: &ThreeDSecureAuthenticationDetails,
    details: &LookupPaymentMethodDetails,
) -> serde_json::Value {
    serde_json::Value::Object(
        [(
            constants::BRAINTREE_THREE_DS_FEATURE_KEY.to_string(),
            serde_json::json!({
                // The 3DS-verified nonce the subsequent Authorize MUST spend. This variant has no
                // `resource_id`, so this is the only channel it can leave by.
                "payment_method_id": payment_method_id,
                // Liability shift is READ, never inferred from the status: `DATA_ONLY_SUCCESSFUL`
                // is a success that shifts nothing, and the failure statuses differ only in
                // whether a retry could still shift.
                "liability_shifted": authentication.liability_shifted,
                "liability_shift_possible": authentication.liability_shift_possible,
                "card_enrolled": authentication.card_enrolled.map(|value| value.to_string()),
                "authentication_status": authentication
                    .authentication_status
                    .map(|value| value.to_string()),
                "pares_status": authentication.pares_status.map(|value| value.to_string()),
                // The closest thing Braintree has to a challenge reason code, and the reason it
                // is NOT mapped onto `AuthenticationData.challenge_code`: it is a Braintree-scoped
                // reason string, not a 3DS `challengeCode`.
                "transaction_status_reason": authentication.transaction_status_reason.clone(),
                "x_id": authentication.x_id.clone(),
                "bin": details.bin.clone(),
                "last4": details.last4.clone(),
                "brand_code": details.brand_code.clone(),
            }),
        )]
        .into_iter()
        .collect(),
    )
}

/// Walk the `node(id:)` readback down to the settled authentication, naming each way it can fail.
///
/// Every level is nullable and NONE of them fails the GraphQL request, so each has to be detected
/// explicitly rather than inferred from a parse failure:
///
/// * `node` is null when the id resolved to nothing (that body also carries `errors[]`, so it is
///   normally caught by the error arm first).
/// * the `... on PaymentMethod` fragment contributes nothing when the id belongs to one of the
///   other 29 `Node` types — a Transaction, a Refund, a Customer.
/// * the `... on CreditCardDetails` fragment yields `{}` on a non-card payment method.
/// * `threeDSecure` / `authentication` are null when the payment method exists but no 3D Secure
///   lookup was ever run against it. **That is not a frictionless success and must never be
///   reported as one.**
fn resolve_post_authenticate_readback(
    node: Option<&BraintreeNodeReadback>,
    http_code: u16,
) -> Result<
    (
        &LookupPaymentMethodDetails,
        &ThreeDSecureAuthenticationDetails,
    ),
    ConnectorError,
> {
    let node = node.ok_or_else(|| {
        utils::unexpected_response_fail(
            http_code,
            "Braintree returned a null node for the 3D Secure readback with no errors",
        )
    })?;

    let details = node.details.as_ref().ok_or_else(|| {
        utils::unexpected_response_fail(
            http_code,
            "Braintree resolved the 3D Secure readback id to a node that is not a PaymentMethod \
             — `connector_order_reference_id` must carry the payment-method id returned by the \
             Authenticate leg",
        )
    })?;

    let authentication = details
        .three_d_secure
        .as_ref()
        .and_then(|three_d_secure| three_d_secure.authentication.as_ref())
        .ok_or_else(|| {
            if details.bin.is_none() && details.brand_code.is_none() {
                utils::unexpected_response_fail(
                    http_code,
                    "Braintree returned a 3D Secure readback with an empty CreditCardDetails \
                     fragment — the instrument is not a credit card",
                )
            } else {
                utils::unexpected_response_fail(
                    http_code,
                    "Braintree returned a credit card with no threeDSecure authentication — no \
                     3D Secure lookup was ever run against this payment method. This is NOT a \
                     frictionless success and must never be reported as one",
                )
            }
        })?;

    Ok((details, authentication))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BraintreePostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<BraintreePostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            BraintreePostAuthenticateResponse::ErrorResponse(error_response) => Ok(Self {
                // No status write on the error path: the shared Braintree error builder is
                // flow-agnostic (Refund, RSync and Capture route through it too), so an
                // `AttemptStatus` written here would be read as a terminal refund failure on a
                // refund transport error.
                response: build_error_response::<PaymentsResponseData>(
                    error_response.errors.as_ref(),
                    item.http_code,
                )
                .map_err(|err| *err),
                ..item.router_data
            }),
            BraintreePostAuthenticateResponse::PostAuthenticateResponse(success) => {
                let (details, authentication) =
                    resolve_post_authenticate_readback(success.data.node.as_ref(), item.http_code)?;

                // `node(id:)` returns no `threeDSecureLookupData`, so there is no lookup block to
                // pass: `transaction_id` and the `version` fallback come from it and are simply
                // absent on this leg. The same mapper as leg 2 — never a second copy.
                let authentication_data = build_three_ds_authentication_data(authentication, None);

                // Unlike leg 2, a settled `AUTHENTICATE_SUCCESSFUL` is expected to carry a CAVV.
                // Braintree is the authority and `DATA_ONLY_SUCCESSFUL` legitimately has none, so
                // this is never an error — but it is worth saying out loud.
                if authentication.cavv.is_none()
                    && matches!(
                        authentication.authentication_status,
                        Some(ThreeDSecureAuthenticationStatus::AuthenticateSuccessful)
                    )
                {
                    tracing::warn!(
                        target: "braintree_three_ds",
                        "Braintree reported AUTHENTICATE_SUCCESSFUL on the PostAuthenticate \
                         readback with no CAVV"
                    );
                }

                // The id that was read. `Node.id` is non-null in the schema and is echoed
                // verbatim, so it is preferred over the request copy; the fallback exists only so
                // a null can never cost the caller the one id it needs next.
                let payment_method_id = success
                    .data
                    .node
                    .as_ref()
                    .and_then(|node| node.id.as_ref())
                    .map(|id| id.peek().clone())
                    .or_else(|| {
                        item.router_data
                            .request
                            .connector_order_reference_id
                            .clone()
                    });

                // `PostAuthenticateResponse` has no `resource_id` and no `connector_feature_data`
                // member, so the 3DS-verified nonce leaves this flow through
                // `resource_common_data.connector_feature_data`, which the response builder
                // sources independently of the response variant. The composite dispatcher then
                // forwards it into the Authorize request, where the marker key below is ALSO the
                // Braintree-hosted-vs-external-MPI discriminator — do not rename it without
                // updating that guard.
                //
                // `authentication_id` and `lookup_transaction_id` from leg 2's blob are not
                // re-emitted: `node(id:)` does not return either, and the dispatcher replaces
                // `connector_feature_data` wholesale rather than merging. Nothing reads them on
                // the Authorize path; the drop is deliberate.
                let connector_feature_data = build_post_authenticate_feature_data(
                    payment_method_id.as_ref(),
                    authentication,
                    details,
                );

                // The §A.7 table, verbatim — the same enum and the same `From` impl leg 2 uses,
                // keyed on the same Braintree field of the same Braintree type. A second copy of
                // a 25-arm mapping is exactly how two legs drift apart.
                //
                // `CHALLENGE_REQUIRED` stays NON-TERMINAL here: it means the caller ran the
                // readback before Braintree finished processing the PaRes, or the cardholder
                // abandoned the challenge. The readback is idempotent and non-consuming, so the
                // honest answer is "still pending, call again" — never a synthesised timeout.
                let status = authentication
                    .authentication_status
                    .map(enums::AttemptStatus::from)
                    .unwrap_or(enums::AttemptStatus::Unspecified);

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        reference_id: payment_method_id.clone(),
                        connector_feature_data: Some(Secret::new(connector_feature_data)),
                        ..item.router_data.resource_common_data.clone()
                    },
                    response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                        // The whole point of the leg. `TransactionResponse` would have carried
                        // the nonce in a first-class field but sets `authentication_data: None`,
                        // discarding the CAVV, ECI, DS-Trans-ID and trans-status — i.e. every
                        // output of the 3DS trio.
                        authentication_data: Some(authentication_data),
                        // This variant has no `resource_id`; PostAuthenticate is the terminal
                        // step of the trio, so the id the caller needs next — the spendable one —
                        // rides here. Deliberately NOT `authenticationId`, which `node(id:)` does
                        // not return at all.
                        connector_response_reference_id: payment_method_id,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn minor(value: i64) -> MinorUnit {
        MinorUnit::new(value)
    }

    /// The connector's own `amount_converter` (`braintree.rs`: `amount_converter: StringMajorUnit`)
    /// resolves to this converter, so a test that goes through it is testing the real path.
    fn major_unit(value: i64) -> StringMajorUnit {
        use common_utils::types::AmountConvertor;
        common_utils::types::StringMajorUnitForConnector
            .convert(minor(value), common_enums::Currency::USD)
            .expect("major unit conversion")
    }

    // --- RSync currency precedence ---------------------------------------------------

    fn metadata_with(json: serde_json::Value) -> Option<pii::SecretSerdeValue> {
        Some(hyperswitch_masking::Secret::new(json))
    }

    /// The first-class `refund_amount` a caller sends must win. Hyperswitch populates it
    /// and does not populate the metadata copy.
    #[test]
    fn rsync_currency_prefers_refund_money_over_metadata() {
        let money = common_utils::types::Money {
            amount: minor(6000),
            currency: enums::Currency::USD,
        };
        let got = rsync_currency(
            Some(money),
            &metadata_with(serde_json::json!({ "currency": "EUR" })),
        );
        assert_eq!(got, Some(enums::Currency::USD));
    }

    /// Callers that still send `currency` only in the metadata blob keep working.
    #[test]
    fn rsync_currency_falls_back_to_metadata() {
        let got = rsync_currency(
            None,
            &metadata_with(serde_json::json!({ "currency": "EUR" })),
        );
        assert_eq!(got, Some(enums::Currency::EUR));
    }

    /// Neither source present must yield `None` so the guard is SKIPPED, not failed.
    /// Returning an error here is what made RSync unreachable from Hyperswitch: the
    /// refund search needs no currency at all, so a missing one cannot make it wrong.
    #[test]
    fn rsync_currency_absent_skips_the_guard_rather_than_failing() {
        assert_eq!(rsync_currency(None, &None), None);
        assert_eq!(
            rsync_currency(
                None,
                &metadata_with(serde_json::json!({ "merchant_account_id": "x" }))
            ),
            None
        );
    }

    // --- L3 string sanitisation (§E.9 charset rule) -----------------------------------------

    #[test]
    fn sanitizes_then_truncates_l3_text() {
        // Sanitise first, then truncate: truncating first would leave an illegal character in
        // place once the disallowed ones are stripped.
        assert_eq!(
            sanitize_l3_text("Blue & Green / Widget, 12\"", 35),
            "Blue Green Widget 12"
        );
        assert_eq!(
            sanitize_l3_text("O'Brien-Smith Co. 42", 35),
            "O'Brien-Smith Co. 42"
        );
        // Accents and emoji are not in the permitted set.
        assert_eq!(sanitize_l3_text("Café ☕ Beans", 35), "Caf Beans");
        // 12-character cap for unitOfMeasure / productCode / commodityCode.
        assert_eq!(sanitize_l3_text("ABCDEFGHIJKLMNOP", 12), "ABCDEFGHIJKL");
    }

    // --- dynamic descriptor phone (validation code 92202) -----------------------------------

    #[test]
    fn descriptor_phone_is_omitted_when_it_cannot_fit_the_10_to_14_char_window() {
        assert_eq!(
            sanitize_descriptor_phone("(312) 555-1212"),
            Some("(312)555-1212".to_string())
        );
        assert_eq!(
            sanitize_descriptor_phone("3125551212"),
            Some("3125551212".to_string())
        );
        // Eleven digits once the country code survives the strip — Braintree wants exactly ten.
        assert_eq!(sanitize_descriptor_phone("+1 (312) 555-1212"), None);
        // Too short once non-permitted characters are stripped.
        assert_eq!(sanitize_descriptor_phone("555-1212"), None);
        // Too long.
        assert_eq!(sanitize_descriptor_phone("+91 (080) 4718-1234 x99"), None);
    }

    // --- §E.9 amount reconciliation ----------------------------------------------------------

    #[test]
    fn l2_l3_breakdown_reconciles_on_a_balanced_body() {
        // 19.98 + 1.66 + 4.99 + 0.40 - 1.00 = 26.03
        assert!(l2_l3_breakdown_reconciles(
            minor(2603),
            &[minor(1998)],
            Some(minor(166)),
            Some(minor(499)),
            Some(minor(40)),
            Some(minor(100)),
        ));
    }

    #[test]
    fn l2_l3_breakdown_rejects_the_double_discount_trap() {
        // The classic mistake: the same 1.00 discount subtracted at the transaction level AND
        // baked into the line total. 18.98 + 1.66 + 4.99 + 0.40 - 1.00 = 25.03 != 26.03.
        assert!(!l2_l3_breakdown_reconciles(
            minor(2603),
            &[minor(1898)],
            Some(minor(166)),
            Some(minor(499)),
            Some(minor(40)),
            Some(minor(100)),
        ));
    }

    #[test]
    fn l2_l3_breakdown_has_nothing_to_balance_without_line_items() {
        // With no line items the sum is meaningless; L2 tax/shipping stand on their own.
        assert!(l2_l3_breakdown_reconciles(
            minor(6000),
            &[],
            Some(minor(166)),
            None,
            None,
            None,
        ));
    }

    // --- AVS / CVV response codes ------------------------------------------------------------

    #[test]
    fn avs_cvv_response_codes_map_to_their_rest_letters() {
        let cases = [
            ("MATCHES", AvsCvvResponseCode::Matches, Some("M")),
            (
                "DOES_NOT_MATCH",
                AvsCvvResponseCode::DoesNotMatch,
                Some("N"),
            ),
            ("NOT_VERIFIED", AvsCvvResponseCode::NotVerified, Some("U")),
            ("NOT_PROVIDED", AvsCvvResponseCode::NotProvided, Some("I")),
            (
                "ISSUER_DOES_NOT_PARTICIPATE",
                AvsCvvResponseCode::IssuerDoesNotParticipate,
                Some("S"),
            ),
            ("SYSTEM_ERROR", AvsCvvResponseCode::SystemError, Some("E")),
            (
                "NOT_APPLICABLE",
                AvsCvvResponseCode::NotApplicable,
                Some("A"),
            ),
            ("BYPASS", AvsCvvResponseCode::Bypass, Some("B")),
        ];
        for (wire, expected, letter) in cases {
            let parsed: AvsCvvResponseCode =
                serde_json::from_value(serde_json::Value::String(wire.to_string())).unwrap();
            assert_eq!(parsed, expected, "{wire}");
            assert_eq!(parsed.as_legacy_code(), letter, "{wire}");
        }
        // An unrecognised value must not fail the whole response.
        let parsed: AvsCvvResponseCode =
            serde_json::from_value(serde_json::json!("SOMETHING_NEW")).unwrap();
        assert_eq!(parsed, AvsCvvResponseCode::Unknown);
        assert_eq!(parsed.as_legacy_code(), None);
    }

    // --- decline surface -> GSM smart-retry inputs -------------------------------------------

    /// Captured verbatim from a Braintree sandbox `chargeCreditCard` for amount 2001.00
    /// (the documented "Insufficient Funds" trigger) with billing postal code 20000 and CVV 200.
    fn declined_transaction() -> TransactionAuthChargeResponseBody {
        serde_json::from_value(serde_json::json!({
            "id": "dHJhbnNhY3Rpb25fcTk5dmU3MjQ",
            "status": "PROCESSOR_DECLINED",
            "processorAuthorizationResponse": {
                "legacyCode": "2001",
                "message": "Insufficient Funds",
                "cvvResponse": "DOES_NOT_MATCH",
                "avsPostalCodeResponse": "DOES_NOT_MATCH",
                "avsStreetAddressResponse": "DOES_NOT_MATCH"
            },
            "statusHistory": [{
                "terminal": true,
                "declineType": "SOFT",
                "processorResponse": { "legacyCode": "2001", "message": "Insufficient Funds" },
                "networkResponse": { "code": "XX", "message": "sample network response text" },
                "merchantAdviceCodeResponse": { "code": "01", "message": null }
            }]
        }))
        .unwrap()
    }

    #[test]
    fn decline_populates_the_network_advice_and_decline_codes() {
        let error = create_transaction_failure_error_response(&declined_transaction(), 200);
        assert_eq!(error.code, "2001");
        assert_eq!(error.message, "Insufficient Funds");
        // Mastercard merchant advice code -> GSM advice code.
        assert_eq!(error.network_advice_code.as_deref(), Some("01"));
        // Raw card-network response, carried verbatim; it is only interpretable with the brand.
        assert_eq!(error.network_decline_code.as_deref(), Some("XX"));
        assert_eq!(
            error.network_error_message.as_deref(),
            Some("sample network response text")
        );
        assert_eq!(
            error.connector_transaction_id.as_deref(),
            Some("dHJhbnNhY3Rpb25fcTk5dmU3MjQ")
        );
        // Flow-agnostic: the terminal state is reported through PaymentFlowData.status, not here.
        assert!(error.attempt_status.is_none());
        let reason = error.reason.expect("reason");
        assert!(reason.contains("ProcessorDeclined"), "{reason}");
        assert!(reason.contains("decline_type=SOFT"), "{reason}");
    }

    #[test]
    fn gateway_rejection_reports_the_reason_rather_than_a_processor_code() {
        let transaction: TransactionAuthChargeResponseBody =
            serde_json::from_value(serde_json::json!({
                "id": "dHJhbnNhY3Rpb25fZHVw",
                "status": "GATEWAY_REJECTED",
                "statusHistory": [{
                    "terminal": true,
                    "gatewayRejectionReason": "DUPLICATE",
                    "networkResponse": { "code": null, "message": null },
                    "merchantAdviceCodeResponse": null
                }]
            }))
            .unwrap();
        let error = create_transaction_failure_error_response(&transaction, 200);
        assert_eq!(error.code, "DUPLICATE");
        assert_eq!(error.message, "DUPLICATE");
        let reason = error.reason.expect("reason");
        assert!(
            reason.contains("gateway_rejection_reason=DUPLICATE"),
            "{reason}"
        );
    }

    #[test]
    fn avs_and_cvv_results_are_surfaced_on_the_success_path() {
        let transaction: TransactionAuthChargeResponseBody =
            serde_json::from_value(serde_json::json!({
                "id": "dHJhbnNhY3Rpb25fNGNqNmV5dGc",
                "status": "SUBMITTED_FOR_SETTLEMENT",
                "processorAuthorizationResponse": {
                    "legacyCode": "1000",
                    "message": "Approved",
                    "cvvResponse": "MATCHES",
                    "avsPostalCodeResponse": "MATCHES",
                    "avsStreetAddressResponse": "MATCHES",
                    "authorizationId": "VHQ3GR",
                    "additionalInformation": null
                }
            }))
            .unwrap();
        let response = build_card_connector_response(&transaction).expect("connector response");
        let domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
            payment_checks,
            auth_code,
            ..
        } = response
            .additional_payment_method_data
            .expect("additional payment method data")
        else {
            panic!("expected a Card connector response")
        };
        assert_eq!(auth_code.as_deref(), Some("VHQ3GR"));
        let checks = payment_checks.expect("payment checks");
        assert_eq!(checks["cvv_response_code"], serde_json::json!("M"));
        assert_eq!(
            checks["avs_postal_code_response_code"],
            serde_json::json!("M")
        );
        assert_eq!(
            checks["avs_street_address_response_code"],
            serde_json::json!("M")
        );
        assert_eq!(checks["processor_response_code"], serde_json::json!("1000"));
    }

    // --- external 3DS pass-through -----------------------------------------------------------

    fn authentication_data(
        eci: Option<&str>,
        version: Option<&str>,
    ) -> router_request_types::AuthenticationData {
        router_request_types::AuthenticationData {
            trans_status: Some(common_enums::TransactionStatus::Success),
            eci: eci.map(ToString::to_string),
            cavv: Some(Secret::new("AAABAWFlmQAAAABjRWWZEEFgFz+=".to_string())),
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: Some("d3adb33f-0000-4000-8000-000000000001".to_string()),
            message_version: version.map(|v| v.parse().expect("semantic version")),
            ds_trans_id: Some("f38e6948-5388-41a6-bca4-b49723c19437".to_string()),
            acs_transaction_id: None,
            transaction_id: Some("xid-value".to_string()),
            network_params: None,
            exemption_indicator: None,
            created_at: None,
            challenge_code: None,
            challenge_cancel: None,
            challenge_code_reason: None,
            message_extension: None,
            authentication_type: None,
        }
    }

    #[test]
    fn three_ds_pass_through_is_omitted_without_an_eci() {
        // `eciFlag` is non-null, so there is no legal pass-through object to build.
        assert!(
            convert_external_three_ds_data(&authentication_data(None, Some("2.2.0"))).is_none()
        );
    }

    #[test]
    fn three_ds_pass_through_uses_the_sdl_field_names() {
        let pass_through =
            convert_external_three_ds_data(&authentication_data(Some("05"), Some("2.2.0")))
                .expect("pass through");
        let json = serde_json::to_value(&pass_through).unwrap();
        assert_eq!(json["eciFlag"], serde_json::json!("05"));
        assert_eq!(json["version"], serde_json::json!("2.2.0"));
        assert_eq!(json["directoryServerResponse"], serde_json::json!("Y"));
        assert_eq!(
            json["directoryServerTransactionId"],
            serde_json::json!("f38e6948-5388-41a6-bca4-b49723c19437")
        );
        assert_eq!(
            json["threeDSecureServerTransactionId"],
            serde_json::json!("d3adb33f-0000-4000-8000-000000000001")
        );
        // Names the brief got wrong must not appear on the wire.
        assert!(json.get("xid").is_none());
        assert!(json.get("threeDSecureVersion").is_none());
        assert!(json.get("authenticationResponse").is_none());
        assert!(json.get("dsTransactionId").is_none());
        // `xId` is a 3DS 1.x artefact and must be suppressed on a 3DS 2 authentication.
        assert!(json.get("xId").is_none());
    }

    #[test]
    fn three_ds_pass_through_carries_the_xid_on_a_3ds_one_authentication() {
        let pass_through =
            convert_external_three_ds_data(&authentication_data(Some("02"), Some("1.0.2")))
                .expect("pass through");
        let json = serde_json::to_value(&pass_through).unwrap();
        assert_eq!(json["xId"], serde_json::json!("xid-value"));
    }

    // --- stored-credential signalling ---------------------------------------------------------

    #[test]
    fn payment_initiator_serializes_as_the_braintree_enum_tokens() {
        assert_eq!(
            serde_json::to_value(PaymentInitiatorType::RecurringFirst).unwrap(),
            serde_json::json!("RECURRING_FIRST")
        );
        assert_eq!(
            serde_json::to_value(PaymentInitiatorType::Unscheduled).unwrap(),
            serde_json::json!("UNSCHEDULED")
        );
    }

    #[test]
    fn an_unrecognised_braintree_status_is_unspecified_not_an_invented_terminal_state() {
        // A status Braintree adds later must not fail the whole response parse...
        let parsed: BraintreePaymentStatus =
            serde_json::from_value(serde_json::json!("SOME_FUTURE_STATUS")).unwrap();
        assert!(matches!(parsed, BraintreePaymentStatus::Unknown));

        // ...and must not be invented into a Pending or a Failure. UCS has no previous status
        // to fall back on, so it reports Unspecified and lets the caller decide.
        assert_eq!(
            enums::AttemptStatus::from(parsed),
            enums::AttemptStatus::Unspecified
        );

        // The known statuses must keep their existing mapping.
        assert_eq!(
            enums::AttemptStatus::from(BraintreePaymentStatus::ProcessorDeclined),
            enums::AttemptStatus::Failure
        );
        assert_eq!(
            enums::AttemptStatus::from(BraintreePaymentStatus::Authorized),
            enums::AttemptStatus::Authorized
        );
    }

    #[test]
    fn cardholder_name_comes_from_the_card_and_is_omitted_when_absent() {
        // Checklist item 17: never fall back to the billing name, and never send an empty
        // string, which Braintree would store verbatim as the cardholder.
        let serialized = serde_json::to_value(CreditCardData::<
            domain_types::payment_method_data::DefaultPCIHolder,
        > {
            number: Default::default(),
            expiration_year: Secret::new("2030".to_string()),
            expiration_month: Secret::new("08".to_string()),
            cvv: Secret::new("999".to_string()),
            cardholder_name: None,
        })
        .unwrap();
        assert!(
            serialized.get("cardholderName").is_none(),
            "cardholderName must be omitted, not empty-stringed: {serialized}"
        );
    }

    #[test]
    fn cavv_algorithm_serializes_as_the_documented_single_digit_codes() {
        assert_eq!(
            serde_json::to_value(ThreeDSecureCavvAlgorithm::CvvWithAtn).unwrap(),
            serde_json::json!("2")
        );
        assert_eq!(
            serde_json::to_value(ThreeDSecureCavvAlgorithm::MastercardSpa).unwrap(),
            serde_json::json!("3")
        );
    }

    #[test]
    fn billing_address_uses_alpha3_country_codes_for_the_pinned_braintree_version() {
        // `Braintree-Version: 2019-01-01` < 2021-02-01, so `CountryCode` is alpha-3 on the wire.
        let address = build_braintree_address(Some(&domain_types::payment_address::Address {
            address: Some(domain_types::payment_address::AddressDetails {
                city: Some(Secret::new("Chicago".to_string())),
                country: Some(common_enums::CountryAlpha2::US),
                line1: Some(Secret::new("1 E Main St".to_string())),
                line2: Some(Secret::new("Suite 403".to_string())),
                line3: None,
                zip: Some(Secret::new("60622".to_string())),
                state: Some(Secret::new("IL".to_string())),
                first_name: Some(Secret::new("Jane".to_string())),
                last_name: Some(Secret::new("Doe".to_string())),
                origin_zip: None,
            }),
            phone: Some(domain_types::payment_address::PhoneDetails {
                number: Some(Secret::new("3125551212".to_string())),
                country_code: Some("+1".to_string()),
            }),
            email: None,
        }))
        .expect("address");
        let json = serde_json::to_value(&address).unwrap();
        assert_eq!(json["countryCode"], serde_json::json!("USA"));
        // Exactly one name per alias pair — the PayPal-style set.
        assert_eq!(json["addressLine1"], serde_json::json!("1 E Main St"));
        assert_eq!(json["adminArea2"], serde_json::json!("Chicago"));
        assert_eq!(json["adminArea1"], serde_json::json!("IL"));
        assert!(json.get("streetAddress").is_none());
        assert!(json.get("locality").is_none());
        assert!(json.get("region").is_none());
        // `PhoneInput` is all-or-nothing and the country code is sent without the leading `+`.
        assert_eq!(json["phone"]["countryPhoneCode"], serde_json::json!("1"));
        assert_eq!(
            json["phone"]["phoneNumber"],
            serde_json::json!("3125551212")
        );
    }

    #[test]
    fn empty_address_is_omitted_rather_than_sent_as_an_empty_object() {
        assert!(
            build_braintree_address(Some(&domain_types::payment_address::Address {
                address: None,
                phone: None,
                email: None,
            }))
            .is_none()
        );
    }

    // --- Braintree-hosted 3DS leg 1: PreAuthenticate ------------------------------------------

    #[test]
    fn pre_authenticate_sends_one_document_with_two_root_mutations() {
        // The whole point of this leg's shape: `tokenizeCreditCard` and `createClientToken` take
        // disjoint inputs, so both ride one HTTP call as root fields of one document. Verified
        // accepted by the Braintree sandbox under `Braintree-Version: 2019-01-01`.
        let request = BraintreePreAuthenticateRequest::<
            domain_types::payment_method_data::DefaultPCIHolder,
        > {
            query: constants::PRE_AUTHENTICATE_MUTATION.to_string(),
            variables: PreAuthenticateVariables {
                card: InputData {
                    credit_card: CreditCardData {
                        number: Default::default(),
                        expiration_year: Secret::new("2030".to_string()),
                        expiration_month: Secret::new("03".to_string()),
                        cvv: Secret::new("123".to_string()),
                        cardholder_name: None,
                    },
                },
                client_token: InputClientTokenData {
                    client_token: ClientTokenInput {
                        merchant_account_id: Secret::new("merchant_account".to_string()),
                    },
                },
            },
        };
        let json = serde_json::to_value(&request).unwrap();
        let query = json["query"].as_str().expect("query");
        assert!(
            query.contains("tokenizeCreditCard(input: $card)"),
            "{query}"
        );
        assert!(
            query.contains("createClientToken(input: $clientToken)"),
            "{query}"
        );
        // The variable names in the document must be the keys of the `variables` object, or
        // Braintree rejects the document before it executes either field.
        assert!(query.contains("$card: TokenizeCreditCardInput!"), "{query}");
        assert!(
            query.contains("$clientToken: CreateClientTokenInput!"),
            "{query}"
        );
        assert!(json["variables"]["card"]["creditCard"].is_object());
        assert_eq!(
            json["variables"]["clientToken"]["clientToken"]["merchantAccountId"],
            serde_json::json!("merchant_account")
        );
        // Nothing money-shaped belongs on this leg — `ClientTokenInput` has no amount member and
        // the lookup's amount belongs to the later `Authenticate` leg.
        assert!(json["variables"]["clientToken"]["clientToken"]
            .get("amount")
            .is_none());
    }

    #[test]
    fn pre_authenticate_reads_both_root_fields_off_a_success_body() {
        let response: BraintreePreAuthenticateResponse =
            serde_json::from_value(serde_json::json!({
                "data": {
                    "tokenizeCreditCard": { "paymentMethod": { "id": "tokencc_bh_test_nonce" } },
                    "createClientToken": { "clientToken": "eyJ2ZXJzaW9uIjoy" }
                }
            }))
            .unwrap();
        match response {
            BraintreePreAuthenticateResponse::PreAuthenticateResponse(success) => {
                assert_eq!(
                    success.data.tokenize_credit_card.payment_method.id.peek(),
                    "tokencc_bh_test_nonce"
                );
                assert_eq!(
                    success.data.create_client_token.client_token.peek(),
                    "eyJ2ZXJzaW9uIjoy"
                );
            }
            BraintreePreAuthenticateResponse::ErrorResponse(_) => {
                panic!("a clean success body must not match the error arm")
            }
        }
    }

    #[test]
    fn pre_authenticate_errors_win_over_a_partially_populated_data_object() {
        // Braintree answers HTTP 200 for everything, and a GraphQL partial success carries both
        // `errors[]` and a `data` object. `ErrorResponse` is listed first in the untagged enum
        // precisely so the errors win here instead of being silently dropped.
        let response: BraintreePreAuthenticateResponse =
            serde_json::from_value(serde_json::json!({
                "data": {
                    "tokenizeCreditCard": { "paymentMethod": { "id": "tokencc_bh_test_nonce" } },
                    "createClientToken": null
                },
                "errors": [{
                    "message": "Merchant account does not exist.",
                    "extensions": { "legacyCode": "93108", "errorClass": "VALIDATION" }
                }]
            }))
            .unwrap();
        let BraintreePreAuthenticateResponse::ErrorResponse(error_response) = response else {
            panic!("a body carrying errors[] must not be read as a success")
        };
        let built =
            build_error_response::<PaymentsResponseData>(error_response.errors.as_ref(), 200)
                .expect_err("a populated errors[] must become an ErrorResponse");
        assert_eq!(built.code, "93108");
        assert_eq!(built.message, "Merchant account does not exist.");
        assert_eq!(built.status_code, 200);
        // Flow-agnostic error builder: it must never assert a terminal attempt status.
        assert!(built.attempt_status.is_none());
    }

    #[test]
    fn pre_authenticate_redirect_form_carries_the_ddc_bootstrap_triple() {
        // `RedirectForm::Braintree::acs_url` is a misnomer inherited from hyperswitch — it is the
        // merchant's own completion URL, not an ACS URL.
        let form = get_braintree_redirect_form(
            Secret::new("eyJ2ZXJzaW9uIjoy".to_string()),
            Secret::new("tokencc_bh_test_nonce".to_string()),
            PaymentMethodData::Card(domain_types::payment_method_data::Card::<
                domain_types::payment_method_data::DefaultPCIHolder,
            > {
                card_number: RawCardNumber(
                    cards::CardNumber::try_from("4111111111111111".to_string()).unwrap(),
                ),
                card_exp_month: Secret::new("03".to_string()),
                card_exp_year: Secret::new("2030".to_string()),
                card_cvc: Secret::new("123".to_string()),
                card_issuer: None,
                card_network: None,
                card_type: None,
                card_issuing_country: None,
                bank_code: None,
                nick_name: None,
                card_holder_name: None,
                co_badged_card_data: None,
            }),
            "https://merchant.example/ddc-return".to_string(),
        )
        .expect("redirect form");
        let RedirectForm::Braintree {
            client_token,
            card_token,
            bin,
            acs_url,
        } = form
        else {
            panic!("expected RedirectForm::Braintree")
        };
        assert_eq!(client_token, "eyJ2ZXJzaW9uIjoy");
        assert_eq!(card_token, "tokencc_bh_test_nonce");
        // `threeDSecure.prepareLookup` takes the first six PAN digits as `bin`.
        assert_eq!(bin, "411111");
        assert_eq!(acs_url, "https://merchant.example/ddc-return");
    }

    // --- Braintree-hosted 3DS leg 2: Authenticate ---------------------------------------------

    /// The verbatim `CHALLENGE_REQUIRED` body captured from the Braintree sandbox
    /// (card 4000000000001091). Truncated only in `pareq` / `termUrl`, whose lengths carry no
    /// meaning for these assertions.
    fn challenge_lookup_body() -> serde_json::Value {
        serde_json::json!({
            "data": { "performThreeDSecureLookup": {
                "threeDSecureLookupData": {
                    "acsUrl": "https://0merchantacsstag.cardinalcommerce.com/MerchantACSWeb/creq.jsp",
                    "authenticationId": "drdw3449dksxx822vb",
                    "version": "2.1.0",
                    "pareq": "eyJtZXNzYWdlVHlwZSI6IkNSZXEi",
                    "md": "drdw3449dksxx822vb",
                    "termUrl": "https://api.sandbox.braintreegateway.com:443/merchants/merchant_id_placeholder/client_api/v1/payment_methods/fda15ca3-c67c-16d8-76d7-e10f21e92bd2/three_d_secure/authenticate?authorization_fingerprint=eyJraWQiOiJ4In0",
                    "transactionId": "8vjQoEtZEyYp7dJ0sLz0"
                },
                "paymentMethod": {
                    "id": "fda15ca3-c67c-16d8-76d7-e10f21e92bd2",
                    "details": {
                        "bin": "400000", "last4": "1091", "brandCode": "VISA",
                        "threeDSecure": { "authentication": {
                            "cavv": null, "eciFlag": "07",
                            "liabilityShifted": false, "liabilityShiftPossible": true,
                            "cardEnrolled": "YES",
                            "authenticationStatus": "CHALLENGE_REQUIRED",
                            "version": "2.1.0",
                            "directoryServerTransactionId": "3edee05f-ef2f-4ca5-9290-be569854e841",
                            "xId": null,
                            "threeDSecureServerTransactionId": "88de0d3f-da3d-4832-a6e5-21184d58560f",
                            "acsTransactionId": "4374ca16-9cb6-43ac-bfef-3d48ceb9d638",
                            "paresStatus": null,
                            "transactionStatus": "CHALLENGE_REQUIRED_FOR_AUTHENTICATION",
                            "transactionStatusReason": null
                        } }
                    }
                }
            } },
            "extensions": { "requestId": "5a3d0790-dac6-4a1e-a109-9de5a932721a" }
        })
    }

    /// The verbatim frictionless `AUTHENTICATE_SUCCESSFUL` body (card 4000000000001000).
    fn frictionless_lookup_body() -> serde_json::Value {
        serde_json::json!({
            "data": { "performThreeDSecureLookup": {
                // NOTE: still NON-NULL, with `acsUrl`/`pareq` null but everything else populated.
                "threeDSecureLookupData": {
                    "acsUrl": null,
                    "authenticationId": "dvywxm7kyhhpcxyyqr",
                    "version": "2.1.0",
                    "pareq": null,
                    "md": "dvywxm7kyhhpcxyyqr",
                    "termUrl": "https://api.sandbox.braintreegateway.com:443/merchants/merchant_id_placeholder/three_d_secure/authenticate?authorization_fingerprint=eyJraWQiOiJ4In0",
                    "transactionId": "bXNZLVb6Nt2FKgd9cQM0"
                },
                "paymentMethod": {
                    "id": "0d3a6993-68a4-11f0-43b2-d3b91990e1bd",
                    "details": {
                        "bin": "400000", "last4": "1000", "brandCode": "VISA",
                        "threeDSecure": { "authentication": {
                            "cavv": "AJkBBkhgQQAAAE4gSEJydQAAAAA=", "eciFlag": "05",
                            "liabilityShifted": true, "liabilityShiftPossible": true,
                            "cardEnrolled": "YES",
                            "authenticationStatus": "AUTHENTICATE_SUCCESSFUL",
                            "version": "2.1.0",
                            "directoryServerTransactionId": "3edee05f-ef2f-4ca5-9290-be569854e841",
                            "xId": null,
                            "threeDSecureServerTransactionId": "88de0d3f-da3d-4832-a6e5-21184d58560f",
                            "acsTransactionId": "4374ca16-9cb6-43ac-bfef-3d48ceb9d638",
                            "paresStatus": "SUCCESSFUL_AUTHENTICATION",
                            "transactionStatus": "SUCCESSFUL_AUTHENTICATION",
                            "transactionStatusReason": null
                        } }
                    }
                }
            } },
            "extensions": { "requestId": "5a3d0790-dac6-4a1e-a109-9de5a932721a" }
        })
    }

    fn parse_lookup_success(body: serde_json::Value) -> PerformThreeDSecureLookupPayload {
        let response: BraintreeAuthenticateResponse = serde_json::from_value(body).unwrap();
        let BraintreeAuthenticateResponse::AuthenticateResponse(success) = response else {
            panic!("a clean success body must not match the error arm")
        };
        success
            .data
            .perform_three_d_secure_lookup
            .expect("performThreeDSecureLookup payload")
    }

    fn authentication_of(
        payload: &PerformThreeDSecureLookupPayload,
    ) -> &ThreeDSecureAuthenticationDetails {
        payload
            .payment_method
            .as_ref()
            .and_then(|pm| pm.details.as_ref())
            .and_then(|details| details.three_d_secure.as_ref())
            .and_then(|three_ds| three_ds.authentication.as_ref())
            .expect("CreditCardDetails authentication block")
    }

    #[test]
    fn authenticate_document_and_variables_key_agree() {
        // The variable name declared in the document must be the key of the `variables` object,
        // or Braintree rejects the document before it executes.
        let request = BraintreeAuthenticateRequest {
            query: constants::AUTHENTICATE_MUTATION.to_string(),
            variables: GenericVariableInput {
                input: PerformThreeDSecureLookupInput {
                    payment_method_id: Secret::new("tokencc_bh_test_nonce".to_string()),
                    amount: major_unit(1000),
                    merchant_account_id: Secret::new("juspay".to_string()),
                    df_reference_id: None,
                    transaction_information: None,
                    cardholder_information: None,
                },
            },
        };
        let json = serde_json::to_value(&request).unwrap();
        let query = json["query"].as_str().expect("query");
        assert!(
            query.contains("$input: PerformThreeDSecureLookupInput!"),
            "{query}"
        );
        assert!(
            query.contains("performThreeDSecureLookup(input: $input)"),
            "{query}"
        );
        assert!(json["variables"]["input"].is_object());
        // `threeDSecure` MUST be traversed through `.authentication`: at the pinned
        // `Braintree-Version` the served schema already types `CreditCardDetails.threeDSecure` as
        // `ThreeDSecureDetails`, whose flat scalars are @deprecated and are a hard GraphQL
        // validation error to select.
        assert!(query.contains("threeDSecure { authentication {"), "{query}");
        // `details` is a union, so the inline fragment is mandatory.
        assert!(query.contains("... on CreditCardDetails"), "{query}");
        assert_eq!(
            json["variables"]["input"]["paymentMethodId"],
            serde_json::json!("tokencc_bh_test_nonce")
        );
        // Braintree distinguishes "absent" from "present and null" — omitted optionals must not
        // serialise as explicit nulls.
        assert!(json["variables"]["input"].get("dfReferenceId").is_none());
        assert!(json["variables"]["input"]
            .get("transactionInformation")
            .is_none());
    }

    #[test]
    fn authenticate_amount_is_major_units_through_the_connector_converter() {
        // `MinorUnit::to_string()` here would be a silent 100x overstatement Braintree cannot
        // detect, because "1000" is itself a valid `Amount`.
        let amount = major_unit(1000);
        assert_eq!(amount.get_amount_as_string(), "10.00");
        let request = BraintreeAuthenticateRequest {
            query: constants::AUTHENTICATE_MUTATION.to_string(),
            variables: GenericVariableInput {
                input: PerformThreeDSecureLookupInput {
                    payment_method_id: Secret::new("tokencc_bh_test_nonce".to_string()),
                    amount,
                    merchant_account_id: Secret::new("juspay".to_string()),
                    df_reference_id: None,
                    transaction_information: None,
                    cardholder_information: None,
                },
            },
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(
            json["variables"]["input"]["amount"],
            serde_json::json!("10.00")
        );
    }

    #[test]
    fn authenticate_browser_information_uses_the_braintree_spellings() {
        // `javascriptEnabled` has ONE capital, unlike the UCS `java_script_enabled`; and
        // `ipAddress` is a SIBLING of `browserInformation`, not a member of it.
        let browser = router_request_types::BrowserInformation {
            java_enabled: Some(false),
            java_script_enabled: Some(true),
            accept_header: Some("text/html".to_string()),
            language: Some("en-US".to_string()),
            color_depth: Some(24),
            screen_height: Some(1080),
            screen_width: Some(1920),
            time_zone: Some(0),
            user_agent: Some("Mozilla/5.0".to_string()),
            ip_address: Some(std::net::IpAddr::from([127, 0, 0, 1])),
            ..Default::default()
        };
        let transaction_information = ThreeDSecureLookupTransactionInformationInput {
            device_channel: Some(ThreeDSecureDeviceChannel::Browser),
            email: None,
            ip_address: browser.ip_address.map(|ip| Secret::new(ip.to_string())),
            browser_information: build_lookup_browser_information(Some(&browser)),
        };
        let json = serde_json::to_value(&transaction_information).unwrap();
        assert_eq!(json["deviceChannel"], serde_json::json!("BROWSER"));
        assert_eq!(json["ipAddress"], serde_json::json!("127.0.0.1"));
        assert!(json["browserInformation"].get("ipAddress").is_none());
        assert_eq!(
            json["browserInformation"]["javascriptEnabled"],
            serde_json::json!(true)
        );
        assert!(json["browserInformation"]
            .get("javaScriptEnabled")
            .is_none());
        assert_eq!(json["browserInformation"]["timeZone"], serde_json::json!(0));
    }

    #[test]
    fn authenticate_billing_country_is_not_converted_to_alpha3() {
        // `ThreeDSecureLookupBillingAddressInput.countryCode` is a plain `String`, not the
        // versioned `CountryCode` scalar the Authorize address input uses, so the alpha-3/alpha-2
        // boundary at `Braintree-Version 2021-02-01` does not apply on this leg.
        let cardholder =
            build_lookup_billing_address(Some(&domain_types::payment_address::Address {
                address: Some(domain_types::payment_address::AddressDetails {
                    first_name: Some(Secret::new("John".to_string())),
                    last_name: Some(Secret::new("Doe".to_string())),
                    line1: Some(Secret::new("123 Main St".to_string())),
                    city: Some(Secret::new("San Francisco".to_string())),
                    state: Some(Secret::new("CA".to_string())),
                    zip: Some(Secret::new("94105".to_string())),
                    country: Some(common_enums::CountryAlpha2::US),
                    ..Default::default()
                }),
                phone: None,
                email: None,
            }))
            .expect("cardholder information");
        let json = serde_json::to_value(&cardholder).unwrap();
        assert_eq!(
            json["billingAddress"]["countryCode"],
            serde_json::json!("US")
        );
        assert_eq!(
            json["billingAddress"]["givenName"],
            serde_json::json!("John")
        );
        assert_eq!(json["billingAddress"]["surname"], serde_json::json!("Doe"));
        // An empty billing address must be omitted entirely, never emitted as `{}`.
        assert!(build_lookup_billing_address(Some(
            &domain_types::payment_address::Address::default()
        ))
        .is_none());
    }

    #[test]
    fn authenticate_df_reference_id_is_read_under_either_spelling() {
        let from_camel =
            extract_df_reference_id(Some(&connector_types::ContinueRedirectionResponse {
                params: None,
                payload: Some(Secret::new(
                    serde_json::json!({ "dfReferenceId": "0_abc123" }),
                )),
            }));
        assert_eq!(
            from_camel.map(|value| value.peek().to_string()),
            Some("0_abc123".to_string())
        );
        let from_snake =
            extract_df_reference_id(Some(&connector_types::ContinueRedirectionResponse {
                params: None,
                payload: Some(Secret::new(
                    serde_json::json!({ "df_reference_id": "0_abc123" }),
                )),
            }));
        assert_eq!(
            from_snake.map(|value| value.peek().to_string()),
            Some("0_abc123".to_string())
        );
        assert!(extract_df_reference_id(None).is_none());
    }

    #[test]
    fn authenticate_discriminator_survives_the_always_present_lookup_data_trap() {
        // FINDING 2. `threeDSecureLookupData` comes back NON-NULL on EVERY outcome, frictionless
        // success included — so `three_d_secure_lookup_data.is_some()` would emit a redirect on
        // every single response and strand a frictionless payment in a challenge that does not
        // exist. This test exists to keep anyone from "simplifying" the discriminator back to it.
        let challenge = parse_lookup_success(challenge_lookup_body());
        let frictionless = parse_lookup_success(frictionless_lookup_body());
        assert!(challenge.three_d_secure_lookup_data.is_some());
        assert!(
            frictionless.three_d_secure_lookup_data.is_some(),
            "the trap: lookup data is present on a frictionless success too"
        );

        let challenge_auth = authentication_of(&challenge);
        let frictionless_auth = authentication_of(&frictionless);
        assert!(is_braintree_challenge(
            challenge_auth.authentication_status,
            challenge
                .three_d_secure_lookup_data
                .as_ref()
                .and_then(|data| data.acs_url.as_ref())
        ));
        assert!(!is_braintree_challenge(
            frictionless_auth.authentication_status,
            frictionless
                .three_d_secure_lookup_data
                .as_ref()
                .and_then(|data| data.acs_url.as_ref())
        ));
        // Both halves of the discriminator are load-bearing: CHALLENGE_REQUIRED with a null
        // acsUrl is not a challenge (it is an UnexpectedResponseError at the call site), and a
        // present acsUrl on a non-challenge status is not one either.
        assert!(!is_braintree_challenge(
            Some(ThreeDSecureAuthenticationStatus::ChallengeRequired),
            None
        ));
        assert!(!is_braintree_challenge(
            Some(ThreeDSecureAuthenticationStatus::AuthenticateSuccessful),
            Some(&"https://acs.example/creq".to_string())
        ));
    }

    #[test]
    fn authenticate_challenge_form_carries_the_acs_step_up_triple() {
        let challenge = parse_lookup_success(challenge_lookup_body());
        let form =
            build_challenge_redirect_form(challenge.three_d_secure_lookup_data.as_ref(), 200)
                .expect("challenge redirect form");
        let RedirectForm::Form {
            endpoint,
            method,
            form_fields,
        } = form
        else {
            panic!(
                "the challenge must use RedirectForm::Form — the Authenticate response mapper \
                    rejects RedirectForm::Braintree, which leg 1 uses"
            )
        };
        assert_eq!(
            endpoint,
            "https://0merchantacsstag.cardinalcommerce.com/MerchantACSWeb/creq.jsp"
        );
        assert_eq!(method, common_utils::Method::Post);
        assert_eq!(
            form_fields.get("PaReq").map(String::as_str),
            Some("eyJtZXNzYWdlVHlwZSI6IkNSZXEi")
        );
        // `md` is echoed, never synthesised — even though it has always equalled
        // `authenticationId`.
        assert_eq!(
            form_fields.get("MD").map(String::as_str),
            Some("drdw3449dksxx822vb")
        );
        assert!(form_fields
            .get("TermUrl")
            .is_some_and(|url| url.contains("authorization_fingerprint=")));
        assert_eq!(form_fields.len(), 3);

        // A frictionless outcome has no acsUrl/pareq, so asking for a form must fail loudly
        // rather than produce a half-built redirect.
        let frictionless = parse_lookup_success(frictionless_lookup_body());
        assert!(build_challenge_redirect_form(
            frictionless.three_d_secure_lookup_data.as_ref(),
            200
        )
        .is_err());
    }

    #[test]
    fn authenticate_status_table_splits_terminal_from_non_terminal() {
        use enums::AttemptStatus;
        // Non-terminal: a challenge must be able to complete (checklist #8).
        let challenge: AttemptStatus = ThreeDSecureAuthenticationStatus::ChallengeRequired.into();
        assert_eq!(challenge, AttemptStatus::AuthenticationPending);
        assert!(!challenge.is_terminal_status());

        // Terminal Braintree state -> terminal UCS state (checklist #7).
        for terminal in [
            ThreeDSecureAuthenticationStatus::AuthenticateFrictionlessFailed,
            ThreeDSecureAuthenticationStatus::AuthenticateFailed,
            ThreeDSecureAuthenticationStatus::AuthenticateFailedAcsError,
            ThreeDSecureAuthenticationStatus::AuthenticateRejected,
            ThreeDSecureAuthenticationStatus::AuthenticateError,
            ThreeDSecureAuthenticationStatus::AuthenticateUnableToAuthenticate,
            ThreeDSecureAuthenticationStatus::AuthenticationUnavailable,
            ThreeDSecureAuthenticationStatus::LookupError,
            ThreeDSecureAuthenticationStatus::LookupCardError,
            ThreeDSecureAuthenticationStatus::LookupServerError,
            ThreeDSecureAuthenticationStatus::LookupFailedAcsError,
            ThreeDSecureAuthenticationStatus::MpiServerError,
            ThreeDSecureAuthenticationStatus::UnsupportedCard,
            ThreeDSecureAuthenticationStatus::UnsupportedAccountType,
            ThreeDSecureAuthenticationStatus::UnsupportedThreeDSecureVersion,
        ] {
            let mapped: AttemptStatus = terminal.into();
            assert_eq!(mapped, AttemptStatus::AuthenticationFailed, "{terminal}");
            assert!(mapped.is_terminal_status(), "{terminal}");
        }

        // Advanceable: Braintree's Authorize is implemented, so the pipeline can move these on.
        // `DATA_ONLY_SUCCESSFUL` is a success even though it shifts NO liability — which is why
        // `liabilityShifted` is read, never inferred from the status.
        for advanceable in [
            ThreeDSecureAuthenticationStatus::AuthenticateSuccessful,
            ThreeDSecureAuthenticationStatus::AuthenticateAttemptSuccessful,
            ThreeDSecureAuthenticationStatus::DataOnlySuccessful,
            ThreeDSecureAuthenticationStatus::ExemptionLowValueSuccessful,
            ThreeDSecureAuthenticationStatus::ExemptionTraSuccessful,
            ThreeDSecureAuthenticationStatus::LookupNotEnrolled,
            ThreeDSecureAuthenticationStatus::LookupBypassed,
            ThreeDSecureAuthenticationStatus::SkippedDueToRule,
            ThreeDSecureAuthenticationStatus::SkippedDueToAdaptiveAuthentication,
        ] {
            assert_eq!(
                AttemptStatus::from(advanceable),
                AttemptStatus::AuthenticationSuccessful,
                "{advanceable}"
            );
        }
    }

    #[test]
    fn authenticate_unknown_status_maps_to_unspecified_not_pending_or_failure() {
        // Braintree added four statuses in 2022-09-30 and four more in 2023-05-23. A new one must
        // not fail the whole response parse, and it must not be invented into a Pending or a
        // Failure UCS cannot substantiate (checklist #6).
        let mut body = challenge_lookup_body();
        body["data"]["performThreeDSecureLookup"]["paymentMethod"]["details"]["threeDSecure"]
            ["authentication"]["authenticationStatus"] =
            serde_json::json!("SOME_STATUS_BRAINTREE_HAS_NOT_INVENTED_YET");
        let payload = parse_lookup_success(body);
        let status = authentication_of(&payload)
            .authentication_status
            .expect("status parses into the catch-all rather than failing");
        assert!(matches!(status, ThreeDSecureAuthenticationStatus::Unknown));
        assert_eq!(
            enums::AttemptStatus::from(status),
            enums::AttemptStatus::Unspecified
        );
        // An unrecognised status is not a challenge either, even with an acsUrl present.
        assert!(!is_braintree_challenge(
            Some(status),
            Some(&"https://acs.example/creq".to_string())
        ));
    }

    #[test]
    fn authenticate_trans_status_maps_eight_to_eight_and_never_defaults() {
        use common_enums::TransactionStatus;
        use ThreeDSecureAuthenticationStatusIndicator as Indicator;
        for (indicator, expected) in [
            (
                Indicator::SuccessfulAuthentication,
                TransactionStatus::Success,
            ),
            (Indicator::FailedAuthentication, TransactionStatus::Failure),
            (
                Indicator::UnableToCompleteAuthentication,
                TransactionStatus::VerificationNotPerformed,
            ),
            (
                Indicator::SuccessfulAttemptsTransaction,
                TransactionStatus::NotVerified,
            ),
            (
                Indicator::AuthenticationRejected,
                TransactionStatus::Rejected,
            ),
            (
                Indicator::ChallengeRequiredForAuthentication,
                TransactionStatus::ChallengeRequired,
            ),
            (
                Indicator::ChallengeRequiredDecoupledAuthentication,
                TransactionStatus::ChallengeRequiredDecoupledAuthentication,
            ),
            (
                Indicator::InformationalChallengePreferenceAcknowledged,
                TransactionStatus::InformationOnly,
            ),
        ] {
            assert_eq!(indicator.to_trans_status(), Some(expected), "{indicator}");
        }
        // `TransactionStatus` derives `Default = Failure`, so an unrecognised indicator MUST come
        // back as `None` — anything that reaches for a default here silently reports a failed
        // authentication.
        assert!(Indicator::Unknown.to_trans_status().is_none());
        assert_eq!(TransactionStatus::default(), TransactionStatus::Failure);
    }

    #[test]
    fn authenticate_reads_the_new_nonce_and_the_liability_booleans() {
        // FINDING 3: the lookup ALWAYS consumes the input nonce and returns a different
        // `paymentMethod.id` — a bare UUID, not a `tokencc_`-prefixed nonce. The subsequent
        // charge must spend the new one.
        let frictionless = parse_lookup_success(frictionless_lookup_body());
        let payment_method = frictionless.payment_method.as_ref().expect("paymentMethod");
        assert_eq!(
            payment_method.id.peek(),
            "0d3a6993-68a4-11f0-43b2-d3b91990e1bd"
        );
        assert!(!payment_method.id.peek().starts_with("tokencc_"));

        let authentication = authentication_of(&frictionless);
        // Liability shift is READ, never inferred from the status.
        assert_eq!(authentication.liability_shifted, Some(true));
        assert_eq!(authentication.liability_shift_possible, Some(true));
        assert_eq!(authentication.eci_flag.as_deref(), Some("05"));
        assert_eq!(
            authentication
                .cavv
                .as_ref()
                .map(|cavv| cavv.peek().as_str()),
            Some("AJkBBkhgQQAAAE4gSEJydQAAAAA=")
        );
        // Braintree returns a full three-part semver, so `SemanticVersion` parses it directly.
        assert!(authentication
            .version
            .as_deref()
            .and_then(|version| {
                <common_utils::types::SemanticVersion as std::str::FromStr>::from_str(version).ok()
            })
            .is_some());

        // A challenge legitimately carries no CAVV and no settled trans_status; that is not an
        // error.
        let challenge_auth_payload = parse_lookup_success(challenge_lookup_body());
        let challenge_auth = authentication_of(&challenge_auth_payload);
        assert!(challenge_auth.cavv.is_none());
        assert!(challenge_auth.pares_status.is_none());
        assert_eq!(
            challenge_auth
                .transaction_status
                .and_then(ThreeDSecureAuthenticationStatusIndicator::to_trans_status),
            Some(common_enums::TransactionStatus::ChallengeRequired)
        );
    }

    #[test]
    fn authenticate_errors_win_over_a_populated_data_object() {
        // Both REAL Braintree error bodies carry a `data` key alongside `errors[]`, so an
        // untagged enum that tried the success variant first — or discriminated on the presence
        // of `data` — would read a hard error as a success with a null payload. `ErrorResponse`
        // is listed FIRST and requires `errors`, which is what makes this work.
        let response: BraintreeAuthenticateResponse = serde_json::from_value(serde_json::json!({
            "errors": [{
                "message": "Nonce is already consumed",
                "path": ["performThreeDSecureLookup"],
                "extensions": { "errorClass": "VALIDATION", "errorType": "user_error" }
            }],
            "data": { "performThreeDSecureLookup": null },
            "extensions": { "requestId": "d1bb4b89-0000-0000-0000-000000000000" }
        }))
        .unwrap();
        let BraintreeAuthenticateResponse::ErrorResponse(error_response) = response else {
            panic!("a body carrying errors[] must not be read as a success")
        };
        let built =
            build_error_response::<PaymentsResponseData>(error_response.errors.as_ref(), 200)
                .expect_err("a populated errors[] must become an ErrorResponse");
        // No `legacyCode` on this body, so the shared fallback applies rather than an empty code.
        assert_eq!(built.code, NO_ERROR_CODE);
        assert_eq!(built.message, "Nonce is already consumed");
        assert_eq!(built.status_code, 200);
        // The error builder is shared with Refund/RSync/Capture, so it must never assert a
        // terminal attempt status (checklist #5).
        assert!(built.attempt_status.is_none());

        // A schema-validation error carries NO `data` key at all, which is why the error variant
        // must not require one.
        let no_data: BraintreeAuthenticateResponse = serde_json::from_value(serde_json::json!({
            "errors": [{ "message": "The variables input contains a field name 'challengeRequested' that is not defined for input object type 'PerformThreeDSecureLookupInput' " }],
            "extensions": { "requestId": "00000000-0000-0000-0000-000000000000" }
        }))
        .unwrap();
        assert!(matches!(
            no_data,
            BraintreeAuthenticateResponse::ErrorResponse(_)
        ));
    }
    // --- Braintree-hosted 3DS, leg 3: PostAuthenticate (the `node(id:)` readback) ------------

    /// Obviously fake, and deliberately so: no value from `creds.json` — no merchant id, no
    /// merchant-account id, no key — may appear in a tracked file.
    const FAKE_READBACK_PAYMENT_METHOD_ID: &str = "00000000-1111-2222-3333-444444444444";

    fn post_authenticate_request(
        connector_order_reference_id: Option<&str>,
        payment_method_data: Option<
            PaymentMethodData<domain_types::payment_method_data::DefaultPCIHolder>,
        >,
    ) -> PaymentsPostAuthenticateData<domain_types::payment_method_data::DefaultPCIHolder> {
        PaymentsPostAuthenticateData {
            payment_method_data,
            amount: minor(1000),
            email: None,
            currency: Some(common_enums::Currency::USD),
            payment_method_type: Some(common_enums::PaymentMethodType::Card),
            router_return_url: None,
            continue_redirection_url: None,
            browser_info: None,
            enrolled_for_3ds: false,
            // Always `None` in these fixtures: the ACS posted its PaRes to Braintree's own
            // termUrl, so this leg must work without any browser payload at all.
            redirect_response: None,
            capture_method: None,
            connector_order_reference_id: connector_order_reference_id.map(ToString::to_string),
        }
    }

    /// A settled `AUTHENTICATE_SUCCESSFUL` readback, shaped exactly like the live sandbox body.
    fn readback_body(authentication_status: &str, cavv: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "data": { "node": {
                "id": FAKE_READBACK_PAYMENT_METHOD_ID,
                "legacyId": "fake_legacy_id",
                "usage": "SINGLE_USE",
                "details": {
                    "bin": "400000",
                    "last4": "1000",
                    "brandCode": "VISA",
                    "threeDSecure": { "authentication": {
                        "cavv": cavv,
                        "eciFlag": "05",
                        "liabilityShifted": true,
                        "liabilityShiftPossible": true,
                        "cardEnrolled": "YES",
                        "authenticationStatus": authentication_status,
                        "version": "2.1.0",
                        "directoryServerTransactionId": "6360cf96-0000-4000-8000-000000000001",
                        "xId": null,
                        "threeDSecureServerTransactionId": "a7dd55d8-0000-4000-8000-000000000002",
                        "acsTransactionId": "361f4374-0000-4000-8000-000000000003",
                        "paresStatus": "SUCCESSFUL_AUTHENTICATION",
                        "transactionStatus": "SUCCESSFUL_AUTHENTICATION",
                        "transactionStatusReason": null
                    } }
                }
            } },
            "extensions": { "requestId": "00000000-0000-0000-0000-00000000000a" }
        })
    }

    fn parse_readback_success(body: serde_json::Value) -> BraintreeThreeDSecureResultSuccess {
        let response: BraintreePostAuthenticateResponse = serde_json::from_value(body).unwrap();
        let BraintreePostAuthenticateResponse::PostAuthenticateResponse(success) = response else {
            panic!("a clean readback body must not match the error arm")
        };
        *success
    }

    #[test]
    fn post_authenticate_document_and_variables_key_agree() {
        // The variable declared in the document must be the key of the `variables` object, or
        // Braintree rejects the document before it executes. This is the ONLY Braintree flow
        // whose variables are `{"id": …}` rather than `{"input": …}`, because `node(id:)` takes a
        // bare id argument — which is why `GenericVariableInput<T>` is not reused.
        let request = BraintreePostAuthenticateRequest {
            query: constants::POST_AUTHENTICATE_QUERY.to_string(),
            variables: BraintreePostAuthenticateVariables {
                id: Secret::new(FAKE_READBACK_PAYMENT_METHOD_ID.to_string()),
            },
        };
        let json = serde_json::to_value(&request).unwrap();
        let query = json["query"].as_str().expect("query");

        assert!(
            query.contains("query braintreeThreeDSecureResult($id: ID!)"),
            "{query}"
        );
        assert!(query.contains("node(id: $id)"), "{query}");
        assert_eq!(
            json["variables"]["id"],
            serde_json::json!(FAKE_READBACK_PAYMENT_METHOD_ID)
        );
        assert!(
            json["variables"].get("input").is_none(),
            "node(id:) takes a bare id, never an `input` object: {json}"
        );

        // `node` returns the Node INTERFACE and `details` is the PaymentMethodDetails UNION, so
        // both inline fragments are mandatory.
        assert!(query.contains("... on PaymentMethod"), "{query}");
        assert!(query.contains("... on CreditCardDetails"), "{query}");
        // `threeDSecure` MUST be traversed through `.authentication`: at the pinned
        // `Braintree-Version` its flat scalars are @deprecated and selecting them is a hard
        // GraphQL validation error.
        assert!(query.contains("threeDSecure { authentication {"), "{query}");

        // The authentication selection is character-for-character leg 2's, so both legs
        // deserialize the same Braintree type into the same Rust struct.
        let authentication_selection = "authentication { cavv eciFlag liabilityShifted \
                                        liabilityShiftPossible cardEnrolled authenticationStatus \
                                        version directoryServerTransactionId xId \
                                        threeDSecureServerTransactionId acsTransactionId \
                                        paresStatus transactionStatus transactionStatusReason }";
        assert!(query.contains(authentication_selection), "{query}");
        assert!(
            constants::AUTHENTICATE_MUTATION.contains(authentication_selection),
            "leg 2 and leg 3 must select the identical authentication block"
        );
    }

    #[test]
    fn post_authenticate_document_selects_neither_created_at_nor_authentication_insight() {
        // Both are live-observed footguns, which is why this is a literal negative assertion on
        // the pinned const:
        //
        // * `createdAt` on a SINGLE_USE payment method answers with a PARTIAL error — a populated
        //   `errors[]` alongside a fully usable `data` — and with `ErrorResponse` first in the
        //   untagged enum that would drag a good readback into the error arm.
        // * `authenticationInsight` requires an `input` argument; selecting it bare is a hard
        //   validation error.
        assert!(!constants::POST_AUTHENTICATE_QUERY.contains("createdAt"));
        assert!(!constants::POST_AUTHENTICATE_QUERY.contains("authenticationInsight"));
    }

    #[test]
    fn post_authenticate_reads_connector_order_reference_id_and_not_payment_method_data() {
        use domain_types::payment_method_data::PaymentMethodToken;

        // The spent input nonce still sits in `payment_method_data` on the composite path.
        // Reading it would send a nonce Braintree has already consumed and `node(id:)` would
        // answer NOT_FOUND, so the id comes from `connector_order_reference_id` — the NEW
        // payment-method id the Authenticate leg returned.
        let spent_nonce = PaymentMethodData::PaymentMethodToken(PaymentMethodToken {
            token: Secret::new("tokencc_bh_spent_input_nonce".to_string()),
            token_payment_method_type: None,
        });
        let resolved = post_authenticate_payment_method_id(&post_authenticate_request(
            Some(FAKE_READBACK_PAYMENT_METHOD_ID),
            Some(spent_nonce.clone()),
        ))
        .expect("connector_order_reference_id is present");
        assert_eq!(resolved.peek(), FAKE_READBACK_PAYMENT_METHOD_ID);

        // …and no `payment_method_data` fallback: without the reference id the leg fails loudly,
        // naming the field, rather than quietly reading the spent nonce.
        let err = post_authenticate_payment_method_id(&post_authenticate_request(
            None,
            Some(spent_nonce),
        ))
        .expect_err("a missing connector_order_reference_id must fail");
        assert!(
            matches!(
                err.current_context(),
                IntegrationError::MissingRequiredField {
                    field_name: "connector_order_reference_id",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn post_authenticate_errors_win_over_a_populated_data_object() {
        // The live NOT_FOUND body carries BOTH a populated `errors[]` and `{"data":{"node":null}}`,
        // so an untagged enum that tried the success variant first — or that discriminated on the
        // presence of `data` — would read a hard error as a success with a null node.
        let response: BraintreePostAuthenticateResponse =
            serde_json::from_value(serde_json::json!({
                "data": { "node": null },
                "errors": [{
                    "message": "An object with this ID was not found.",
                    "extensions": { "errorClass": "NOT_FOUND" }
                }]
            }))
            .unwrap();
        let BraintreePostAuthenticateResponse::ErrorResponse(error_response) = response else {
            panic!("a body carrying errors[] must not be read as a success")
        };
        let built =
            build_error_response::<PaymentsResponseData>(error_response.errors.as_ref(), 200)
                .expect_err("a populated errors[] must become an ErrorResponse");
        assert_eq!(built.code, NO_ERROR_CODE);
        assert_eq!(built.message, "An object with this ID was not found.");
        assert_eq!(built.status_code, 200);
        // The error builder is shared with Refund/RSync/Capture, so it must never assert a
        // terminal attempt status.
        assert!(built.attempt_status.is_none());

        // A schema-validation error carries NO `data` key at all, which is why the error variant
        // must not require one.
        let no_data: BraintreePostAuthenticateResponse =
            serde_json::from_value(serde_json::json!({
                "errors": [{ "message": "Field 'authenticationInsight' argument 'input' of type 'AuthenticationInsightInput!' is required but not provided." }]
            }))
            .unwrap();
        assert!(matches!(
            no_data,
            BraintreePostAuthenticateResponse::ErrorResponse(_)
        ));
    }

    #[test]
    fn post_authenticate_maps_a_settled_readback_onto_authentication_data() {
        let success = parse_readback_success(readback_body(
            "AUTHENTICATE_SUCCESSFUL",
            serde_json::json!("AJkBBkhgQQAAAE4gSEJydQAAAAA="),
        ));
        let (details, authentication) =
            resolve_post_authenticate_readback(success.data.node.as_ref(), 200)
                .expect("a populated readback resolves");

        // The same mapper leg 2 uses, with no lookup block — `node(id:)` returns none.
        let authentication_data = build_three_ds_authentication_data(authentication, None);
        assert_eq!(
            authentication_data
                .cavv
                .as_ref()
                .map(|cavv| cavv.peek().as_str()),
            Some("AJkBBkhgQQAAAE4gSEJydQAAAAA=")
        );
        assert_eq!(authentication_data.eci.as_deref(), Some("05"));
        assert_eq!(
            authentication_data.ds_trans_id.as_deref(),
            Some("6360cf96-0000-4000-8000-000000000001")
        );
        assert_eq!(
            authentication_data.threeds_server_transaction_id.as_deref(),
            Some("a7dd55d8-0000-4000-8000-000000000002")
        );
        assert_eq!(
            authentication_data.trans_status,
            Some(common_enums::TransactionStatus::Success)
        );
        assert_eq!(
            authentication_data
                .message_version
                .as_ref()
                .map(ToString::to_string),
            Some("2.1.0".to_string())
        );
        // `node(id:)` carries no `threeDSecureLookupData`, so the lookup's own transaction id is
        // legitimately absent on this leg.
        assert!(authentication_data.transaction_id.is_none());
        // Braintree exposes no CRes/RReq member anywhere on `ThreeDSecureAuthentication`, so the
        // challenge fields stay `None` even here, where the challenge is over.
        assert!(authentication_data.challenge_code.is_none());
        assert!(authentication_data.challenge_cancel.is_none());
        assert!(authentication_data.challenge_code_reason.is_none());

        // The verified nonce leaves by `connector_feature_data`, under the key the Authorize
        // guard reads.
        let feature_data = build_post_authenticate_feature_data(
            Some(&FAKE_READBACK_PAYMENT_METHOD_ID.to_string()),
            authentication,
            details,
        );
        let blob = &feature_data[constants::BRAINTREE_THREE_DS_FEATURE_KEY];
        assert_eq!(
            blob["payment_method_id"],
            serde_json::json!(FAKE_READBACK_PAYMENT_METHOD_ID)
        );
        // Liability shift is READ, never inferred from the status.
        assert_eq!(blob["liability_shifted"], serde_json::json!(true));
        assert_eq!(blob["liability_shift_possible"], serde_json::json!(true));
        // Raw Braintree spellings, not Rust variant names.
        assert_eq!(
            blob["authentication_status"],
            serde_json::json!("AUTHENTICATE_SUCCESSFUL")
        );
        assert_eq!(blob["card_enrolled"], serde_json::json!("YES"));
        assert_eq!(blob["bin"], serde_json::json!("400000"));
        assert_eq!(blob["last4"], serde_json::json!("1000"));
    }

    #[test]
    fn post_authenticate_status_mapping_is_leg_twos_table_verbatim() {
        // No second table: the same enum and the same `From` impl, keyed on the same Braintree
        // field of the same Braintree type.
        let settled = parse_readback_success(readback_body(
            "AUTHENTICATE_SUCCESSFUL",
            serde_json::json!("AJkBBkhgQQAAAE4gSEJydQAAAAA="),
        ));
        let (_, authentication) =
            resolve_post_authenticate_readback(settled.data.node.as_ref(), 200).unwrap();
        assert_eq!(
            authentication
                .authentication_status
                .map(enums::AttemptStatus::from),
            Some(enums::AttemptStatus::AuthenticationSuccessful)
        );

        // Still pending: the caller ran the readback before Braintree finished processing the
        // PaRes, or the cardholder abandoned the challenge. NON-TERMINAL — the readback is
        // idempotent and non-consuming, so calling again is the documented remedy. A synthesised
        // terminal failure here would be a lie.
        let pending =
            parse_readback_success(readback_body("CHALLENGE_REQUIRED", serde_json::Value::Null));
        let (_, pending_authentication) =
            resolve_post_authenticate_readback(pending.data.node.as_ref(), 200).unwrap();
        let pending_status = pending_authentication
            .authentication_status
            .map(enums::AttemptStatus::from)
            .unwrap();
        assert_eq!(pending_status, enums::AttemptStatus::AuthenticationPending);
        assert!(!pending_status.is_terminal_status());

        // Terminal stays terminal — this one was observed on THIS leg's own readback path, after
        // a PaRes reached Braintree's termUrl.
        let failed = parse_readback_success(readback_body(
            "AUTHENTICATE_UNABLE_TO_AUTHENTICATE",
            serde_json::Value::Null,
        ));
        let (_, failed_authentication) =
            resolve_post_authenticate_readback(failed.data.node.as_ref(), 200).unwrap();
        let failed_status = failed_authentication
            .authentication_status
            .map(enums::AttemptStatus::from)
            .unwrap();
        assert_eq!(failed_status, enums::AttemptStatus::AuthenticationFailed);
        assert!(failed_status.is_terminal_status());

        // An unrecognised status parses (it must never fail the whole response) and maps to
        // `Unspecified` — never an invented Pending or Failure.
        let unknown = parse_readback_success(readback_body(
            "SOME_STATUS_BRAINTREE_ADDED_LATER",
            serde_json::Value::Null,
        ));
        let (_, unknown_authentication) =
            resolve_post_authenticate_readback(unknown.data.node.as_ref(), 200).unwrap();
        assert_eq!(
            unknown_authentication
                .authentication_status
                .map(enums::AttemptStatus::from),
            Some(enums::AttemptStatus::Unspecified)
        );
    }

    #[test]
    fn post_authenticate_detects_every_empty_fragment_explicitly() {
        // None of these fails the GraphQL request, so each has to be detected rather than
        // inferred from a parse failure — and none of them is a frictionless success.

        // 1. The id resolved to one of the other 29 `Node` types: the `... on PaymentMethod`
        //    fragment contributes nothing.
        let other_node = parse_readback_success(serde_json::json!({
            "data": { "node": { "id": "not-a-payment-method-id" } }
        }));
        let err = resolve_post_authenticate_readback(other_node.data.node.as_ref(), 200)
            .expect_err("a non-PaymentMethod node must not pass");
        assert!(
            format!("{err:?}").contains("not a PaymentMethod"),
            "{err:?}"
        );

        // 2. A non-card payment method: the `... on CreditCardDetails` fragment yields `{}`.
        let non_card = parse_readback_success(serde_json::json!({
            "data": { "node": { "id": "a-paypal-payment-method", "usage": "SINGLE_USE",
                                "details": {} } }
        }));
        let err = resolve_post_authenticate_readback(non_card.data.node.as_ref(), 200)
            .expect_err("a non-card payment method must not pass");
        assert!(format!("{err:?}").contains("not a credit card"), "{err:?}");

        // 3. A card that never had a 3D Secure lookup run against it. This is the dangerous one:
        //    reporting it as a success would claim an authentication that never happened.
        let no_lookup = parse_readback_success(serde_json::json!({
            "data": { "node": { "id": "a-card-with-no-3ds", "usage": "SINGLE_USE",
                                "details": { "bin": "400000", "last4": "1000",
                                             "brandCode": "VISA", "threeDSecure": null } } }
        }));
        let err = resolve_post_authenticate_readback(no_lookup.data.node.as_ref(), 200)
            .expect_err("a card with no 3DS block must not pass");
        assert!(
            format!("{err:?}").contains("no 3D Secure lookup was ever run"),
            "{err:?}"
        );

        // 4. A null node with no errors at all.
        let err =
            resolve_post_authenticate_readback(None, 200).expect_err("a null node must not pass");
        assert!(format!("{err:?}").contains("null node"), "{err:?}");
    }

    #[test]
    fn next_authentication_step_routes_the_braintree_hosted_trio() {
        use interfaces::connector_types::{
            AuthenticationStep, RedirectState, ValidationTrait as _,
        };

        let connector = crate::connectors::Braintree::<
            domain_types::payment_method_data::DefaultPCIHolder,
        >::new();
        let three_ds = common_enums::AuthenticationType::ThreeDs;
        let card = common_enums::PaymentMethod::Card;

        assert_eq!(
            connector.next_authentication_step(three_ds, card, RedirectState::InitialRequest, None),
            AuthenticationStep::PreAuthenticate
        );
        // The device-data-collection return carries params (the dfReferenceId).
        assert_eq!(
            connector.next_authentication_step(
                three_ds,
                card,
                RedirectState::RedirectWithParams,
                None
            ),
            AuthenticationStep::Authenticate
        );
        assert_eq!(
            connector.next_authentication_step(
                three_ds,
                card,
                RedirectState::RedirectWithParams,
                Some(AuthenticationStep::Authenticate)
            ),
            AuthenticationStep::Authorize
        );
        // The ACS return carries NO params, because the PaRes went to Braintree's own termUrl and
        // never to UCS. That is exactly the signature of "the challenge is over, go read the
        // result" — it is the load-bearing arm of this override.
        assert_eq!(
            connector.next_authentication_step(
                three_ds,
                card,
                RedirectState::RedirectWithoutParams,
                None
            ),
            AuthenticationStep::PostAuthenticate
        );
        assert_eq!(
            connector.next_authentication_step(
                three_ds,
                card,
                RedirectState::RedirectWithoutParams,
                Some(AuthenticationStep::PostAuthenticate)
            ),
            AuthenticationStep::Authorize
        );

        // Non-3DS and non-card never enter the trio.
        assert_eq!(
            connector.next_authentication_step(
                common_enums::AuthenticationType::NoThreeDs,
                card,
                RedirectState::InitialRequest,
                None
            ),
            AuthenticationStep::Authorize
        );
        assert_eq!(
            connector.next_authentication_step(
                three_ds,
                common_enums::PaymentMethod::Wallet,
                RedirectState::InitialRequest,
                None
            ),
            AuthenticationStep::Authorize
        );
    }

    #[test]
    fn braintree_hosted_3ds_is_never_cross_wired_into_the_external_mpi_pass_through() {
        // THE trap this run exists to close, pinned in both directions.
        //
        // Both ingresses reach Authorize carrying `authentication_data`: the composite dispatcher
        // copies PostAuthenticate's into the Authorize request, exactly as hyperswitch does on
        // its `CompleteAuthorize`. Only the `connector_feature_data` marker tells them apart.
        let external = authentication_data(Some("05"), Some("2.2.0"));
        let hosted_marker: pii::SecretSerdeValue = Secret::new(serde_json::json!({
            constants::BRAINTREE_THREE_DS_FEATURE_KEY: {
                "payment_method_id": FAKE_READBACK_PAYMENT_METHOD_ID,
                "liability_shifted": true
            }
        }));

        // Direction 1 — Braintree-hosted: send NOTHING. Braintree holds the authentication on the
        // payment method and attaches it itself (live-proven: a verified nonce charged with no
        // passThru came back liabilityShifted: true). Re-declaring it as `threeDSecurePassThru`
        // would assert an EXTERNALLY performed authentication for one Braintree performed.
        assert_eq!(
            braintree_authorize_three_ds_mode(true, Some(&external), Some(&hosted_marker)),
            BraintreeAuthorizeThreeDsMode::Hosted
        );

        // Direction 2 — external MPI, no marker: the pass-through MUST still be sent.
        assert_eq!(
            braintree_authorize_three_ds_mode(true, Some(&external), None),
            BraintreeAuthorizeThreeDsMode::ExternalPassThrough
        );
        assert!(convert_external_three_ds_data(&external).is_some());

        // The guard is NARROWED, not deleted: a 3DS payment that ran neither topology still fails
        // closed rather than charging unauthenticated.
        assert_eq!(
            braintree_authorize_three_ds_mode(true, None, None),
            BraintreeAuthorizeThreeDsMode::Unauthenticated
        );
        // …but a hosted run that legitimately produced no `AuthenticationData` and only the
        // marker must NOT be rejected after the trio already completed.
        assert_eq!(
            braintree_authorize_three_ds_mode(true, None, Some(&hosted_marker)),
            BraintreeAuthorizeThreeDsMode::Hosted
        );
        // An unrelated `connector_feature_data` blob is not a Braintree-hosted marker.
        let unrelated: pii::SecretSerdeValue =
            Secret::new(serde_json::json!({ "some_other_connector": { "x": 1 } }));
        assert_eq!(
            braintree_authorize_three_ds_mode(true, Some(&external), Some(&unrelated)),
            BraintreeAuthorizeThreeDsMode::ExternalPassThrough
        );
        // No 3DS at all and nothing supplied: nothing to assert either way.
        assert_eq!(
            braintree_authorize_three_ds_mode(false, None, None),
            BraintreeAuthorizeThreeDsMode::None
        );
    }

    // --- Network Transaction ID (NTID) for merchant-initiated transactions ------------------
    //
    // Every literal below is a deliberately fake placeholder. Nothing here comes from
    // `creds.json` or from a real sandbox transaction.

    /// Obviously-fake 15-digit NTID, shaped like the ones Braintree returns but not one of them.
    const FAKE_NTID: &str = "111111111111111";
    /// Obviously-fake Braintree vault (multi-use) token — Regime A's credential handle.
    const FAKE_VAULT_TOKEN: &str = "cGF5bWVudG1ldGhvZF9mYWtlXzAwMDA";
    /// Obviously-fake single-use token from `PaymentMethodService/Tokenize` — Regime B's.
    const FAKE_SINGLE_USE_TOKEN: &str = "tokencc_fake_0000_0000_0000_000";

    fn regime_a() -> BraintreeMitVaultRegime {
        BraintreeMitVaultRegime::BraintreeVaulted {
            connector_mandate_id: FAKE_VAULT_TOKEN.to_string(),
        }
    }

    fn regime_b(network_transaction_id: Option<&str>) -> BraintreeMitVaultRegime {
        BraintreeMitVaultRegime::ExternallyVaulted {
            single_use_token: Secret::new(FAKE_SINGLE_USE_TOKEN.to_string()),
            network_transaction_id: network_transaction_id
                .map(|ntid| Secret::new(ntid.to_string())),
        }
    }

    /// Builds the MIT request body exactly as `MandatePaymentRequest::try_from` does, so the
    /// assertions below are about the real wire output and not a hand-copied approximation.
    fn mit_payment_input(vault_regime: &BraintreeMitVaultRegime) -> PaymentInput {
        let options = CreditCardTransactionOptions {
            three_d_secure_authentication: None,
            billing_address: None,
            external_vault: vault_regime.external_vault(),
        };
        PaymentInput {
            payment_method_id: vault_regime.payment_method_id(),
            api_request_key: Some("fake-merchant-request-id".to_string()),
            transaction: TransactionBody::Mandate(MandateTransactionBody {
                amount: major_unit(1200),
                merchant_account_id: Secret::new("fake_merchant_account".to_string()),
                channel: constants::CHANNEL_CODE.to_string(),
                order_id: "fake_order_ref".to_string(),
                payment_initiator: PaymentInitiatorType::Unscheduled,
            }),
            options: (!options.is_empty()).then_some(options),
        }
    }

    #[test]
    fn regime_a_can_never_emit_external_vault() {
        // RULE NT-1, and the single most important assertion in this file. Braintree's gateway
        // ACCEPTS `externalVault` on a Braintree-vaulted token, accepts it with no NTID, and
        // accepts a garbage NTID — all returning SUBMITTED_FOR_SETTLEMENT. No live test can
        // catch a regime error, so it has to be impossible by construction and pinned here.
        assert!(regime_a().external_vault().is_none());
        // The Braintree-vaulted variant has exactly one field, and it is the token; there is no
        // input that could turn `external_vault()` into `Some`.
        let body = serde_json::to_value(mit_payment_input(&regime_a())).unwrap();
        assert!(body.get("options").is_none());
        assert_eq!(
            body["paymentMethodId"].as_str(),
            Some(FAKE_VAULT_TOKEN),
            "Regime A must charge the Braintree vault token"
        );
    }

    #[test]
    fn regime_a_request_body_is_unchanged_by_ntid_support() {
        // A byte-for-byte pin of the ONLY RepeatPayment path the connector has ever served.
        // Adding `external_vault` to `CreditCardTransactionOptions` must not add an `options`
        // key, nor reorder or rename anything else.
        assert_eq!(
            serde_json::to_value(mit_payment_input(&regime_a())).unwrap(),
            serde_json::json!({
                "paymentMethodId": FAKE_VAULT_TOKEN,
                "apiRequestKey": "fake-merchant-request-id",
                "transaction": {
                    "amount": "12.00",
                    "merchantAccountId": "fake_merchant_account",
                    "channel": "HyperSwitchBT_Ecom",
                    "orderId": "fake_order_ref",
                    "paymentInitiator": "UNSCHEDULED"
                }
            })
        );
    }

    #[test]
    fn regime_b_emits_external_vault_under_options_not_transaction() {
        // `externalVault` hangs off `CreditCardTransactionOptionsInput`, so it is a sibling of
        // `transaction`. `transaction.externalVault` is rejected at variable coercion, and the
        // NTID field is `verifyingNetworkTransactionId` — `previousNetworkTransactionId` does
        // not exist on this input type.
        let body = serde_json::to_value(mit_payment_input(&regime_b(Some(FAKE_NTID)))).unwrap();
        assert_eq!(
            body["options"],
            serde_json::json!({
                "externalVault": {
                    "status": "VAULTED",
                    "verifyingNetworkTransactionId": FAKE_NTID
                }
            })
        );
        assert!(body["transaction"].get("externalVault").is_none());
        // The MIT signal itself is unchanged and still travels on `transaction`.
        assert_eq!(body["transaction"]["paymentInitiator"], "UNSCHEDULED");
        assert_eq!(
            body["paymentMethodId"].as_str(),
            Some(FAKE_SINGLE_USE_TOKEN),
            "Regime B must charge the externally vaulted single-use token"
        );
    }

    #[test]
    fn regime_b_without_an_ntid_degrades_instead_of_failing() {
        // The gRPC recurring path may hand over no NTID at all. Braintree accepts a bare
        // `{ status: VAULTED }`, so the MIT downgrades rather than erroring.
        let body = serde_json::to_value(mit_payment_input(&regime_b(None))).unwrap();
        assert_eq!(
            body["options"]["externalVault"],
            serde_json::json!({ "status": "VAULTED" })
        );
    }

    #[test]
    fn external_vault_status_is_always_present_and_never_a_free_string() {
        // `status` is `ExternalVaultStatus!` — NON-NULL. Omitting it is a hard coercion error
        // ("Field 'status' has coerced Null value for NonNull type 'ExternalVaultStatus!'"), so
        // it is modelled as the enum tag and can never be skipped.
        assert_eq!(
            serde_json::to_value(TransactionExternalVaultOptions::WillVault).unwrap(),
            serde_json::json!({ "status": "WILL_VAULT" })
        );
        // And `WILL_VAULT` carries no NTID: Braintree hard-errors on that pair, so the variant
        // has no field that could hold one. This assertion exists to fail if someone adds one.
        assert!(
            serde_json::to_value(TransactionExternalVaultOptions::WillVault)
                .unwrap()
                .get("verifyingNetworkTransactionId")
                .is_none()
        );
    }

    #[test]
    fn options_is_empty_accounts_for_external_vault() {
        // The silent-drop trap: `options` is only serialized when `!is_empty()`, so forgetting
        // `external_vault` here would discard the entire external-vault object with no error.
        let options = CreditCardTransactionOptions {
            three_d_secure_authentication: None,
            billing_address: None,
            external_vault: Some(TransactionExternalVaultOptions::Vaulted {
                verifying_network_transaction_id: Some(Secret::new(FAKE_NTID.to_string())),
            }),
        };
        assert!(!options.is_empty());
        assert!(CreditCardTransactionOptions {
            three_d_secure_authentication: None,
            billing_address: None,
            external_vault: None,
        }
        .is_empty());
    }

    // --- READ path: Transaction -> paymentMethodSnapshot -> networkTransactionId --------------

    fn transaction_with_snapshot(snapshot: serde_json::Value) -> TransactionAuthChargeResponseBody {
        serde_json::from_value(serde_json::json!({
            "id": "dHJhbnNhY3Rpb25fZmFrZQ",
            "status": "SUBMITTED_FOR_SETTLEMENT",
            "paymentMethodSnapshot": snapshot
        }))
        .unwrap()
    }

    #[test]
    fn network_transaction_id_is_read_through_the_snapshot_union() {
        assert_eq!(
            transaction_with_snapshot(serde_json::json!({
                "networkTransactionId": FAKE_NTID,
                "creditCard": { "bin": "411111", "last4": "1111", "brandCode": "VISA" }
            }))
            .network_transaction_id()
            .as_deref(),
            Some(FAKE_NTID)
        );
    }

    #[test]
    fn an_unmatched_snapshot_union_member_yields_none_rather_than_failing() {
        // `PaymentMethodSnapshot` has nine members and only `CreditCardTransactionDetails`
        // carries the field, so the inline fragment matches nothing on a PayPal/Venmo/bank
        // snapshot and the object arrives empty. That MUST deserialize, not fail: a hard error
        // here would turn a perfectly good payment into a response-parsing failure.
        assert!(transaction_with_snapshot(serde_json::json!({}))
            .network_transaction_id()
            .is_none());
        // An explicit null on a matched member behaves the same way.
        assert!(
            transaction_with_snapshot(serde_json::json!({ "networkTransactionId": null }))
                .network_transaction_id()
                .is_none()
        );
        // And a response with no snapshot key at all (the wallet mutations never select it).
        let no_snapshot: TransactionAuthChargeResponseBody = serde_json::from_value(
            serde_json::json!({ "id": "dHJhbnNhY3Rpb25fZmFrZQ", "status": "AUTHORIZED" }),
        )
        .unwrap();
        assert!(no_snapshot.network_transaction_id().is_none());
    }

    #[test]
    fn network_transaction_id_is_not_gated_on_a_success_status() {
        // The NTID is assigned at authorization time and is returned on PROCESSOR_DECLINED too.
        let declined: TransactionAuthChargeResponseBody =
            serde_json::from_value(serde_json::json!({
                "id": "dHJhbnNhY3Rpb25fZmFrZQ",
                "status": "PROCESSOR_DECLINED",
                "paymentMethodSnapshot": { "networkTransactionId": FAKE_NTID }
            }))
            .unwrap();
        assert_eq!(
            declined.network_transaction_id().as_deref(),
            Some(FAKE_NTID)
        );
    }

    #[test]
    fn card_mutations_select_the_snapshot_and_never_the_undefined_transaction_field() {
        const FRAGMENT: &str =
            "paymentMethodSnapshot { ... on CreditCardTransactionDetails { networkTransactionId } }";
        for mutation in [
            constants::CHARGE_CREDIT_CARD_MUTATION,
            constants::AUTHORIZE_CREDIT_CARD_MUTATION,
            constants::CHARGE_AND_VAULT_TRANSACTION_MUTATION,
            constants::AUTHORIZE_AND_VAULT_CREDIT_CARD_MUTATION,
        ] {
            assert!(
                mutation.contains(FRAGMENT),
                "missing fragment in {mutation}"
            );
            // `Transaction.networkTransactionId` does NOT exist at Braintree-Version 2019-01-01.
            // Selecting it is a document-level validation error, which rejects the whole document
            // BEFORE execution — it would break every Authorize, not just the MIT ones. The field
            // must therefore only ever appear inside the inline fragment.
            assert_eq!(
                mutation.matches("networkTransactionId").count(),
                1,
                "networkTransactionId must be reachable only through the snapshot fragment"
            );
        }
        // The wallet mutations go through `chargePaymentMethod` / `authorizePaymentMethod`, whose
        // snapshot is never `CreditCardTransactionDetails`; selecting the fragment there would
        // validate but always yield null, so they are deliberately left alone.
        for mutation in [
            constants::CHARGE_GOOGLE_PAY_MUTATION,
            constants::AUTHORIZE_GOOGLE_PAY_MUTATION,
            constants::CHARGE_APPLE_PAY_MUTATION,
            constants::AUTHORIZE_APPLE_PAY_MUTATION,
            constants::CHARGE_AND_VAULT_APPLE_PAY_MUTATION,
            constants::AUTHORIZE_AND_VAULT_APPLE_PAY_MUTATION,
            constants::CHARGE_PAYPAL_MUTATION,
            constants::AUTHORIZE_PAYPAL_MUTATION,
        ] {
            assert!(!mutation.contains("paymentMethodSnapshot"));
        }
    }

    // --- CIT -> MIT hand-off through mandate_metadata -----------------------------------------

    #[test]
    fn mandate_metadata_round_trips_the_network_transaction_id() {
        let metadata = build_braintree_mandate_metadata(Some(FAKE_NTID.to_string()))
            .expect("an NTID must produce metadata");
        assert_eq!(
            braintree_ntid_from_mandate_metadata(
                &connector_types::MandateReferenceId::ConnectorMandateId(
                    connector_types::ConnectorMandateReferenceId::new(
                        Some(FAKE_VAULT_TOKEN.to_string()),
                        None,
                        None,
                        Some(metadata),
                        None,
                    )
                )
            )
            .as_deref(),
            Some(FAKE_NTID)
        );
        // No NTID to carry -> no metadata, so responses without a snapshot keep their old shape.
        assert!(build_braintree_mandate_metadata(None).is_none());
    }

    #[test]
    fn a_missing_or_unusable_mandate_metadata_degrades_to_no_ntid() {
        // Absent, foreign-shaped and null-valued metadata must all yield `None` and never an
        // error — in Regime A the NTID is not sent anyway, and erroring would break the one
        // RepeatPayment path that already works.
        let reference = |metadata: Option<pii::SecretSerdeValue>| {
            connector_types::MandateReferenceId::ConnectorMandateId(
                connector_types::ConnectorMandateReferenceId::new(
                    Some(FAKE_VAULT_TOKEN.to_string()),
                    None,
                    None,
                    metadata,
                    None,
                ),
            )
        };
        assert!(braintree_ntid_from_mandate_metadata(&reference(None)).is_none());
        assert!(
            braintree_ntid_from_mandate_metadata(&reference(Some(Secret::new(
                serde_json::json!({ "some_other_connector": "value" })
            ))))
            .is_none()
        );
        assert!(
            braintree_ntid_from_mandate_metadata(&reference(Some(Secret::new(
                serde_json::json!({ "network_transaction_id": null })
            ))))
            .is_none()
        );
        assert!(
            braintree_ntid_from_mandate_metadata(&reference(Some(Secret::new(serde_json::json!(
                "not-an-object"
            )))))
            .is_none()
        );
        // A `NetworkMandateId` reference carries no metadata slot at all.
        assert!(braintree_ntid_from_mandate_metadata(
            &connector_types::MandateReferenceId::NetworkMandateId(
                connector_types::NetworkMandateIdRef {
                    network_transaction_id: FAKE_NTID.to_string(),
                    transaction_link_id: None,
                }
            )
        )
        .is_none());
    }

    // --- §W incoming webhooks ----------------------------------------------------------------
    //
    // The old `sample_webhook_body` fixture was written against the (broken) structs rather
    // than against a captured payload, so it encoded exactly the defects the parser had and
    // kept validating them. Every test below is written against Braintree's own SDK sample
    // shape instead: `<subject>` wrapper, kebab-case elements, `type=` attributes,
    // `nil="true"` empties, lowercase statuses and decimal major-unit amounts.

    /// The real `sample_webhook_body`, round-tripped through the real decode path. This is the
    /// regression guard on the whole fixture: if the fixture and the structs ever drift apart
    /// again, this fails.
    fn sample_request() -> connector_types::RequestDetails {
        use interfaces::connector_types::IncomingWebhook;

        let connector =
            super::super::Braintree::<domain_types::payment_method_data::DefaultPCIHolder>::new();
        connector_types::RequestDetails {
            method: connector_types::HttpMethod::Post,
            uri: None,
            headers: std::collections::HashMap::new(),
            body: connector.sample_webhook_body().to_vec(),
            query_params: None,
        }
    }

    /// Wraps a `<subject>` body in the full form-urlencoded + base64 envelope Braintree sends,
    /// so every test exercises `decode_from_request` — the production path — rather than the
    /// XML parse alone.
    fn braintree_webhook_request(xml: &str) -> connector_types::RequestDetails {
        let encoded = super::super::BASE64_ENGINE.encode(xml.as_bytes());
        let body = serde_urlencoded::to_string([
            ("bt_signature", "dummy_public_key|dummy_signature"),
            ("bt_payload", encoded.as_str()),
        ])
        .expect("form-encode the Braintree webhook envelope");

        connector_types::RequestDetails {
            method: connector_types::HttpMethod::Post,
            uri: None,
            headers: std::collections::HashMap::new(),
            body: body.into_bytes(),
            query_params: None,
        }
    }

    const DISPUTE_XML: &str = concat!(
        "<notification>",
        "<timestamp type=\"datetime\">2024-01-01T00:00:00Z</timestamp>",
        "<kind>dispute_opened</kind>",
        "<source-merchant-id>sub_merchant_1</source-merchant-id>",
        "<subject><dispute>",
        "<amount-disputed>10.00</amount-disputed>",
        "<amount-won nil=\"true\"/>",
        "<case-number>CASE-001</case-number>",
        "<currency-iso-code>USD</currency-iso-code>",
        "<id>dispute_id_001</id>",
        "<kind>chargeback</kind>",
        "<status>open</status>",
        "<reason>fraud</reason>",
        "<reason-code nil=\"true\"/>",
        "<created-at type=\"datetime\">2024-01-01T00:00:00Z</created-at>",
        "<reply-by-date type=\"date\">2024-01-15</reply-by-date>",
        "<evidence><comment nil=\"true\"/><url nil=\"true\"/></evidence>",
        "<transaction>",
        "<id>sale_id_001</id>",
        "<amount>10.00</amount>",
        "<order-id>order_001</order-id>",
        "<payment-instrument-type>credit_card</payment-instrument-type>",
        "</transaction>",
        "</dispute></subject>",
        "</notification>"
    );

    const REFUND_FAILED_XML: &str = concat!(
        "<notification>",
        "<timestamp type=\"datetime\">2024-01-01T00:00:00Z</timestamp>",
        "<kind>refund_failed</kind>",
        "<subject><transaction>",
        "<id>refund_id_001</id>",
        "<status>processor_declined</status>",
        "<amount>10.00</amount>",
        "<currency-iso-code>USD</currency-iso-code>",
        "<order-id>order_001</order-id>",
        "<refunded-transaction-id>sale_id_001</refunded-transaction-id>",
        "<processor-response-code>2001</processor-response-code>",
        "<processor-response-text>Insufficient Funds</processor-response-text>",
        "</transaction></subject>",
        "</notification>"
    );

    const TRANSACTION_SETTLED_XML: &str = concat!(
        "<notification>",
        "<timestamp type=\"datetime\">2024-01-01T00:00:00Z</timestamp>",
        "<kind>transaction_settled</kind>",
        "<subject><transaction>",
        "<id>sale_id_002</id>",
        "<status>settled</status>",
        "<amount>100.00</amount>",
        "<currency-iso-code>USD</currency-iso-code>",
        "<order-id>order_002</order-id>",
        "<payment-instrument-type>us_bank_account</payment-instrument-type>",
        "</transaction></subject>",
        "</notification>"
    );

    // §W.2.2.1 — pins the actual quick-xml 0.31 behaviour rather than trusting it: `type=`
    // attributes on a scalar element must not break the bind, and `nil="true"` empties must
    // not fail the notification.
    #[test]
    fn parses_attributed_and_nil_elements() {
        let notification = decode_from_request(&braintree_webhook_request(DISPUTE_XML))
            .expect("attributed/nil payload must parse");

        // `<timestamp type="datetime">` — attribute present, text still binds.
        assert_eq!(notification.timestamp, "2024-01-01T00:00:00Z");
        let dispute = notification.dispute().expect("dispute subject");
        assert_eq!(
            dispute.created_at.as_deref(),
            Some("2024-01-01T00:00:00Z"),
            "created-at carries type=\"datetime\""
        );
        assert_eq!(dispute.reply_by_date.as_deref(), Some("2024-01-15"));
        // `nil="true"` empties map to None through `deserialize_nil_aware` (typed targets) and
        // must not fail the parse for the untyped ones either.
        assert!(dispute.amount_won.is_none(), "<amount-won nil=\"true\"/>");
        assert!(dispute.evidence.is_some());
        // Verified behaviour of quick-xml 0.31, not an assumption: without the helper a
        // `nil="true"` element deserializes into `Option<String>` as `Some("")` — an empty
        // `connector_reason_code` reaching the merchant — and into a typed target as a hard
        // parse error that fails the WHOLE notification. Every optional element on these
        // structs therefore routes through `deserialize_nil_aware`.
        assert_eq!(
            dispute.reason_code, None,
            "<reason-code nil=\"true\"/> -> None"
        );
    }

    // §W.0.2 / defect W-1 + W-2: the `<subject>` level and kebab-case element names. Before
    // this fix `dispute` was a direct child of `<notification>` and every field was looked up
    // under a snake_case name the wire never uses, so a real chargeback could not deserialize.
    #[test]
    fn parses_subject_wrapped_kebab_case_dispute() {
        let notification = decode_from_request(&braintree_webhook_request(DISPUTE_XML))
            .expect("subject-wrapped dispute must parse");

        assert_eq!(
            notification.source_merchant_id.as_deref(),
            Some("sub_merchant_1")
        );
        let dispute = notification
            .dispute()
            .expect("subject.dispute is populated");
        assert_eq!(dispute.id, "dispute_id_001");
        assert_eq!(dispute.case_number.as_deref(), Some("CASE-001"));
        assert_eq!(dispute.currency_iso_code, Some(enums::Currency::USD));
        assert_eq!(
            dispute
                .amount_disputed
                .as_ref()
                .map(|a| a.get_amount_as_string()),
            Some("10.00".to_string()),
            "amount-disputed is a decimal MAJOR-unit string, not integer minor units"
        );
        assert_eq!(dispute.transaction.id, "sale_id_001");
        assert_eq!(dispute.transaction.order_id.as_deref(), Some("order_001"));
    }

    // §W.7.4 / defect W-5: `10.00` major -> 1000 minor -> "1000" through the connector's
    // configured webhook converter. The old code typed this `MinorUnit` and would have read
    // `10.00` as an integer (in fact it failed to parse at all).
    #[test]
    fn dispute_amount_is_major_units_converted_back_to_minor() {
        let notification = decode_from_request(&braintree_webhook_request(DISPUTE_XML)).unwrap();
        let response = build_webhook_dispute_response(&notification, b"raw").unwrap();

        assert_eq!(response.amount.to_string(), "1000");
        assert_eq!(response.currency, enums::Currency::USD);
        assert_eq!(response.dispute_id, "dispute_id_001");
        assert_eq!(response.status, enums::DisputeStatus::DisputeOpened);
        assert_eq!(response.stage, enums::DisputeStage::Dispute);
        assert_eq!(
            response.connector_response_reference_id.as_deref(),
            Some("order_001")
        );
        assert_eq!(response.dispute_message.as_deref(), Some("fraud"));
        assert_eq!(response.status_code, 200);
    }

    // §W.6.2 / defect W-3: the old matcher accepted only the three uppercase spellings and
    // returned Err on anything else — so Braintree's modern lowercase `<kind>chargeback</kind>`
    // discarded the commonest dispute webhook in existence.
    #[test]
    fn dispute_stage_is_case_insensitive_and_never_errors() {
        for spelling in ["chargeback", "CHARGEBACK", "Chargeback", " chargeback "] {
            assert_eq!(
                get_dispute_stage(Some(spelling)),
                enums::DisputeStage::Dispute,
                "{spelling}"
            );
        }
        assert_eq!(
            get_dispute_stage(Some("pre_arbitration")),
            enums::DisputeStage::PreArbitration
        );
        assert_eq!(
            get_dispute_stage(Some("PRE_ARBITRATION")),
            enums::DisputeStage::PreArbitration
        );
        assert_eq!(
            get_dispute_stage(Some("retrieval")),
            enums::DisputeStage::PreDispute
        );
        // An unrecognised or absent stage must NOT discard a notification that has a reply-by
        // deadline attached: it degrades to the default stage.
        assert_eq!(
            get_dispute_stage(Some("who_knows")),
            enums::DisputeStage::Dispute
        );
        assert_eq!(get_dispute_stage(None), enums::DisputeStage::Dispute);
    }

    // §W.4.1: all 8 dispute kinds map, and every one classifies as a dispute event so the
    // fan-out in `process_webhook_event` reaches `process_dispute_webhook`.
    #[test]
    fn all_eight_dispute_kinds_map_to_dispute_events() {
        let expected = [
            (
                "dispute_opened",
                connector_types::EventType::DisputeOpened,
                enums::DisputeStatus::DisputeOpened,
            ),
            (
                "dispute_accepted",
                connector_types::EventType::DisputeAccepted,
                enums::DisputeStatus::DisputeAccepted,
            ),
            (
                "dispute_auto_accepted",
                connector_types::EventType::DisputeAccepted,
                enums::DisputeStatus::DisputeAccepted,
            ),
            (
                "dispute_disputed",
                connector_types::EventType::DisputeChallenged,
                enums::DisputeStatus::DisputeChallenged,
            ),
            // defect W-4: previously unmapped, so it fell to the misc arm and routed a dispute
            // payload to `process_payment_webhook`.
            (
                "dispute_under_review",
                connector_types::EventType::DisputeChallenged,
                enums::DisputeStatus::DisputeChallenged,
            ),
            (
                "dispute_expired",
                connector_types::EventType::DisputeExpired,
                enums::DisputeStatus::DisputeExpired,
            ),
            (
                "dispute_won",
                connector_types::EventType::DisputeWon,
                enums::DisputeStatus::DisputeWon,
            ),
            (
                "dispute_lost",
                connector_types::EventType::DisputeLost,
                enums::DisputeStatus::DisputeLost,
            ),
        ];

        for (kind, event_type, dispute_status) in expected {
            let mapped = get_status(kind);
            assert!(
                mapped.is_dispute_event(),
                "{kind} must classify as a dispute event"
            );
            assert_eq!(mapped, event_type, "{kind}");
            assert_eq!(get_dispute_status(kind), dispute_status, "{kind}");
        }
    }

    // §W.4.2 / §W.4.4: the transaction family is exactly two kinds and the refund family
    // exactly one. `transaction_authorized`, `transaction_voided`,
    // `transaction_processor_declined`, `transaction_gateway_rejected`, `refund_settled` and
    // `refund_succeeded` DO NOT EXIST — they must not be silently mapped.
    #[test]
    fn transaction_and_refund_kinds_are_exactly_the_documented_set() {
        assert_eq!(
            get_status("transaction_settled"),
            connector_types::EventType::PaymentIntentSuccess
        );
        assert_eq!(
            get_status("transaction_settlement_declined"),
            connector_types::EventType::PaymentIntentFailure
        );
        assert!(get_status("transaction_settled").is_payment_event());
        assert!(get_status("transaction_settlement_declined").is_payment_event());

        assert_eq!(
            get_status("refund_failed"),
            connector_types::EventType::RefundFailure
        );
        assert!(get_status("refund_failed").is_refund_event());

        for invented in [
            "transaction_authorized",
            "transaction_voided",
            "transaction_processor_declined",
            "transaction_gateway_rejected",
            "transaction_captured",
            "refund_settled",
            "refund_succeeded",
        ] {
            assert_eq!(
                get_status(invented),
                connector_types::EventType::IncomingWebhookEventUnspecified,
                "{invented} is not a Braintree webhook kind and must not be mapped"
            );
        }
    }

    // §W.9 / defect W-1: an unmodelled kind must be a benign `Ok(None)`, not a hard
    // ParseEvent error. Previously every non-dispute kind returned
    // `Err(WebhookReferenceIdNotFound)`.
    #[test]
    fn unknown_kind_is_unspecified_and_has_no_reference() {
        let xml = "<notification><timestamp>2024-01-01T00:00:00Z</timestamp>\
                   <kind>subscription_went_past_due</kind></notification>";
        let notification = decode_from_request(&braintree_webhook_request(xml))
            .expect("an unmodelled kind must still deserialize");

        assert_eq!(
            get_status(&notification.kind),
            connector_types::EventType::IncomingWebhookEventUnspecified
        );
        assert!(matches!(get_webhook_reference(&notification), Ok(None)));
    }

    // §W.7.2: `<id>` on a refund_failed payload is the REFUND, the parent sale is
    // `<refunded-transaction-id>`. Swapping them updates the wrong row.
    #[test]
    fn refund_failed_ids_are_not_swapped() {
        let notification =
            decode_from_request(&braintree_webhook_request(REFUND_FAILED_XML)).unwrap();
        let response = build_webhook_refund_response(&notification, b"raw").unwrap();

        assert_eq!(
            response.connector_refund_id.as_deref(),
            Some("refund_id_001")
        );
        assert_eq!(
            response.connector_response_reference_id.as_deref(),
            Some("sale_id_001")
        );
        assert_ne!(
            response.connector_refund_id,
            response.connector_response_reference_id
        );
        assert_eq!(
            response.merchant_transaction_id.as_deref(),
            Some("order_001")
        );
        // `processor_declined` is a terminal Braintree state -> a terminal UCS state.
        assert_eq!(response.status, enums::RefundStatus::Failure);
        // Checklist #11: a refund failure webhook must carry the reason, not a bare status.
        assert_eq!(response.error_code.as_deref(), Some("2001"));
        assert_eq!(
            response.error_message.as_deref(),
            Some("Insufficient Funds")
        );

        match get_webhook_reference(&notification).unwrap() {
            Some(connector_types::WebhookResourceReference::Refund(reference)) => {
                assert_eq!(
                    reference.connector_refund_id.as_deref(),
                    Some("refund_id_001")
                );
                assert_eq!(
                    reference.connector_transaction_id.as_deref(),
                    Some("sale_id_001")
                );
                assert_eq!(
                    reference.merchant_transaction_id.as_deref(),
                    Some("order_001")
                );
                assert!(reference.merchant_refund_id.is_none());
            }
            other => panic!("expected a Refund reference, got {other:?}"),
        }
    }

    // The `-fk` spelling appears only in four SDKs' *sample generators*; no SDK parses it.
    // Tolerated as an alias so a harness payload produced by one of them still parses —
    // production logic is keyed on `refunded-transaction-id`.
    #[test]
    fn refunded_transaction_fk_alias_parses() {
        let xml = REFUND_FAILED_XML.replace("refunded-transaction-id", "refunded-transaction-fk");
        let notification = decode_from_request(&braintree_webhook_request(&xml)).unwrap();
        let transaction = notification.transaction().expect("transaction subject");
        assert_eq!(
            transaction.parent_transaction_id().as_deref(),
            Some("sale_id_001")
        );
    }

    // §W.3.4: the webhook `<status>` is lowercase snake_case while `BraintreePaymentStatus` is
    // SCREAMING_SNAKE (it parses GraphQL). Reusing the GraphQL enum would send every real
    // webhook status into its `#[serde(other)] Unknown` arm — a silent, total misreading.
    #[test]
    fn lowercase_webhook_statuses_do_not_fall_into_the_unknown_arm() {
        let notification =
            decode_from_request(&braintree_webhook_request(TRANSACTION_SETTLED_XML)).unwrap();
        let transaction = notification.transaction().unwrap();
        assert_eq!(
            transaction.status,
            BraintreeWebhookTransactionStatus::Settled
        );

        // The same bytes through the GraphQL enum are Unknown — this is the bug being avoided.
        assert!(matches!(
            serde_json::from_str::<BraintreePaymentStatus>("\"processor_declined\"").unwrap(),
            BraintreePaymentStatus::Unknown
        ));
        assert!(matches!(
            serde_json::from_str::<BraintreeWebhookTransactionStatus>("\"processor_declined\"")
                .unwrap(),
            BraintreeWebhookTransactionStatus::ProcessorDeclined
        ));

        // Unrecognised statuses degrade to Unspecified, never an invented Pending or Failure.
        let unknown: BraintreeWebhookTransactionStatus =
            serde_json::from_str("\"some_future_status\"").unwrap();
        assert_eq!(unknown, BraintreeWebhookTransactionStatus::Unknown);
        assert_eq!(
            enums::AttemptStatus::from(BraintreePaymentStatus::from(unknown)),
            enums::AttemptStatus::Unspecified
        );
    }

    // §W.5: terminal Braintree state -> terminal UCS state, derived from `<status>` and routed
    // through the single `From<BraintreePaymentStatus> for AttemptStatus` mapping.
    #[test]
    fn payment_webhook_maps_settled_and_settlement_declined() {
        let settled =
            decode_from_request(&braintree_webhook_request(TRANSACTION_SETTLED_XML)).unwrap();
        let response = build_webhook_payment_response(&settled, b"raw").unwrap();
        assert_eq!(response.status, enums::AttemptStatus::Charged);
        assert!(matches!(
            response.resource_id,
            Some(ResponseId::ConnectorTransactionId(ref id)) if id == "sale_id_002"
        ));
        assert_eq!(
            response.connector_response_reference_id.as_deref(),
            Some("order_002")
        );
        // Settlement is not capture: these stay None and PSync reports the captured amount.
        assert!(response.amount_captured.is_none());
        assert!(response.minor_amount_captured.is_none());
        assert!(response.error_code.is_none());

        let declined_xml = TRANSACTION_SETTLED_XML
            .replace("transaction_settled", "transaction_settlement_declined")
            .replace(
                "<status>settled</status>",
                "<status>settlement_declined</status>",
            )
            .replace(
                "<order-id>order_002</order-id>",
                "<order-id>order_002</order-id>\
                 <processor-response-code>4001</processor-response-code>\
                 <processor-response-text>Settlement Declined</processor-response-text>",
            );
        let declined = decode_from_request(&braintree_webhook_request(&declined_xml)).unwrap();
        let response = build_webhook_payment_response(&declined, b"raw").unwrap();
        assert_eq!(response.status, enums::AttemptStatus::Failure);
        assert_eq!(response.error_code.as_deref(), Some("4001"));
        assert_eq!(
            response.error_message.as_deref(),
            Some("Settlement Declined")
        );
    }

    // The misc-event fall-through routes every unmodelled kind to `process_payment_webhook`.
    // With no `<transaction>` at all it must degrade to `Unspecified`, not error — the default
    // trait impl (a hard `WebhooksNotImplemented`) is what this replaces.
    #[test]
    fn unmodelled_kind_falls_through_to_an_unspecified_payment_response() {
        let xml = "<notification><timestamp>2024-01-01T00:00:00Z</timestamp>\
                   <kind>disbursement</kind></notification>";
        let notification = decode_from_request(&braintree_webhook_request(xml)).unwrap();
        let response = build_webhook_payment_response(&notification, b"raw").unwrap();

        assert_eq!(response.status, enums::AttemptStatus::Unspecified);
        assert!(response.resource_id.is_none());
        assert!(response.error_code.is_none());
    }

    // §W.9: `connector_dispute_id` is deliberately None — the shadow normaliser resolves a
    // prism Dispute reference as `connector_dispute_id.or(connector_transaction_id)`, and HS
    // emits the transaction id, so populating it would break byte-for-byte parity.
    #[test]
    fn dispute_reference_uses_the_parent_transaction_id() {
        let notification = decode_from_request(&braintree_webhook_request(DISPUTE_XML)).unwrap();
        match get_webhook_reference(&notification).unwrap() {
            Some(connector_types::WebhookResourceReference::Dispute(reference)) => {
                assert!(reference.connector_dispute_id.is_none());
                assert_eq!(
                    reference.connector_transaction_id.as_deref(),
                    Some("sale_id_001")
                );
            }
            other => panic!("expected a Dispute reference, got {other:?}"),
        }
    }

    // Defect W-7: the shipped `sample_webhook_body` must parse through the REAL code path and
    // produce the real response. The previous fixture was derived from the parser under test,
    // so it could not falsify it; this asserts against values read off the payload itself.
    #[test]
    fn sample_webhook_body_round_trips_through_the_real_path() {
        let request = sample_request();
        let notification = decode_from_request(&request).expect("sample body must parse");

        assert_eq!(notification.kind, "dispute_opened");
        assert_eq!(
            get_status(&notification.kind),
            connector_types::EventType::DisputeOpened
        );

        let dispute = notification
            .dispute()
            .expect("sample body has a subject.dispute");
        assert_eq!(dispute.id, "dummy_dispute_id_001");
        // Lowercase stage — the spelling the old fixture (and the old matcher) could not handle.
        assert_eq!(dispute.kind.as_deref(), Some("chargeback"));

        let response = build_webhook_dispute_response(&notification, &request.body).unwrap();
        assert_eq!(response.amount.to_string(), "1000");
        assert_eq!(response.currency, enums::Currency::USD);
        assert_eq!(response.stage, enums::DisputeStage::Dispute);
        assert_eq!(
            response.connector_response_reference_id.as_deref(),
            Some("dummy_order_001")
        );
    }

    // §W.7.4: `amount` and `currency` on the dispute response are non-Option, so a missing
    // element is a typed error and NEVER a substituted zero.
    #[test]
    fn a_missing_disputed_amount_is_an_error_not_a_zero() {
        let xml = DISPUTE_XML.replace("<amount-disputed>10.00</amount-disputed>", "");
        let notification = decode_from_request(&braintree_webhook_request(&xml)).unwrap();
        let error = build_webhook_dispute_response(&notification, b"raw").unwrap_err();
        assert!(matches!(
            error.current_context(),
            domain_types::errors::WebhookError::WebhookMissingRequiredField {
                field: "amount-disputed"
            }
        ));

        let xml = DISPUTE_XML.replace("<currency-iso-code>USD</currency-iso-code>", "");
        let notification = decode_from_request(&braintree_webhook_request(&xml)).unwrap();
        let error = build_webhook_dispute_response(&notification, b"raw").unwrap_err();
        assert!(matches!(
            error.current_context(),
            domain_types::errors::WebhookError::WebhookMissingRequiredField {
                field: "currency-iso-code"
            }
        ));
    }
}
