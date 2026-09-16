use crate::{connectors::braintree::BraintreeRouterData, types::ResponseRouterData, utils};
use base64::Engine;
use common_enums::enums;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::XmlExt,
    pii,
    types::{AmountConvertor, MinorUnit, StringMajorUnit, StringMajorUnitForConnector},
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
    payment_method_data::{
        DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
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
///
/// Two more placement rules govern the two scheme-level identifiers selected here. They are
/// **different identifiers on different branches of the response** and must never be
/// cross-assigned (tech spec §M.5, RULE M-1):
///
/// 3. `networkTransactionId` (the NTID, all schemes) is NOT a flat field on `Transaction` —
///    selecting `transaction { networkTransactionId }` is a GraphQL *validation* error, which
///    rejects the document before execution and would therefore break EVERY Authorize, not just
///    a mandate one. Its sole carrier in the whole schema is
///    `CreditCardTransactionDetails.networkTransactionId`, reached through
///    `Transaction.paymentMethodSnapshot` — a UNION (`PaymentMethodSnapshot`, 9 members), so the
///    traversal must be an inline fragment. The sibling member `CreditCardDetails` is also a
///    legitimate resolution for a card and carries no NTID, so a snapshot that resolves to any
///    other member arrives as `{}` and must deserialize to "NTID absent", never to a parse error.
/// 4. `mastercardTransactionLinkId` (the Mastercard TLID) hangs off
///    `processorAuthorizationResponse`, is Mastercard-only (null for Visa, Amex, Discover, JCB,
///    Diners, UnionPay) and is read-only telemetry — Braintree has no input field that accepts a
///    TLID. It feeds `network_txn_link_id` and nothing else.
///
/// Both were introspection- and live-verified at `Braintree-Version: 2019-01-01` before being
/// added here (the spec had left `mastercardTransactionLinkId`'s presence at the pin open):
/// transaction `bpr4zzv7` returned `paymentMethodSnapshot.networkTransactionId = 020260915233603`
/// with `mastercardTransactionLinkId: null` on a Visa.
/// The `paymentMethodSnapshot` inline-fragment traversal, on its own so that every mutation
/// that needs a network transaction id splices the SAME selection instead of restating it.
///
/// `paymentMethodSnapshot` is declared on BOTH `Transaction` (spent by
/// `card_transaction_fields!` below, for Authorize / RepeatPayment) and `Verification` (spent by
/// `VAULT_CREDIT_CARD_MUTATION`, for SetupMandate). In both places it is the union
/// `PaymentMethodSnapshot`, and in both places `CreditCardTransactionDetails` is the only member
/// carrying `networkTransactionId` — so one definition serves both and they cannot drift.
/// `CreditCardDetails` is a sibling member that legitimately resolves for a card and carries no
/// NTID, which is why `CreditCardTransactionSnapshot` keeps every level optional (RULE M-2).
macro_rules! payment_method_snapshot_fields {
    () => {
        "paymentMethodSnapshot { ... on CreditCardTransactionDetails { networkTransactionId } }"
    };
}

macro_rules! card_transaction_fields {
    () => {
        concat!(
        "id legacyId createdAt status orderId amount { value currencyCode } \
         processorAuthorizationResponse { legacyCode message cvvResponse avsPostalCodeResponse \
         avsStreetAddressResponse authorizationId additionalInformation retrievalReferenceNumber \
         mastercardTransactionLinkId } ",
        payment_method_snapshot_fields!(),
        " statusHistory { status terminal \
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
        )
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
    /// `initialRequestedAuthorizationAmount` is selected alongside `amount` so a partial capture
    /// can be distinguished from a full one: on a partially captured transaction `amount` is the
    /// cumulative *captured* figure while `initialRequestedAuthorizationAmount` retains the
    /// originally authorized total. Live-verified accepted at `Braintree-Version: 2019-01-01`.
    pub const CAPTURE_TRANSACTION_MUTATION: &str = "mutation captureTransaction($input: CaptureTransactionInput!) { captureTransaction(input: $input) { clientMutationId transaction { id legacyId amount { value currencyCode } initialRequestedAuthorizationAmount { value currencyCode } status } } }";
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
    /// SetupMandate — a REAL zero-amount card verification that also vaults the card.
    ///
    /// `vaultCreditCard` is the canonical SetupMandate mutation, and the only one of the four
    /// candidates that fits (tech spec §SM.2, live-SDL verified):
    ///
    /// * `verifyCreditCard` / `verifyPaymentMethod` require a payment method that is ALREADY
    ///   `MULTI_USE`. Handed the single-use token a freshly tokenized card produces, the former
    ///   answers `errorClass: AUTHORIZATION` / "You are unauthorized to perform this action." —
    ///   which reads like a credentials failure and is not one. They re-verify an existing
    ///   mandate; they cannot create one.
    /// * `vaultPaymentMethod` accepts a single-use token but its
    ///   `PaymentMethodVerificationOptionsInput` carries only `merchantAccountId` and `skip`,
    ///   with no amount override at all.
    /// * `vaultCreditCard` accepts the single-use token, verifies against the card network and
    ///   vaults in ONE call. Verification is ON BY DEFAULT — confirmed live: sending no
    ///   `verification` key still produced `status: VERIFIED`, while `verification: { skip: true }`
    ///   produced `verification: null` and vaulted an UNVERIFIED card. This connector therefore
    ///   never sends `skip`.
    ///
    /// Selection-set rules encoded here, each one a live-verified trap:
    ///
    /// * `Verification.amount` is `@deprecated` in favour of
    ///   `paymentMethodVerificationDetails.amount` and is deliberately NOT selected. The
    ///   deprecated field is also invisible to an introspection that omits
    ///   `includeDeprecated: true`, which is how it keeps being re-added.
    /// * `riskData.liabilityShift` is an OBJECT. A bare selection on it is a document-level
    ///   `SubselectionRequired` validation error, which rejects the whole document before
    ///   execution and would break 100% of SetupMandate calls — so `riskData` is omitted entirely
    ///   rather than selected shallowly.
    /// * `verification.networkTransactionId` does not exist. The NTID is reachable only through
    ///   the `paymentMethodSnapshot` union, spliced from the shared
    ///   `payment_method_snapshot_fields!` so it cannot drift from the Authorize selection set.
    /// * `paymentMethodVerificationDetails` is the union `VerificationDetails`, so its amount
    ///   needs an inline fragment; `CreditCardVerificationDetails` has exactly one field.
    /// * `paymentMethod.id` is the OPAQUE GLOBAL id and is the mandate reference. `legacyId` is
    ///   selected for logs only — passing it to `chargePaymentMethod` fails with `legacyCode
    ///   91565` "Unknown or expired single-use payment method", proven live in both directions.
    ///
    /// Gated live under both `Braintree-Version: 2019-01-01` (this connector's pin) and
    /// `2024-05-01` with byte-identical results, so no version bump is required.
    pub const VAULT_CREDIT_CARD_MUTATION: &str = concat!(
        "mutation vaultCreditCard($input: VaultCreditCardInput!) { vaultCreditCard(input: $input) { ",
        "paymentMethod { id legacyId usage createdAt } ",
        "verification { id legacyId status createdAt merchantAccountId gatewayRejectionReason ",
        "processorResponse { legacyCode message cvvResponse avsPostalCodeResponse ",
        "avsStreetAddressResponse additionalInformation mastercardTransactionLinkId } ",
        "networkResponse { code message } ",
        payment_method_snapshot_fields!(),
        " paymentMethodVerificationDetails { ",
        "... on CreditCardVerificationDetails { amount { value currencyIsoCode } } } } } }"
    );
    pub const DELETE_PAYMENT_METHOD_FROM_VAULT_MUTATION: &str = "mutation deletePaymentMethodFromVault($input: DeletePaymentMethodFromVaultInput!) { deletePaymentMethodFromVault(input: $input) { clientMutationId } }";
    /// Carries the same amount pair as `CAPTURE_TRANSACTION_MUTATION` so a PSync issued after a
    /// partial capture keeps reporting `PartialCharged` instead of walking the payment back to
    /// `Charged`. Live-verified accepted at `Braintree-Version: 2019-01-01`.
    pub const TRANSACTION_QUERY: &str = "query($input: TransactionSearchInput!) { search { transactions(input: $input) { edges { node { id status amount { value currencyCode } initialRequestedAuthorizationAmount { value currencyCode } } } } } }";
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
    /// Level 2/Level 3, shipping and descriptor fields, flattened onto `TransactionInput`.
    /// A merchant-initiated charge goes through the same `TransactionInput` as Authorize, so
    /// it must not silently lose the merchant's descriptor and order data.
    #[serde(flatten)]
    enrichment: TransactionEnrichment,
}

/// Braintree GraphQL `PaymentInitiator` (`TransactionInput.paymentInitiator`).
///
/// The SDL declares eight members, split by who initiates the charge. Only the three
/// **merchant**-initiated ones are modelled here, because this enum is only ever written onto a
/// `RepeatPayment`, which is merchant-initiated by definition:
///
/// * `RECURRING` — fixed amount on a predefined schedule (subscriptions).
/// * `UNSCHEDULED` — stored credential, no fixed schedule or amount (balance top-up).
/// * `INSTALLMENT` — subsequent payment of an installment plan.
///
/// The five omitted members are customer-initiated or orthogonal and are deliberately absent
/// rather than dead arms: `RECURRING_FIRST` / `INSTALLMENT_FIRST` / `MOTO` / `ESTIMATED_MOTO`
/// are CIT values, and `ESTIMATED` encodes amount-uncertainty rather than an initiator (its own
/// SDL comment admits both initiators, so it cannot carry the MIT signal). There is no "CIT"
/// value to reach for on the Authorize side either — per the enum's SDL doc comment, a plain
/// customer-initiated ecommerce transaction omits the field entirely, which is what Authorize
/// already does.
///
/// `INSTALLMENT` is region-gated and fails SILENTLY: where installments are unsupported
/// Braintree re-categorizes the transaction as recurring with no error and no response field
/// announcing the downgrade. Never report upstream that a transaction was processed as an
/// installment on the strength of having sent this value.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentInitiatorType {
    Unscheduled,
    Recurring,
    Installment,
}

impl From<Option<common_enums::MitCategory>> for PaymentInitiatorType {
    /// The MIT category the caller declared drives the initiator, rather than a hardcoded
    /// value (checklist Theme 19).
    ///
    /// `Resubmission` — a retry of a previously declined MIT — has no Braintree counterpart;
    /// it maps to `UNSCHEDULED`, the member whose SDL definition ("not recurring on a
    /// predefined schedule or amount") a dunning retry satisfies. `None` also maps to
    /// `UNSCHEDULED`, preserving the value this connector sent before the category existed.
    fn from(category: Option<common_enums::MitCategory>) -> Self {
        match category {
            Some(common_enums::MitCategory::Recurring) => Self::Recurring,
            Some(common_enums::MitCategory::Installment) => Self::Installment,
            Some(common_enums::MitCategory::Unscheduled)
            | Some(common_enums::MitCategory::Resubmission)
            | None => Self::Unscheduled,
        }
    }
}

/// Braintree GraphQL `ExternalVaultStatus`, narrowed to the one member this connector emits.
///
/// The SDL declares two: `VAULTED` (the credential is already held in the external vault) and
/// `WILL_VAULT` (this transaction is the one establishing it). `WILL_VAULT` is legal only on
/// the vault-establishing *customer*-initiated charge — which this connector does not issue,
/// because Braintree is the vault on every CIT it performs — so it is deliberately absent
/// rather than modelled and unreachable.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalVaultStatus {
    Vaulted,
}

/// Braintree GraphQL `TransactionExternalVaultOptionsInput`
/// (`input.options.externalVault` — a sibling of `input.transaction`, NOT a member of it;
/// `TransactionInput` has no `externalVault` field at all).
///
/// `status` is NON-NULL in the SDL: omitting it is a hard coercion error, so it is never
/// optional and never skipped. The two gateway-enforced pairing rules are both made
/// unrepresentable by construction rather than checked at runtime — `WILL_VAULT` alongside an
/// NTID is rejected outright by Braintree, and this type cannot express it because
/// `ExternalVaultStatus` has no such member.
///
/// **This object must never be emitted for a Braintree-vaulted credential.** Its own SDL doc
/// comment says so — *"Do not use for transactions created from Braintree multi-use payment
/// methods"* — and the gateway does NOT enforce it: a stale, cross-card or entirely fabricated
/// NTID still settles, with no error and no observable signal, so no acceptance test can catch
/// a violation. That is why the regime is decided exactly once, in `BraintreeMandateCredential`,
/// and this struct is reachable only from the externally-vaulted arm.
///
/// The NTID is also withheld on a *customer*-initiated charge against an external vault — the
/// SDL's third, easily-missed clause: *"If the status is VAULTED, but the customer is directly
/// initiating the charge, do not pass this value."* `RepeatPayment` is merchant-initiated by
/// definition, so that case does not arise on this flow.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionExternalVaultOptions {
    status: ExternalVaultStatus,
    /// Wrapped for symmetry with `payment_method_id` and with the reference connectors, which
    /// carry the NTID as `Secret<String>` on the request side (cybersource
    /// `previous_transaction_id`, stripe `mit_exemption.network_transaction_id`).
    verifying_network_transaction_id: Secret<String>,
}

impl TransactionExternalVaultOptions {
    /// The only constructor: a merchant-initiated charge on an externally vaulted credential.
    fn vaulted_mit(network_transaction_id: Secret<String>) -> Self {
        Self {
            status: ExternalVaultStatus::Vaulted,
            verifying_network_transaction_id: network_transaction_id,
        }
    }
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
    amount_converter: &'static (dyn AmountConvertor<Output = StringMajorUnit> + Sync),
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

/// The three request-side inputs `build_transaction_enrichment` needs beyond `PaymentFlowData`.
///
/// Exists so Authorize and RepeatPayment share one enrichment builder instead of each growing
/// its own copy: a merchant-initiated charge uses the same `TransactionInput` as the
/// customer-initiated one, so the Level 2/Level 3, shipping and descriptor mapping must not be
/// allowed to drift between them.
trait BraintreeEnrichmentSource {
    fn currency(&self) -> enums::Currency;
    /// Flow-level order tax, used as the fallback when `l2_l3_data` carries none. Only
    /// `PaymentsAuthorizeData` has such a field; `RepeatPaymentData` has no equivalent, so its
    /// impl returns `None` and the `l2_l3_data` value is the only source.
    fn order_tax_amount(&self) -> Option<MinorUnit>;
    fn billing_descriptor(&self) -> Option<&connector_types::BillingDescriptor>;
}

impl<T: PaymentMethodDataTypes> BraintreeEnrichmentSource for PaymentsAuthorizeData<T> {
    fn currency(&self) -> enums::Currency {
        self.currency
    }
    fn order_tax_amount(&self) -> Option<MinorUnit> {
        self.order_tax_amount
    }
    fn billing_descriptor(&self) -> Option<&connector_types::BillingDescriptor> {
        self.billing_descriptor.as_ref()
    }
}

impl<T: PaymentMethodDataTypes> BraintreeEnrichmentSource for RepeatPaymentData<T> {
    fn currency(&self) -> enums::Currency {
        self.currency
    }
    fn order_tax_amount(&self) -> Option<MinorUnit> {
        None
    }
    fn billing_descriptor(&self) -> Option<&connector_types::BillingDescriptor> {
        self.billing_descriptor.as_ref()
    }
}

/// Builds the Level 2 / Level 3, shipping and dynamic-descriptor block shared by the
/// customer-initiated (Authorize) and merchant-initiated (RepeatPayment) card charges.
///
/// Every money field goes through the connector's `StringMajorUnit` amount converter —
/// including `tax.taxAmount`, which the SDL types as the `Amount` scalar while the rest of
/// this area is plain `String`. Both land on the wire as the same major-unit decimal
/// string, so one converter covers both.
fn build_transaction_enrichment<Req: BraintreeEnrichmentSource>(
    request: &Req,
    flow_data: &PaymentFlowData,
    amount_converter: &'static (dyn AmountConvertor<Output = StringMajorUnit> + Sync),
) -> Result<TransactionEnrichment, Report<IntegrationError>> {
    let currency = request.currency();
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
        .or(request.order_tax_amount())
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
    let descriptor = request.billing_descriptor().and_then(|billing_descriptor| {
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

/// Which vault holds the credential a merchant-initiated charge is about to spend.
///
/// This is the whole regime decision, taken exactly once, in one place, from the shape of
/// `MandateReferenceId`. It exists as a type rather than as a pair of booleans because the rule
/// it encodes is not enforced by the gateway: sending `options.externalVault` on a
/// Braintree-vaulted token is accepted and settles normally, and sending a wrong NTID is
/// accepted and settles normally, so a mistake here surfaces only as degraded interchange
/// qualification and elevated issuer declines — never as a test failure.
#[derive(Debug)]
pub enum BraintreeMandateCredential {
    /// **Braintree-vaulted.** `paymentMethodId` is a Braintree *multi-use* token minted by the
    /// vaulting CIT and handed back as `connector_mandate_id`. Braintree holds the credential
    /// and replays the stored-credential chain itself, so the merchant-initiated signal is
    /// `transaction.paymentInitiator` and nothing else: `options.externalVault` MUST be
    /// omitted, per the input type's own SDL prohibition on Braintree multi-use payment
    /// methods. Supplying an NTID here would be redundant at best and would assert something
    /// false at worst.
    BraintreeVaulted { payment_method_id: Secret<String> },
    /// **Externally vaulted.** UCS (or an upstream vault) holds the credential; all we hold is
    /// the NTID captured from the CIT. `paymentMethodId` is a *single-use* token minted for
    /// this transaction alone via `PaymentMethodService/Tokenize`, and the stored-credential
    /// chain is asserted explicitly through
    /// `options.externalVault.verifyingNetworkTransactionId`.
    ExternallyVaulted {
        payment_method_id: Secret<String>,
        network_transaction_id: Secret<String>,
    },
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
        BraintreeMandateCredential,
        BraintreeMeta,
    )> for MandatePaymentRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        (item, credential, metadata): (
            BraintreeRouterData<
                RouterDataV2<
                    RepeatPayment,
                    PaymentFlowData,
                    RepeatPaymentData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            BraintreeMandateCredential,
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

        // The regime split, and the ONLY place `options.externalVault` can be produced.
        let (payment_method_id, external_vault) = match credential {
            BraintreeMandateCredential::BraintreeVaulted { payment_method_id } => {
                (payment_method_id, None)
            }
            BraintreeMandateCredential::ExternallyVaulted {
                payment_method_id,
                network_transaction_id,
            } => (
                payment_method_id,
                Some(TransactionExternalVaultOptions::vaulted_mit(
                    network_transaction_id,
                )),
            ),
        };

        // Same `options` object Authorize builds — `billingAddress` is a sibling of
        // `input.transaction`, not a member of it — so an MIT keeps the AVS data the CIT sent
        // instead of silently dropping it. `options` is omitted entirely when both halves are
        // absent, which is the pre-existing behaviour for a Braintree-vaulted MIT with no
        // billing address on the request.
        let billing_address = build_billing_address(&item.router_data.resource_common_data);
        let options = (billing_address.is_some() || external_vault.is_some()).then_some(
            CreditCardTransactionOptions {
                billing_address,
                // Braintree-hosted and external-MPI 3DS both belong to the customer-initiated
                // leg: a merchant-initiated charge is unattended, so there is no cardholder to
                // authenticate and nothing to declare here.
                three_d_secure_authentication: None,
                external_vault,
            },
        );

        let enrichment = build_transaction_enrichment(
            &item.router_data.request,
            &item.router_data.resource_common_data,
            item.connector.amount_converter,
        )?;

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
                payment_initiator: PaymentInitiatorType::from(
                    item.router_data.request.mit_category,
                ),
                enrichment,
            }),
        );
        Ok(Self {
            query,
            variables: VariablePaymentInput {
                input: PaymentInput {
                    payment_method_id,
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
    /// Mastercard Transaction Link Identifier (TLID). Present only on Mastercard
    /// authorizations — `null` on every other scheme — and read-only: Braintree exposes no
    /// input field that accepts a TLID. Surfaced as `network_txn_link_id` and never as
    /// `network_txn_id`, which is a different identifier on a different branch of the
    /// response (RULE M-1; see the `card_transaction_fields!` doc comment).
    pub mastercard_transaction_link_id: Option<String>,
}

/// `Transaction.paymentMethodSnapshot` narrowed to its `CreditCardTransactionDetails` member.
///
/// `paymentMethodSnapshot` is a GraphQL UNION with nine members and the selection set asks for
/// exactly one inline fragment, so a snapshot that resolves to any other member — including
/// `CreditCardDetails`, which is a legitimate resolution for a card and carries no NTID — comes
/// back as the empty object `{}`. Both levels are therefore optional and no field is required:
/// an unmatched union member must mean "NTID absent", never a failed response parse (RULE M-2).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardTransactionSnapshot {
    /// Opaque scheme identifier, observed as a 15-digit numeric string. Never parsed, padded
    /// or validated. Nullable even on `CreditCardTransactionDetails`.
    #[serde(default)]
    pub network_transaction_id: Option<String>,
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
    /// `Transaction.paymentMethodSnapshot`, the only carrier of the scheme NTID. Optional at
    /// both levels because the union fragment need not match — see
    /// `CreditCardTransactionSnapshot`.
    #[serde(default)]
    payment_method_snapshot: Option<CreditCardTransactionSnapshot>,
}

/// Shapes the AVS / CVV / processor verdicts into the `payment_checks` blob UCS hands back on
/// `PaymentFlowData.connector_response`.
///
/// Shared deliberately by every Braintree card flow that receives a processor verdict:
/// Authorize and RepeatPayment read it off `Transaction.processorAuthorizationResponse`, and
/// SetupMandate reads it off `Verification.processorResponse`. The two SDL types declare the
/// same AVS/CVV triple (`cvvResponse`, `avsPostalCodeResponse`, `avsStreetAddressResponse`, all
/// typed `AvsCvvResponseCode`) plus `legacyCode`/`message`, so one `BraintreeProcessorResponse`
/// models both and one builder serves both. Forking this for SetupMandate would let a merchant's
/// AVS handling silently diverge between "verify the card" and "charge the card".
///
/// `authorization_id` and `retrieval_reference_number` exist only on the transaction shape and
/// arrive as `None` on a verification; they are emitted either way so the JSON key set a caller
/// parses does not change between flows.
fn build_card_payment_checks_response(
    processor_response: &BraintreeProcessorResponse,
) -> ConnectorResponseData {
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

    ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: None,
            payment_checks: Some(payment_checks),
            card_network: None,
            domestic_network: None,
            auth_code: processor_response.authorization_id.clone(),
        },
    )
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

    /// The scheme network transaction id, for `PaymentsResponseData::network_txn_id`.
    ///
    /// Braintree assigns the NTID at authorization time, so it is present on declines as well
    /// as approvals; the caller must not gate reading it on a success status. Absence is never
    /// an error — the snapshot may resolve to a non-card union member, and the field is
    /// nullable even when it does not.
    fn network_transaction_id(&self) -> Option<String> {
        self.payment_method_snapshot
            .as_ref()?
            .network_transaction_id
            .clone()
    }

    /// The Mastercard TLID, for `PaymentsResponseData::network_txn_link_id`. Read only from
    /// `processorAuthorizationResponse` — never from the snapshot, which carries the NTID and
    /// not this (RULE M-1). `None` on every non-Mastercard scheme, which is the normal case.
    fn network_transaction_link_id(&self) -> Option<String> {
        self.processor_response()?
            .mastercard_transaction_link_id
            .clone()
    }

    /// Surfaces the AVS and CVV verdicts on `PaymentFlowData.connector_response` so callers
    /// can reason about address/CVV mismatches without re-fetching the transaction.
    ///
    /// Delegates to [`build_card_payment_checks_response`], which SetupMandate's zero-amount
    /// verification also calls — the two flows read the same AVS/CVV codes off the same SDL
    /// shape, so they share one builder rather than each shaping its own `payment_checks` blob.
    fn build_connector_response_data(&self) -> Option<ConnectorResponseData> {
        Some(build_card_payment_checks_response(
            self.processor_response()?,
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
                        // Captured from `paymentMethodSnapshot` so a later merchant-initiated
                        // RepeatPayment can replay the stored-credential chain. Absent NTID is
                        // `None`, never an error.
                        network_txn_id: transaction_data.network_transaction_id(),
                        network_txn_link_id: transaction_data.network_transaction_link_id(),
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
    /// `errors[].extensions.errorClass`. Braintree's own classification of WHY the document was
    /// rejected, and the only signal in the GraphQL error envelope that distinguishes "this
    /// request will never succeed as sent" from "something broke on the way". SetupMandate
    /// branches its `attempt_status` on it; the other flows deserialize it and ignore it.
    pub error_class: Option<BraintreeErrorClass>,
}

/// Braintree GraphQL `errors[].extensions.errorClass`.
///
/// Not an SDL enum — it is a free-form string in the error envelope — so `Unknown` is a real,
/// reachable arm rather than a formality, and an unrecognised class must never fail the parse.
/// The split below is the whole point of modelling it: only a class that means "Braintree
/// definitively refused this document, nothing was verified and nothing was vaulted" may produce
/// a terminal attempt status.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreeErrorClass {
    /// Input the gateway rejected outright (bad merchant account, spent or expired token, …).
    Validation,
    /// Credentials are valid but may not perform this operation.
    Authorization,
    /// Credentials were not accepted.
    Authentication,
    /// The addressed resource does not exist.
    NotFound,
    /// The client is not permitted to call this API at all.
    UnsupportedClient,
    /// Rate/quota limit — retryable, so NOT terminal.
    ResourceLimit,
    /// Braintree-side fault — ambiguous, so NOT terminal.
    Internal,
    /// Braintree or a downstream dependency is unavailable — ambiguous, so NOT terminal.
    ServiceAvailability,
    #[serde(other)]
    Unknown,
}

impl BraintreeErrorClass {
    /// `true` when the class means the request was refused before anything happened, so the
    /// attempt can be closed as failed without risking a false FAILURE on work that may have
    /// been performed (review Theme 1). Everything else — including `Unknown` — is ambiguous
    /// and must leave the attempt non-terminal.
    fn is_terminal_rejection(self) -> bool {
        match self {
            Self::Validation
            | Self::Authorization
            | Self::Authentication
            | Self::NotFound
            | Self::UnsupportedClient => true,
            Self::ResourceLimit | Self::Internal | Self::ServiceAvailability | Self::Unknown => {
                false
            }
        }
    }
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
                        // Captured from `paymentMethodSnapshot` so a later merchant-initiated
                        // RepeatPayment can replay the stored-credential chain. Absent NTID is
                        // `None`, never an error.
                        network_txn_id: transaction_data.network_transaction_id(),
                        network_txn_link_id: transaction_data.network_transaction_link_id(),
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

/// `attempt_status` for a refund that failed inside the GraphQL **envelope** — HTTP 200 with a
/// populated `errors[]` and no `data.refundTransaction`, so Braintree never created a refund.
///
/// Branches on Braintree's own `errorClass` through the same `is_terminal_rejection` classifier
/// SetupMandate uses, rather than stamping one status on every failure (review Theme 1). A
/// `VALIDATION` rejection — a refund against an unsettled transaction, a bad merchant account id —
/// will never succeed as sent and is safely terminal. An `INTERNAL`, `SERVICE_AVAILABILITY` or
/// `RESOURCE_LIMIT` fault, or a class this enum does not recognise, is ambiguous: the refund may
/// yet exist, so it must stay non-terminal and `None` leaves the caller's own status standing.
///
/// Without this, `generate_refund_response` reads `attempt_status: None` and reports *every*
/// envelope error — a definite decline included — as `REFUND_STATUS_UNSPECIFIED`.
///
/// The axis here is "could a refund exist?", not "did the gateway decline?". That is what makes
/// terminal safe for classes that are not declines at all — `AUTHENTICATION` and `AUTHORIZATION`
/// included. An envelope error means `data.refundTransaction` is absent, so Braintree created
/// nothing: there is no refund for RSync to converge on, and reporting non-terminal would leave
/// the refund polling forever against an id that will never exist — the other half of Theme 1,
/// and the exact PayNearMe failure it was written from. The classes left ambiguous are the ones
/// where Braintree's own fault could mean the refund WAS created and only the response was lost.
///
/// Deliberately NOT applied to RSync. `NotFound` classifies as terminal here because on Execute it
/// means the transaction being refunded does not exist; on a *sync* the same class can equally mean
/// the refund is simply not indexed yet, and closing a refund as failed for that reason is exactly
/// the false-terminal outcome Theme 1 forbids.
fn refund_envelope_attempt_status(errors: &[ErrorDetails]) -> Option<FlowStatus> {
    let terminal = errors
        .iter()
        .filter_map(|error| error.extensions.as_ref())
        .filter_map(|extensions| extensions.error_class)
        .any(BraintreeErrorClass::is_terminal_rejection);
    terminal.then_some(FlowStatus::Refund(enums::RefundStatus::Failure))
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
                    build_error_response(&error_response.errors, item.http_code).map_err(|err| {
                        let mut err = *err;
                        err.attempt_status = refund_envelope_attempt_status(&error_response.errors);
                        err
                    })
                }
                BraintreeRefundResponse::SuccessResponse(refund_data) => {
                    let refund_data = refund_data.data.refund_transaction.refund;
                    let refund_status = enums::RefundStatus::from(refund_data.status.clone());
                    if utils::is_refund_failure(refund_status) {
                        let mut error_response = create_failure_error_response(
                            refund_data.status,
                            Some(refund_data.id),
                            item.http_code,
                        );
                        // Braintree has EXPLICITLY declined this refund (`BraintreeRefundStatus::Failed`),
                        // so the outcome is terminal on the refund flow. `generate_refund_response`
                        // reads *only* `ErrorResponse.attempt_status`, so leaving it `None` reports a
                        // hard decline as `REFUND_STATUS_UNSPECIFIED`.
                        //
                        // Set here and only here, never inside `create_failure_error_response` or the
                        // flow-agnostic `build_error_response`: those are shared with Capture / Void /
                        // PSync, where `None` is correct (the payment path falls back to
                        // `router_data.status`) and where transport / 4xx / 5xx / GraphQL-envelope
                        // errors are ambiguous and must not be stamped terminal.
                        error_response.attempt_status =
                            Some(FlowStatus::Refund(enums::RefundStatus::Failure));
                        Err(error_response)
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
        // Currency for the cross-currency guard, resolved in precedence order:
        //   1. `refund_money` — the first-class `RefundServiceGetRequest.refund_amount` field,
        //      which is what Hyperswitch actually populates on an RSync.
        //   2. the legacy `"currency"` key inside `refund_connector_metadata`, kept so a caller
        //      that still sends the metadata shape keeps working.
        //   3. neither — and that is deliberately NOT an error.
        //
        // Why absence is not a failure: the request built below carries only the refund id
        // (`REFUND_QUERY` + `RefundSearchInput { id }`). Neither an amount nor a currency ever
        // reaches Braintree on this flow, so a missing currency cannot produce a wrong call —
        // hard-erroring on it could only reject syncs that would otherwise have succeeded.
        // Both sources are optional by contract (`RefundSyncData.refund_money` and
        // `.refund_connector_metadata` are `Option`, behind optional proto fields), so absence is
        // the ordinary case rather than an anomaly. When a currency *is* resolvable the guard still
        // runs, so a genuine cross-currency mismatch is still rejected.
        let currency = item
            .router_data
            .request
            .refund_money
            .as_ref()
            .map(|money| money.currency)
            .or_else(|| {
                extract_metadata_field::<enums::Currency>(
                    &item.router_data.request.refund_connector_metadata,
                    "currency",
                )
                .ok()
            });
        if let Some(currency) = currency {
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

/// Braintree GraphQL `CreditCardInput`. Every member is nullable in the SDL, `cvv` included —
/// introspected at `Braintree-Version: 2019-01-01` and confirmed live: a `tokenizeCreditCard`
/// with no `cvv` returns a token.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditCardData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    number: RawCardNumber<T>,
    expiration_year: Secret<String>,
    expiration_month: Secret<String>,
    /// Absent on a stored credential replayed unattended: no cardholder is present to enter a
    /// CVV and scheme rules forbid retaining one. Omitted rather than sent empty — an empty
    /// string is a *failed* CVV check, not an absent one.
    #[serde(skip_serializing_if = "Option::is_none")]
    cvv: Option<Secret<String>>,
    cardholder_name: Secret<String>,
}

/// `TokenizeCreditCardInput.creditCard`, in the two carriers a PAN reaches this connector in.
///
/// The split exists because the two UCS variants type the card number differently, not because
/// Braintree wants two shapes — both serialize to the same `CreditCardInput` object:
///
/// * `PaymentMethodData::Card` carries `RawCardNumber<T>`, generic so that a vault-token
///   holder's templated PAN (`{{$card_number}}`) passes through untouched.
/// * `PaymentMethodData::CardDetailsForNetworkTransactionId` types its number as a concrete
///   `cards::CardNumber` — always a real PAN, never a vault template — so that arm is pinned to
///   `DefaultPCIHolder`. This mirrors the established precedent in `worldpay/requests.rs`,
///   whose `PaymentInstrument::RawCardForNTI` is likewise `RawCardDetails<DefaultPCIHolder>`
///   inside an otherwise generic enum.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum BraintreeTokenizeCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Raw(CreditCardData<T>),
    NetworkTransactionId(CreditCardData<DefaultPCIHolder>),
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
    credit_card: BraintreeTokenizeCard<T>,
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
                        credit_card: BraintreeTokenizeCard::Raw(CreditCardData {
                            number: card_data.card_number,
                            expiration_year: card_data.card_exp_year,
                            expiration_month: card_data.card_exp_month,
                            cvv: Some(card_data.card_cvc),
                            cardholder_name: item
                                .router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                                .unwrap_or(Secret::new("".to_string())),
                        }),
                    },
                },
            }),
            // The card behind a network-transaction-id mandate. Braintree accepts no raw PAN on
            // any transaction mutation, so a merchant-initiated charge on an externally vaulted
            // credential must first exchange that PAN for a single-use token — which is exactly
            // what this flow does. Hyperswitch reaches here because Braintree declares a
            // `[tokenization]` entry in `config/*.toml`: on a network-transaction-id MIT it runs
            // `PaymentMethodService/Tokenize` first, then forwards the token it gets back to
            // `RecurringPaymentService/Charge`, where `braintree_single_use_token` spends it as
            // `paymentMethodId` alongside `options.externalVault`. Without this arm the whole
            // externally-vaulted regime is unreachable from Hyperswitch.
            //
            // Tokenizing it is the SAME operation as tokenizing a card — `tokenizeCreditCard`
            // stores card data, it does not authorize — with one difference: there is no CVV.
            // A stored credential replayed unattended has no cardholder present to supply one
            // and scheme rules forbid retaining it, so the variant carries no CVV field at all.
            // `CreditCardInput.cvv` is nullable at the pin and a tokenization without it
            // succeeds, so the field is omitted rather than faked with an empty string.
            PaymentMethodData::CardDetailsForNetworkTransactionId(card_data) => Ok(Self {
                query: constants::TOKENIZE_CREDIT_CARD.to_string(),
                variables: VariableInput {
                    input: InputData {
                        credit_card: BraintreeTokenizeCard::NetworkTransactionId(CreditCardData {
                            number: RawCardNumber(card_data.card_number.clone()),
                            // Braintree accepts either width; the 4-digit form is sent so a
                            // two-digit year stored on the mandate is never re-interpreted.
                            expiration_year: card_data.get_expiry_year_4_digit(),
                            expiration_month: card_data.card_exp_month.clone(),
                            cvv: None,
                            // The mandate carries its own cardholder name; fall back to the
                            // billing name the Card arm uses when it does not.
                            cardholder_name: card_data.card_holder_name.clone().unwrap_or_else(
                                || {
                                    item.router_data
                                        .resource_common_data
                                        .get_optional_billing_full_name()
                                        .unwrap_or(Secret::new("".to_string()))
                                },
                            ),
                        }),
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
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
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

/// Braintree GraphQL `MonetaryAmount`: a decimal **major-unit** string plus an ISO currency
/// code, e.g. `{"value":"10.00","currencyCode":"USD"}`. `value` is typed as `StringMajorUnit`
/// rather than a bare `String` so it can only leave this module through the shared amount
/// converter, never through a hand-rolled decimal parse.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeMonetaryAmount {
    value: StringMajorUnit,
    /// Braintree's `CurrencyCodeAlpha`, kept as a `String` rather than `enums::Currency` on
    /// purpose. This field is only ever used to pick the converter and to check that the two
    /// amounts being compared share a currency; typing it as the enum would make an unmapped
    /// ISO code a hard deserialization failure that took the whole Capture/PSync response down,
    /// to guard a comparison that is allowed to be skipped. It is parsed into `enums::Currency`
    /// at the point of use, where an unparsable code falls back to the unrefined status.
    currency_code: String,
}

/// Refine a mapped `Charged` into `PartialCharged` when Braintree reports that less than the
/// originally authorized amount has been captured.
///
/// On a partially captured transaction `Transaction.amount` is the cumulative *captured* figure
/// while `Transaction.initialRequestedAuthorizationAmount` retains the authorized total, so the
/// gateway response is the only place this comparison is available: UCS never carries the
/// authorized total onto a Capture (the `PaymentFlowData` built from `PaymentServiceCaptureRequest`
/// hardcodes `minor_amount_authorized: None`, `PaymentsCaptureData` has no authorized-total field,
/// and the proto request has none either).
///
/// Deliberately conservative — it only ever narrows `Charged` to `PartialCharged`, never the other
/// way, and falls back to `Charged` whenever the answer is not certain:
/// `initialRequestedAuthorizationAmount` is nullable in the SDL, the values are decimal strings
/// that need a parseable currency to reach minor units, and a cross-currency pair would make the
/// comparison meaningless. `Charged` is the pre-existing behaviour, so the fallback is a no-op.
fn refine_capture_status(
    status: enums::AttemptStatus,
    captured: Option<&BraintreeMonetaryAmount>,
    authorized: Option<&BraintreeMonetaryAmount>,
) -> enums::AttemptStatus {
    if status != enums::AttemptStatus::Charged {
        return status;
    }
    let (Some(captured), Some(authorized)) = (captured, authorized) else {
        return status;
    };
    if captured.currency_code != authorized.currency_code {
        return status;
    }
    // Both sides go through the same converter, so the comparison is on minor units and never on
    // the lexical ordering of the decimal strings.
    let to_minor = |amount: &BraintreeMonetaryAmount| -> Option<MinorUnit> {
        let currency = amount.currency_code.parse::<enums::Currency>().ok()?;
        StringMajorUnitForConnector
            .convert_back(amount.value.clone(), currency)
            .ok()
    };
    match (to_minor(captured), to_minor(authorized)) {
        (Some(captured_minor), Some(authorized_minor)) if captured_minor < authorized_minor => {
            enums::AttemptStatus::PartialCharged
        }
        _ => status,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponseTransactionBody {
    id: String,
    status: BraintreePaymentStatus,
    amount: Option<BraintreeMonetaryAmount>,
    initial_requested_authorization_amount: Option<BraintreeMonetaryAmount>,
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
                let status = refine_capture_status(
                    enums::AttemptStatus::from(transaction_data.status.clone()),
                    transaction_data.amount.as_ref(),
                    transaction_data
                        .initial_requested_authorization_amount
                        .as_ref(),
                );
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
                        Some(void_data.id),
                        item.http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        // `reverseTransaction` returns `union TransactionReversal = Refund | Transaction`:
                        // an unsettled transaction is VOIDED and comes back under its own id, a settled
                        // one is refunded and comes back under the refund's id. Either way the id in the
                        // response is the one a subsequent sync can address, so it is reported rather
                        // than discarded (`NoResponseId` used to drop it and left Void the only flow on
                        // this connector with no identifier).
                        resource_id: ResponseId::ConnectorTransactionId(void_data.id),
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
#[serde(rename_all = "camelCase")]
pub struct NodeData {
    id: String,
    status: BraintreePaymentStatus,
    amount: Option<BraintreeMonetaryAmount>,
    initial_requested_authorization_amount: Option<BraintreeMonetaryAmount>,
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
                // Same refinement as Capture, so a PSync issued after a partial capture does not
                // walk the payment back from `PartialCharged` to `Charged`.
                let status = refine_capture_status(
                    enums::AttemptStatus::from(edge_data.node.status.clone()),
                    edge_data.node.amount.as_ref(),
                    edge_data
                        .node
                        .initial_requested_authorization_amount
                        .as_ref(),
                );
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
                // Never on Authorize. This flow is customer-initiated, and the SDL forbids the
                // NTID on a customer-initiated charge even against an external vault; the
                // `WILL_VAULT` bootstrap is likewise not this connector's model, because every
                // CIT it performs vaults into Braintree's own vault.
                external_vault: None,
            },
        );

        let enrichment = build_transaction_enrichment(
            &item.router_data.request,
            &item.router_data.resource_common_data,
            item.connector.amount_converter,
        )?;

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

// ---------------------------------------------------------------------------
// Incoming webhooks
//
// Braintree posts `application/x-www-form-urlencoded` with exactly two fields,
// `bt_signature` and `bt_payload`. `bt_payload` is line-wrapped base64 of an XML
// `<notification>` whose resource object is nested TWO levels deep:
//
//     notification > subject > (transaction | dispute | check | transaction-review)
//
// Every element name on the wire is kebab-case, every typed element carries a
// `type="..."` attribute, and every nullable element is emitted as a self-closing
// `<foo nil="true"/>`. Consequences encoded below and verified by round-tripping
// Braintree's own SDK sample payloads through this module:
//
//  * every struct carries `#[serde(rename_all = "kebab-case")]`;
//  * no struct carries `deny_unknown_fields` — that would turn every `type=`
//    attribute into a parse failure;
//  * every optional element is `Option<_> + #[serde(default)]`, and every element
//    that can be `nil="true"` and is NOT plain `Option<String>` goes through
//    `deserialize_nil_aware`, because quick-xml surfaces a nil element as an
//    empty value (`Some("")`) rather than as a missing field. `""` is not a valid
//    `Decimal`, `Currency` or datetime, so without this one nil element would fail
//    the whole notification — and a dropped notification is a lost dispute with a
//    reply-by deadline attached.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct BraintreeWebhookResponse {
    pub bt_signature: String,
    pub bt_payload: String,
}

/// Braintree emits an absent optional element as `<foo nil="true"/>`, which quick-xml
/// deserializes as an empty value rather than as a missing field. Map empty/whitespace
/// content to `None` before delegating, so one nil element cannot fail the whole
/// notification parse.
///
/// The delegation goes through `serde_json::Value::String` rather than
/// `serde::de::value::StringDeserializer` deliberately: the string deserializer does not
/// implement `deserialize_newtype_struct`, so `StringMajorUnit` (a newtype over `String`)
/// fails against it with "invalid type: string, expected tuple struct".
fn deserialize_nil_aware<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) => serde_json::from_value::<T>(serde_json::Value::String(value.to_owned()))
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// Braintree's Relay **global** id for a legacy entity id: unpadded standard-alphabet base64
/// of `"<type>_<legacy id>"`.
///
/// Braintree runs two id spaces and the webhook surface uses the one UCS does not store.
/// The GraphQL Authorize/Refund legs write `transaction.id` / `refund.id` — the global id —
/// into `connector_transaction_id` / `connector_refund_id`, while every XML webhook body
/// carries the legacy id in `<id>`. Emitting the legacy id from a webhook reference therefore
/// resolves nothing: the caller looks up an id it never wrote.
///
/// The encoding is pinned against real sandbox values, not assumed. A captured GraphQL
/// response in the connector's spec shows both forms of the same transaction side by side —
/// `"id":"dHJhbnNhY3Rpb25fZHowd2hyOTQ","legacyId":"dz0whr94"` — and
/// `dHJhbnNhY3Rpb25fZHowd2hyOTQ` is base64 of `transaction_dz0whr94`. Two details matter and
/// both are load-bearing:
///
/// * **No padding.** `"transaction_dz0whr94"` is 20 bytes, the one length that *requires* a
///   trailing `=` under padded base64 — and the real id has none. Hence `STANDARD_NO_PAD`
///   rather than this module's `BASE64_ENGINE`, which pads.
/// * **Standard alphabet.** Legacy ids are lowercase alphanumeric, so the encoded bytes never
///   reach a `+` or `/` and the standard and URL-safe alphabets happen to coincide on every
///   real id. Standard is named explicitly so the choice is deliberate rather than accidental.
///
/// Idempotent: a value that already decodes to `"<prefix>_…"` is passed through unchanged, so
/// the function stays correct if Braintree ever starts emitting global ids in the XML.
fn to_braintree_global_id(prefix: &str, id: &str) -> String {
    let engine = base64::engine::general_purpose::STANDARD_NO_PAD;
    let already_global = engine
        .decode(id.trim_end_matches('='))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .is_some_and(|decoded| decoded.starts_with(&format!("{prefix}_")));

    if already_global {
        return id.to_string();
    }

    engine.encode(format!("{prefix}_{id}"))
}

/// `<type>` prefixes of the global id space. Only the two UCS actually resolves against are
/// modelled; a dispute id is reported, never matched, so it is left in the gateway's own form.
const GLOBAL_ID_PREFIX_TRANSACTION: &str = "transaction";
const GLOBAL_ID_PREFIX_REFUND: &str = "refund";

/// The complete Braintree notification `kind` catalogue — all 41 values of
/// `Braintree::WebhookNotification::Kind`, plus `Unknown` for anything Braintree adds
/// later.
///
/// This is a parsed view of `Notification::kind`, not the deserialization target: the
/// wire field stays a `String` so that (a) an unrecognised kind can never fail the parse
/// and (b) `get_webhook_resource_object` still surfaces the kind Braintree actually sent
/// rather than the word "unknown". Every mapping below matches on this enum with no
/// `_` arm, so a new kind is a compile error rather than a silent mismapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BraintreeWebhookKind {
    // --- transaction family (5) -------------------------------------------------
    TransactionSettled,
    TransactionSettlementDeclined,
    /// Deprecated by Braintree. A funding event; carries no `<status>` at all.
    TransactionDisbursed,
    /// Fraud Protection. Subject is `<transaction-review>`, not `<transaction>`.
    TransactionReviewed,
    /// Describes a NEW transaction linked by `<retried-transaction-id>`, not a status
    /// change on the original attempt.
    TransactionRetried,
    // --- refund family (1) ------------------------------------------------------
    RefundFailed,
    // --- dispute family (8) -----------------------------------------------------
    DisputeOpened,
    DisputeWon,
    DisputeLost,
    DisputeAccepted,
    DisputeAutoAccepted,
    DisputeDisputed,
    DisputeExpired,
    DisputeUnderReview,
    // --- connectivity (1) -------------------------------------------------------
    /// Sent by the Control Panel's "Check URL" button. Subject is a single
    /// `<check type="boolean">true</check>`; there is no payment, refund or dispute.
    Check,
    // --- out of scope (26) ------------------------------------------------------
    // Enumerated rather than folded into `Unknown` so the mapping below records a
    // deliberate decision for each one instead of leaning on an unaudited catch-all.
    SubscriptionBillingSkipped,
    SubscriptionCanceled,
    SubscriptionChargedSuccessfully,
    SubscriptionChargedUnsuccessfully,
    SubscriptionExpired,
    SubscriptionTrialEnded,
    SubscriptionWentActive,
    SubscriptionWentPastDue,
    Disbursement,
    AccountUpdaterDailyReport,
    LocalPaymentCompleted,
    LocalPaymentExpired,
    LocalPaymentFunded,
    LocalPaymentReversed,
    PaymentMethodCustomerDataUpdated,
    PaymentMethodRevokedByCustomer,
    GrantedPaymentInstrumentRevoked,
    GrantedPaymentMethodRevoked,
    GrantorUpdatedGrantedPaymentMethod,
    RecipientUpdatedGrantedPaymentMethod,
    OauthAccessRevoked,
    PartnerMerchantConnected,
    PartnerMerchantDisconnected,
    PartnerMerchantDeclined,
    ConnectedMerchantStatusTransitioned,
    ConnectedMerchantPaypalStatusChanged,
    /// Any kind Braintree adds after this was written.
    Unknown,
}

impl BraintreeWebhookKind {
    pub(super) fn from_wire(kind: &str) -> Self {
        match kind {
            "transaction_settled" => Self::TransactionSettled,
            "transaction_settlement_declined" => Self::TransactionSettlementDeclined,
            "transaction_disbursed" => Self::TransactionDisbursed,
            "transaction_reviewed" => Self::TransactionReviewed,
            "transaction_retried" => Self::TransactionRetried,
            "refund_failed" => Self::RefundFailed,
            "dispute_opened" => Self::DisputeOpened,
            "dispute_won" => Self::DisputeWon,
            "dispute_lost" => Self::DisputeLost,
            "dispute_accepted" => Self::DisputeAccepted,
            "dispute_auto_accepted" => Self::DisputeAutoAccepted,
            "dispute_disputed" => Self::DisputeDisputed,
            "dispute_expired" => Self::DisputeExpired,
            "dispute_under_review" => Self::DisputeUnderReview,
            "check" => Self::Check,
            "subscription_billing_skipped" => Self::SubscriptionBillingSkipped,
            "subscription_canceled" => Self::SubscriptionCanceled,
            "subscription_charged_successfully" => Self::SubscriptionChargedSuccessfully,
            "subscription_charged_unsuccessfully" => Self::SubscriptionChargedUnsuccessfully,
            "subscription_expired" => Self::SubscriptionExpired,
            "subscription_trial_ended" => Self::SubscriptionTrialEnded,
            "subscription_went_active" => Self::SubscriptionWentActive,
            "subscription_went_past_due" => Self::SubscriptionWentPastDue,
            "disbursement" => Self::Disbursement,
            "account_updater_daily_report" => Self::AccountUpdaterDailyReport,
            "local_payment_completed" => Self::LocalPaymentCompleted,
            "local_payment_expired" => Self::LocalPaymentExpired,
            "local_payment_funded" => Self::LocalPaymentFunded,
            "local_payment_reversed" => Self::LocalPaymentReversed,
            "payment_method_customer_data_updated" => Self::PaymentMethodCustomerDataUpdated,
            "payment_method_revoked_by_customer" => Self::PaymentMethodRevokedByCustomer,
            "granted_payment_instrument_revoked" => Self::GrantedPaymentInstrumentRevoked,
            "granted_payment_method_revoked" => Self::GrantedPaymentMethodRevoked,
            "grantor_updated_granted_payment_method" => Self::GrantorUpdatedGrantedPaymentMethod,
            "recipient_updated_granted_payment_method" => {
                Self::RecipientUpdatedGrantedPaymentMethod
            }
            "oauth_access_revoked" => Self::OauthAccessRevoked,
            "partner_merchant_connected" => Self::PartnerMerchantConnected,
            "partner_merchant_disconnected" => Self::PartnerMerchantDisconnected,
            "partner_merchant_declined" => Self::PartnerMerchantDeclined,
            "connected_merchant_status_transitioned" => Self::ConnectedMerchantStatusTransitioned,
            "connected_merchant_paypal_status_changed" => {
                Self::ConnectedMerchantPaypalStatusChanged
            }
            _ => Self::Unknown,
        }
    }

    /// True for the two kinds whose `<status>` element describes the ORIGINAL payment.
    ///
    /// `transaction_disbursed`, `transaction_reviewed` and `transaction_retried` also
    /// arrive under a `<transaction>` subject, but their status (when present at all)
    /// belongs to a funding batch, a fraud review or a *different, retried* transaction
    /// — so reading it as the attempt status would move a payment on evidence that is
    /// not about that payment.
    fn carries_payment_status(self) -> bool {
        matches!(
            self,
            Self::TransactionSettled | Self::TransactionSettlementDeclined
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Notification {
    /// Deliberately `String`, not an enum: an unrecognised kind must reach the
    /// `Unknown` arm of `BraintreeWebhookKind::from_wire`, never fail the parse, and the
    /// raw value must survive into `get_webhook_resource_object`.
    pub kind: String,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub timestamp: Option<String>,
    /// Present only when the webhook was delivered to a partner/marketplace parent
    /// account on behalf of a sub-merchant. Not used for routing; captured so the raw
    /// resource object is complete.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub source_merchant_id: Option<String>,
    /// The entity wrapper. Optional so that a kind whose subject UCS does not model
    /// still parses and reaches the unspecified/ignored arm.
    #[serde(default)]
    pub subject: Option<NotificationSubject>,
}

impl Notification {
    pub(super) fn event_kind(&self) -> BraintreeWebhookKind {
        BraintreeWebhookKind::from_wire(&self.kind)
    }

    pub(super) fn transaction(&self) -> Option<&BraintreeWebhookTransaction> {
        self.subject.as_ref()?.transaction.as_ref()
    }

    pub(super) fn dispute(&self) -> Option<&BraintreeDisputeData> {
        self.subject.as_ref()?.dispute.as_ref()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NotificationSubject {
    #[serde(default)]
    pub dispute: Option<BraintreeDisputeData>,
    /// Carries BOTH the payment-lifecycle entity (`transaction_settled`,
    /// `transaction_settlement_declined`) and the refund entity (`refund_failed`).
    /// Braintree uses one `<transaction>` element for both, so the refund reading is a
    /// projection of this struct, not a second wire type — which is what keeps the
    /// refund id and the parent sale id from being swapped.
    #[serde(default)]
    pub transaction: Option<BraintreeWebhookTransaction>,
    /// `transaction_reviewed` only. A different element with a different identifier
    /// field (`transaction-id`, not `id`) and no status, so it cannot be read as a
    /// `<transaction>`. Declared so the payload survives into the raw resource object.
    #[serde(default)]
    pub transaction_review: Option<BraintreeTransactionReview>,
    /// `check` only: `<check type="boolean">true</check>`.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub check: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BraintreeTransactionReview {
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub decision: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reviewer_email: Option<pii::Email>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reviewer_note: Option<Secret<String>>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reviewed_time: Option<String>,
}

/// The `<transaction>` subject entity.
///
/// Only the identifier is treated as required; everything else is optional because the
/// five kinds that use this element carry materially different field sets
/// (`transaction_disbursed` has no `<status>` at all, `refund_failed` has no
/// `<currency-iso-code>`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BraintreeWebhookTransaction {
    /// For a payment kind: the sale's own id. For `refund_failed`: the REFUND's own id
    /// — the parent sale is `refunded_transaction_id`.
    ///
    /// This is Braintree's **legacy** id (`0gtq6gtd`). It is NOT what UCS stored: the
    /// GraphQL Authorize leg writes the **global** id into `connector_transaction_id`.
    /// Read it through `global_transaction_id()` / `global_refund_id()`, never directly,
    /// or the reference resolves against the wrong id space (see `to_braintree_global_id`).
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub id: Option<String>,
    /// The Relay global id, when the payload carries one. Braintree's XML webhook bodies
    /// generally do not — the legacy XML representation emits it, the SDK sample generators
    /// do not — so this is the preferred-but-usually-absent source and
    /// `to_braintree_global_id` derives the value in its absence.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub global_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub status: Option<BraintreeWebhookTransactionStatus>,
    #[serde(rename = "type", default)]
    pub transaction_type: Option<String>,
    /// Decimal MAJOR units on the wire, and the scale is not uniform: the settlement
    /// kinds emit `100.00` while `refund_failed`, `transaction_retried` and
    /// `transaction_disbursed` emit a bare `100`. `StringMajorUnit` parses both through
    /// `Decimal::from_str`; a minor-unit integer type parses neither.
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
    /// discriminator that proves a `transaction_settled` payload is an ACH/SEPA event
    /// and not a card one.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub payment_instrument_type: Option<String>,
    /// `refund_failed` only: the id of the SALE this refund was taken against.
    ///
    /// `refunded-transaction-fk` is a stale spelling emitted by the Python/Node/PHP/Java
    /// *sample generators*; no SDK ever parses it. It is accepted as an alias purely so
    /// a payload produced by one of those generators still parses. Production logic is
    /// never keyed on the `-fk` spelling.
    #[serde(default, alias = "refunded-transaction-fk")]
    pub refunded_transaction_id: Option<String>,
    /// `transaction_retried` only: the transaction this one is a retry OF. Recorded so
    /// the raw resource object is complete; never read as this attempt's identifier.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub retried_transaction_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub processor_response_code: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub processor_response_text: Option<String>,
    /// `Option<String>` rather than a datetime: these are informational, and a
    /// `nil="true"` element must not be able to fail the parse.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub updated_at: Option<String>,
}

/// Braintree transaction status as it appears in webhook XML — the REST spelling,
/// lowercase snake_case.
///
/// Deliberately NOT `BraintreePaymentStatus`: that one is
/// `rename_all = "SCREAMING_SNAKE_CASE"` because it parses the GraphQL response. Feeding
/// webhook XML into it would send every real status into its unknown arm — a silent,
/// total misreading that no test on the GraphQL path could catch. Same value set, different
/// encoding, so the two are bridged by `to_payment_status` below and the
/// `-> AttemptStatus` mapping stays single-sourced.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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
    /// Braintree adds statuses over time. An unrecognised one must not fail the parse
    /// and must not be given a meaning: it maps to no payment status at all, so the
    /// caller keeps whatever status it already had.
    #[serde(other)]
    Unknown,
}

impl BraintreeWebhookTransactionStatus {
    /// Bridge to the GraphQL-side enum so there is exactly ONE
    /// `-> enums::AttemptStatus` mapping for Braintree and the webhook path cannot drift
    /// from the PSync path. `None` means "this status says nothing about the payment".
    fn to_payment_status(self) -> Option<BraintreePaymentStatus> {
        match self {
            Self::Authorized => Some(BraintreePaymentStatus::Authorized),
            Self::Authorizing => Some(BraintreePaymentStatus::Authorizing),
            Self::AuthorizationExpired => Some(BraintreePaymentStatus::AuthorizedExpired),
            Self::Failed => Some(BraintreePaymentStatus::Failed),
            Self::ProcessorDeclined => Some(BraintreePaymentStatus::ProcessorDeclined),
            Self::GatewayRejected => Some(BraintreePaymentStatus::GatewayRejected),
            Self::Voided => Some(BraintreePaymentStatus::Voided),
            Self::Settling => Some(BraintreePaymentStatus::Settling),
            Self::Settled => Some(BraintreePaymentStatus::Settled),
            Self::SettlementPending => Some(BraintreePaymentStatus::SettlementPending),
            Self::SettlementDeclined => Some(BraintreePaymentStatus::SettlementDeclined),
            Self::SettlementConfirmed => Some(BraintreePaymentStatus::SettlementConfirmed),
            Self::SubmittedForSettlement => Some(BraintreePaymentStatus::SubmittedForSettlement),
            Self::Unknown => None,
        }
    }

    /// Refund reading of the same element. Exhaustive, no `_` arm.
    ///
    /// `RefundStatus::Unknown` — not `Pending` — is the honest target for a status that
    /// says nothing about the refund: UCS must not invent a Pending it cannot
    /// substantiate, and a Pending refund is retried forever. The success arm is
    /// unreachable today (Braintree has no refund-success webhook kind) but is written
    /// out so the mapping is already right if one is ever added.
    fn to_refund_status(self) -> enums::RefundStatus {
        match self {
            Self::ProcessorDeclined
            | Self::GatewayRejected
            | Self::Failed
            | Self::SettlementDeclined => enums::RefundStatus::Failure,
            Self::Settled
            | Self::Settling
            | Self::SettlementConfirmed
            | Self::SubmittedForSettlement
            | Self::SettlementPending => enums::RefundStatus::Success,
            Self::Authorized | Self::Authorizing | Self::AuthorizationExpired | Self::Voided => {
                enums::RefundStatus::Unknown
            }
            Self::Unknown => enums::RefundStatus::Unknown,
        }
    }
}

impl BraintreeWebhookTransaction {
    /// This entity's id in the global space, read as a TRANSACTION. Prefers an explicit
    /// `<global-id>` and derives one otherwise.
    pub(super) fn global_transaction_id(&self) -> Option<String> {
        self.resolve_global_id(GLOBAL_ID_PREFIX_TRANSACTION)
    }

    /// This entity's id in the global space, read as a REFUND. On a `refund_failed` payload
    /// `<id>` is the refund's own id, and a refund's global id is prefixed `refund_`, not
    /// `transaction_`.
    pub(super) fn global_refund_id(&self) -> Option<String> {
        self.resolve_global_id(GLOBAL_ID_PREFIX_REFUND)
    }

    /// The PARENT SALE of a refund, in the global space. `<refunded-transaction-id>` is a
    /// legacy transaction id, so it takes the transaction prefix even though it appears on a
    /// refund payload.
    pub(super) fn global_refunded_transaction_id(&self) -> Option<String> {
        self.refunded_transaction_id
            .as_deref()
            .map(|id| to_braintree_global_id(GLOBAL_ID_PREFIX_TRANSACTION, id))
    }

    fn resolve_global_id(&self, prefix: &str) -> Option<String> {
        match self.global_id.as_deref() {
            Some(global_id) => Some(global_id.to_string()),
            None => self
                .id
                .as_deref()
                .map(|id| to_braintree_global_id(prefix, id)),
        }
    }
}

/// The `<dispute>` subject entity.
///
/// Two payload generations exist and the modern one is a strict superset: the legacy
/// payload has `<amount>` but no `<amount-disputed>`, no `<case-number>`, no
/// `<created-at>`, no `<status-history>` and no `<evidence>`. Every one of those is
/// therefore optional, with `amount` as the documented fallback for the disputed amount.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BraintreeDisputeData {
    /// Optional on the struct, required at the point of use: a dispute with no id has
    /// nothing to report, and that is a typed error rather than a parse failure.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub id: Option<String>,
    /// Decimal MAJOR units, e.g. `100.00`.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub amount: Option<StringMajorUnit>,
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
    /// The dispute STAGE — `chargeback` | `pre_arbitration` | `retrieval`. Distinct from
    /// the notification kind, and case-variable on the wire.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub kind: Option<String>,
    /// `open` | `won` | `lost` | `accepted` | `expired` | `disputed` | `under_review`.
    /// Informational only: UCS derives `DisputeStatus` from the notification kind.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reason: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reason_code: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reason_description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub forwarded_comments: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reference_number: Option<String>,
    /// Set when this dispute is a pre-arbitration escalation of an earlier one.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub original_dispute_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub merchant_account_id: Option<String>,
    /// `Option<String>`, not `Option<PrimitiveDateTime>`: Braintree emits these as
    /// `nil="true"` empty elements when unset, which a datetime deserializer cannot
    /// survive. Parse downstream if a typed value is ever needed.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub updated_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub reply_by_date: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub received_date: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub date_opened: Option<String>,
    /// `type="array"` on the wire, with repeated `<evidence>` children.
    #[serde(default)]
    pub evidence: Option<DisputeEvidenceList>,
    /// `type="array"` on the wire. Not consumed by any mapping; declared so it survives
    /// into the raw resource object.
    #[serde(default)]
    pub status_history: Option<DisputeStatusHistoryList>,
    #[serde(default)]
    pub transaction: Option<DisputeTransaction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DisputeTransaction {
    /// The disputed SALE's **legacy** id. Read it through `global_transaction_id()`: the
    /// payment-lookup key has to be in the global id space UCS stored.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub global_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub amount: Option<StringMajorUnit>,
    /// The merchant-assigned reference from the original Authorize.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub order_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub purchase_order_number: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub payment_instrument_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub payment_instrument_subtype: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub merchant_account_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub created_at: Option<String>,
}

impl DisputeTransaction {
    /// The disputed sale in the global id space — the key the caller resolves the payment by.
    pub(super) fn global_transaction_id(&self) -> Option<String> {
        match self.global_id.as_deref() {
            Some(global_id) => Some(global_id.to_string()),
            None => self
                .id
                .as_deref()
                .map(|id| to_braintree_global_id(GLOBAL_ID_PREFIX_TRANSACTION, id)),
        }
    }
}

/// The `<evidence type="array">` wrapper. A bare `Option<DisputeEvidence>` keeps only
/// the LAST child when Braintree sends several, which `dispute_lost` routinely does.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DisputeEvidenceList {
    #[serde(default)]
    pub evidence: Vec<DisputeEvidence>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DisputeEvidence {
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub id: Option<Secret<String>>,
    /// The element is `comments`, plural. The singular `comment` never appears on the
    /// wire, and it is `nil="true"` on any evidence item that is a file rather than text.
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub comments: Option<Secret<String>>,
    /// `Option<String>`, NOT `url::Url`: Braintree emits values such as
    /// `s3.amazonaws.com/foo.jpg`, which is not an absolute URL, and emits the element
    /// as `nil="true"` on text evidence. A `url::Url` target rejects both and fails the
    /// whole notification.
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

/// The `<status-history type="array">` wrapper. Declared, unconsumed.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DisputeStatusHistoryList {
    #[serde(default)]
    pub status_history: Vec<DisputeStatusHistoryEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DisputeStatusHistoryEntry {
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub timestamp: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub disbursement_date: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nil_aware")]
    pub effective_date: Option<String>,
}

/// Maps the Braintree notification `kind` to the prism webhook event type.
///
/// The fan-out in `webhook_utils::process_webhook_event` selects the payment / refund /
/// dispute handler purely from `EventType::is_payment_event()` / `is_refund_event()` /
/// `is_dispute_event()`, and anything matching none of the three — including
/// `IncomingWebhookEventUnspecified`, which is a misc event — falls through to the
/// PAYMENT handler. So an unmodelled kind must be `IncomingWebhookEventUnspecified` AND
/// `process_payment_webhook` must be safe for a subject it cannot read; both halves are
/// required, and neither may be a success or a payment-finalising failure.
pub(super) fn get_status(kind: &str) -> connector_types::EventType {
    match BraintreeWebhookKind::from_wire(kind) {
        // --- dispute family ---------------------------------------------------
        BraintreeWebhookKind::DisputeOpened => connector_types::EventType::DisputeOpened,
        BraintreeWebhookKind::DisputeAccepted | BraintreeWebhookKind::DisputeAutoAccepted => {
            connector_types::EventType::DisputeAccepted
        }
        // `dispute_under_review` is the non-terminal state between `dispute_disputed`
        // and won/lost. UCS has no "under review" variant, and the conservative-looking
        // alternative (Unspecified) is a misc event that would route a DISPUTE payload
        // to the payment handler. `DisputeChallenged` is coarse but correctly classed.
        BraintreeWebhookKind::DisputeDisputed | BraintreeWebhookKind::DisputeUnderReview => {
            connector_types::EventType::DisputeChallenged
        }
        BraintreeWebhookKind::DisputeExpired => connector_types::EventType::DisputeExpired,
        BraintreeWebhookKind::DisputeWon => connector_types::EventType::DisputeWon,
        BraintreeWebhookKind::DisputeLost => connector_types::EventType::DisputeLost,

        // --- transaction family: only the two settlement kinds move a payment ---
        // Braintree documents these as ACH / SEPA Direct Debit only; a card payment
        // produces no payment-lifecycle webhook at all.
        BraintreeWebhookKind::TransactionSettled => {
            connector_types::EventType::PaymentIntentSuccess
        }
        BraintreeWebhookKind::TransactionSettlementDeclined => {
            connector_types::EventType::PaymentIntentFailure
        }

        // --- refund family: the only refund kind Braintree has ------------------
        // There is no `refund_settled` and no `refund_succeeded`; a successful refund is
        // observable only through RSync.
        BraintreeWebhookKind::RefundFailed => connector_types::EventType::RefundFailure,

        // --- connectivity -------------------------------------------------------
        // The Control Panel's "Check URL" POST. It has a dedicated variant, so use it
        // rather than letting a boolean-only subject fall through to the payment handler.
        BraintreeWebhookKind::Check => connector_types::EventType::EndpointVerification,

        // --- everything else ----------------------------------------------------
        // Funding batches, fraud reviews, retries of a different transaction,
        // subscriptions, Local Payment Methods, vault/grant, OAuth, partner-merchant
        // onboarding, the Account Updater report — and every kind added after this was
        // written. None of them carries a payment, refund or dispute state change UCS
        // can substantiate, so none of them may produce a status.
        BraintreeWebhookKind::TransactionDisbursed
        | BraintreeWebhookKind::TransactionReviewed
        | BraintreeWebhookKind::TransactionRetried
        | BraintreeWebhookKind::SubscriptionBillingSkipped
        | BraintreeWebhookKind::SubscriptionCanceled
        | BraintreeWebhookKind::SubscriptionChargedSuccessfully
        | BraintreeWebhookKind::SubscriptionChargedUnsuccessfully
        | BraintreeWebhookKind::SubscriptionExpired
        | BraintreeWebhookKind::SubscriptionTrialEnded
        | BraintreeWebhookKind::SubscriptionWentActive
        | BraintreeWebhookKind::SubscriptionWentPastDue
        | BraintreeWebhookKind::Disbursement
        | BraintreeWebhookKind::AccountUpdaterDailyReport
        | BraintreeWebhookKind::LocalPaymentCompleted
        | BraintreeWebhookKind::LocalPaymentExpired
        | BraintreeWebhookKind::LocalPaymentFunded
        | BraintreeWebhookKind::LocalPaymentReversed
        | BraintreeWebhookKind::PaymentMethodCustomerDataUpdated
        | BraintreeWebhookKind::PaymentMethodRevokedByCustomer
        | BraintreeWebhookKind::GrantedPaymentInstrumentRevoked
        | BraintreeWebhookKind::GrantedPaymentMethodRevoked
        | BraintreeWebhookKind::GrantorUpdatedGrantedPaymentMethod
        | BraintreeWebhookKind::RecipientUpdatedGrantedPaymentMethod
        | BraintreeWebhookKind::OauthAccessRevoked
        | BraintreeWebhookKind::PartnerMerchantConnected
        | BraintreeWebhookKind::PartnerMerchantDisconnected
        | BraintreeWebhookKind::PartnerMerchantDeclined
        | BraintreeWebhookKind::ConnectedMerchantStatusTransitioned
        | BraintreeWebhookKind::ConnectedMerchantPaypalStatusChanged
        | BraintreeWebhookKind::Unknown => {
            connector_types::EventType::IncomingWebhookEventUnspecified
        }
    }
}

/// Maps the Braintree notification `kind` to the prism dispute status.
///
/// Returns `Option` rather than defaulting: `DisputeStatus` has no unknown variant, so a
/// total function would have to manufacture a concrete dispute state for a kind that is
/// not a dispute at all. The caller fails closed instead. Unreachable in practice —
/// `process_dispute_webhook` is entered only after `get_status` returned a dispute event
/// — but the two functions must not be coupled by an invariant no type enforces.
/// `DisputeCancelled` has no Braintree kind and is never produced.
pub(super) fn get_dispute_status(kind: BraintreeWebhookKind) -> Option<enums::DisputeStatus> {
    match kind {
        BraintreeWebhookKind::DisputeOpened => Some(enums::DisputeStatus::DisputeOpened),
        BraintreeWebhookKind::DisputeAccepted | BraintreeWebhookKind::DisputeAutoAccepted => {
            Some(enums::DisputeStatus::DisputeAccepted)
        }
        BraintreeWebhookKind::DisputeDisputed | BraintreeWebhookKind::DisputeUnderReview => {
            Some(enums::DisputeStatus::DisputeChallenged)
        }
        BraintreeWebhookKind::DisputeExpired => Some(enums::DisputeStatus::DisputeExpired),
        BraintreeWebhookKind::DisputeWon => Some(enums::DisputeStatus::DisputeWon),
        BraintreeWebhookKind::DisputeLost => Some(enums::DisputeStatus::DisputeLost),
        _ => None,
    }
}

/// Maps the dispute's own `<kind>` — the STAGE, distinct from the notification kind — to
/// the prism dispute stage.
///
/// Case-insensitive: Braintree's legacy sample emits `CHARGEBACK` and the modern one
/// emits `chargeback`, and both are valid on the wire. Returns a value rather than a
/// `Result`: an unrecognised stage must not discard a dispute notification that has a
/// reply-by deadline attached.
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
// newline-stripped `bt_payload`. Newline stripping is a DECODE-path transformation only;
// the signature is computed over the un-stripped value (see `verify_webhook_source`).
pub(super) fn decode_from_request(
    request: &connector_types::RequestDetails,
) -> Result<Notification, Report<domain_types::errors::WebhookError>> {
    let notif = get_webhook_object_from_body(&request.body)?;
    decode_webhook_payload(notif.bt_payload.replace('\n', "").as_bytes())
}

/// Builds the typed webhook resource reference emitted during the stateless ParseEvent
/// phase.
///
/// `Ok(None)` — not `Err` — is the answer for any kind UCS does not model. ParseEvent runs
/// with no secrets and no merchant context, and a subscription or disbursement webhook is
/// something UCS declines to act on, not a malformed request.
/// `WebhookReferenceIdNotFound` is reserved for a payload whose kind IS modelled but whose
/// identifying subject is genuinely missing.
pub(super) fn get_webhook_reference(
    notification: &Notification,
) -> Result<
    Option<connector_types::WebhookResourceReference>,
    Report<domain_types::errors::WebhookError>,
> {
    let missing_subject =
        || error_stack::report!(domain_types::errors::WebhookError::WebhookReferenceIdNotFound);

    match notification.event_kind() {
        BraintreeWebhookKind::DisputeOpened
        | BraintreeWebhookKind::DisputeWon
        | BraintreeWebhookKind::DisputeLost
        | BraintreeWebhookKind::DisputeAccepted
        | BraintreeWebhookKind::DisputeAutoAccepted
        | BraintreeWebhookKind::DisputeDisputed
        | BraintreeWebhookKind::DisputeExpired
        | BraintreeWebhookKind::DisputeUnderReview => {
            let dispute = notification.dispute().ok_or_else(missing_subject)?;
            Ok(Some(connector_types::WebhookResourceReference::Dispute(
                connector_types::DisputeWebhookReference {
                    // The shadow normaliser resolves a prism Dispute reference via
                    // `connector_dispute_id.or(connector_transaction_id)`, preferring
                    // connector_dispute_id, so it stays `None` here and the disputed sale is
                    // what the caller resolves by. The dispute's own id is still reported, on
                    // `DisputeWebhookDetailsResponse::dispute_id`.
                    connector_dispute_id: None,
                    // DELIBERATE DIVERGENCE from byte-for-byte HS parity. HS emits
                    // `PaymentId(ConnectorTransactionId(transaction.id))` — the LEGACY id —
                    // and this line used to copy that. It cannot resolve anything: UCS's
                    // GraphQL Authorize stored the GLOBAL id. The parity that was being
                    // preserved is parity with a code path that never reaches this point
                    // anyway, because HS's own Direct Braintree dispute webhook has the same
                    // structural defects fixed here (no `<subject>` level, no kebab-case) and
                    // fails to deserialize a real payload before it ever builds a reference.
                    // Matching the id space UCS actually wrote beats matching a value nothing
                    // produces.
                    connector_transaction_id: dispute
                        .transaction
                        .as_ref()
                        .and_then(DisputeTransaction::global_transaction_id),
                },
            )))
        }
        BraintreeWebhookKind::RefundFailed => {
            let transaction = notification.transaction().ok_or_else(missing_subject)?;
            Ok(Some(connector_types::WebhookResourceReference::Refund(
                connector_types::RefundWebhookReference {
                    // `<id>` on a refund_failed payload is the REFUND. The parent sale is
                    // `<refunded-transaction-id>`. Swapping these makes every refund
                    // webhook update the wrong row. Both are emitted in the global id space,
                    // with the prefix each entity actually carries.
                    connector_refund_id: transaction.global_refund_id(),
                    merchant_refund_id: None,
                    connector_transaction_id: transaction.global_refunded_transaction_id(),
                    merchant_transaction_id: transaction.order_id.clone(),
                },
            )))
        }
        BraintreeWebhookKind::TransactionSettled
        | BraintreeWebhookKind::TransactionSettlementDeclined => {
            let transaction = notification.transaction().ok_or_else(missing_subject)?;
            Ok(Some(connector_types::WebhookResourceReference::Payment(
                connector_types::PaymentWebhookReference {
                    connector_transaction_id: transaction.global_transaction_id(),
                    merchant_transaction_id: transaction.order_id.clone(),
                },
            )))
        }
        // `check` has no resource to reference, and every unmodelled kind is one UCS
        // declines to act on. Both are `Ok(None)`.
        _ => Ok(None),
    }
}

/// Resolves the disputed amount to the connector's configured webhook unit.
///
/// `amount-disputed` is absent on Braintree's legacy dispute payload, where `<amount>` is
/// the only money element — hence the fallback. Both are decimal major-unit strings, so
/// they are parsed as `StringMajorUnit` and converted back to minor units before going
/// out through the webhook converter. `DisputeWebhookDetailsResponse::amount` is not an
/// `Option`, and a zero disputed amount is a materially wrong fact on a merchant's
/// ledger, so a payload with neither element is a typed error, never a substituted `0`.
fn resolve_dispute_amount(
    dispute: &BraintreeDisputeData,
    currency: enums::Currency,
) -> Result<common_utils::types::StringMinorUnit, Report<domain_types::errors::WebhookError>> {
    let amount_major = dispute
        .amount_disputed
        .clone()
        .or_else(|| dispute.amount.clone())
        .ok_or_else(|| {
            error_stack::report!(
                domain_types::errors::WebhookError::WebhookMissingRequiredField {
                    field: "amount-disputed",
                }
            )
        })?;

    let minor_amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
        &StringMajorUnitForConnector,
        amount_major,
        currency,
    )?;

    domain_types::utils::convert_amount_for_webhook(
        &common_utils::types::StringMinorUnitForConnector,
        minor_amount,
        currency,
    )
}

// Builds the dispute webhook response, including the webhook amount conversion.
pub(super) fn build_webhook_dispute_response(
    notification: &Notification,
    raw_body: &[u8],
) -> Result<
    connector_types::DisputeWebhookDetailsResponse,
    Report<domain_types::errors::WebhookError>,
> {
    let dispute = notification.dispute().ok_or_else(|| {
        error_stack::report!(domain_types::errors::WebhookError::WebhookResourceObjectNotFound)
    })?;

    let status = get_dispute_status(notification.event_kind()).ok_or_else(|| {
        error_stack::report!(
            domain_types::errors::WebhookError::WebhookMissingRequiredField { field: "kind" }
        )
        .attach_printable("Braintree dispute handler reached with a non-dispute notification kind")
    })?;

    let currency = dispute.currency_iso_code.ok_or_else(|| {
        error_stack::report!(
            domain_types::errors::WebhookError::WebhookMissingRequiredField {
                field: "currency-iso-code",
            }
        )
    })?;

    Ok(connector_types::DisputeWebhookDetailsResponse {
        amount: resolve_dispute_amount(dispute, currency)?,
        currency,
        // Reported in Braintree's own form, NOT translated to a global id. Unlike a
        // transaction or refund id, nothing resolves against this one — UCS never created the
        // dispute and stored no id for it — and the `dispute_` global prefix is not attested
        // by any captured sandbox value. Translating an id on an unverified prefix, purely for
        // symmetry with ids that genuinely need it, would be a guess printed into the
        // merchant's dispute record.
        dispute_id: dispute.id.clone().ok_or_else(|| {
            error_stack::report!(
                domain_types::errors::WebhookError::WebhookMissingRequiredField { field: "id" }
            )
        })?,
        status,
        stage: get_dispute_stage(dispute.kind.as_deref()),
        connector_response_reference_id: dispute
            .transaction
            .as_ref()
            .and_then(|transaction| transaction.order_id.clone()),
        dispute_message: dispute.reason.clone(),
        connector_reason_code: dispute.reason_code.clone(),
        // A dispute payload carries no card or bank details, so the raw envelope is safe
        // to surface here — unlike the transaction family (see `build_webhook_*_response`).
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}

/// Builds the payment webhook response.
///
/// Reached for the two settlement kinds AND, via the misc-event fall-through described on
/// `get_status`, for every kind UCS does not model — `check` included. That is why the
/// transaction subject is optional here and why the status is gated on the kind rather
/// than read off whatever `<status>` happens to be present: `transaction_retried` carries
/// `submitted_for_settlement`, but it describes the RETRY, not the attempt UCS is holding.
/// Anything not gated in resolves to `AttemptStatus::Unspecified`, which leaves the
/// caller's existing status untouched.
pub(super) fn build_webhook_payment_response(
    notification: &Notification,
) -> Result<connector_types::WebhookDetailsResponse, Report<domain_types::errors::WebhookError>> {
    let kind = notification.event_kind();
    let transaction = notification.transaction();

    if kind.carries_payment_status() {
        // Braintree documents transaction webhooks as ACH / SEPA Direct Debit only, and
        // both SDK samples hard-code `us_bank_account`. Do not reject a card one — a
        // webhook must not be dropped on a shape assumption — but make the anomaly
        // visible if the family is ever extended to cards.
        if let Some(instrument) = transaction.and_then(|t| t.payment_instrument_type.as_deref()) {
            if instrument != "us_bank_account" {
                tracing::warn!(
                    target: "braintree_webhook",
                    payment_instrument_type = instrument,
                    kind = %notification.kind,
                    "Braintree transaction webhook on a non-bank instrument"
                );
            }
        }
    }

    let status = if kind.carries_payment_status() {
        transaction
            .and_then(|transaction| transaction.status)
            .and_then(BraintreeWebhookTransactionStatus::to_payment_status)
            .map(enums::AttemptStatus::from)
            .unwrap_or(enums::AttemptStatus::Unspecified)
    } else {
        enums::AttemptStatus::Unspecified
    };

    let is_declined = kind == BraintreeWebhookKind::TransactionSettlementDeclined;

    Ok(connector_types::WebhookDetailsResponse {
        resource_id: transaction
            .and_then(BraintreeWebhookTransaction::global_transaction_id)
            .map(ResponseId::ConnectorTransactionId),
        status,
        connector_response_reference_id: transaction
            .and_then(|transaction| transaction.order_id.clone()),
        // Braintree echoes no separate request reference on a webhook.
        connector_request_reference_id: None,
        // No Braintree webhook kind reports a mandate.
        mandate_reference: None,
        error_code: is_declined
            .then(|| transaction.and_then(|t| t.processor_response_code.clone()))
            .flatten(),
        error_message: is_declined
            .then(|| transaction.and_then(|t| t.processor_response_text.clone()))
            .flatten(),
        error_reason: None,
        // Deliberately NOT the raw envelope. The `<transaction>` subject carries a full
        // `<credit-card><number>` on `refund_failed` and `<us-bank-account>` routing /
        // account-holder details on `transaction_settled`, and the payload reaches UCS as
        // base64 inside a form body — which defeats the response masker entirely, since it
        // has no key to gate on. The parsed resource object, which models neither block,
        // is available through `get_webhook_resource_object`.
        raw_connector_response: None,
        status_code: 200,
        response_headers: None,
        // A settlement is not a capture. Equating the two is an inference to make in the
        // caller from PSync, which reports the captured amount directly, not here.
        amount_captured: None,
        minor_amount_captured: None,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

/// Builds the refund webhook response.
///
/// `refund_failed` is the only kind that reaches this handler; there is no
/// `refund_settled` and no `refund_succeeded`, so a successful refund is observable only
/// through RSync.
pub(super) fn build_webhook_refund_response(
    notification: &Notification,
) -> Result<connector_types::RefundWebhookDetailsResponse, Report<domain_types::errors::WebhookError>>
{
    // Fail closed rather than reading a refund status off a kind that is not a refund.
    // `transaction_retried` carries `<status>submitted_for_settlement</status>` under the same
    // `<transaction>` element, and reporting that as a refund SUCCESS would close out a refund
    // that never happened. Unreachable through the normal fan-out, which routes here only on
    // `EventType::RefundFailure` — but the two must not be coupled by an untyped invariant.
    if notification.event_kind() != BraintreeWebhookKind::RefundFailed {
        Err(error_stack::report!(
            domain_types::errors::WebhookError::WebhookMissingRequiredField { field: "kind" }
        )
        .attach_printable("Braintree refund handler reached with a non-refund notification kind"))?
    }

    let transaction = notification.transaction().ok_or_else(|| {
        error_stack::report!(domain_types::errors::WebhookError::WebhookResourceObjectNotFound)
    })?;

    Ok(connector_types::RefundWebhookDetailsResponse {
        // `<id>` IS the refund. `<refunded-transaction-id>` is the parent sale and goes
        // in `connector_response_reference_id` below. Both in the global id space, so they
        // match what RSync stored.
        connector_refund_id: transaction.global_refund_id(),
        merchant_transaction_id: transaction.order_id.clone(),
        status: transaction
            .status
            .map(BraintreeWebhookTransactionStatus::to_refund_status)
            .unwrap_or(enums::RefundStatus::Unknown),
        connector_response_reference_id: transaction.global_refunded_transaction_id(),
        error_code: transaction.processor_response_code.clone(),
        error_message: transaction.processor_response_text.clone(),
        // A `refund_failed` subject carries `<credit-card><number>` — a full PAN. See the
        // note on `build_webhook_payment_response`.
        raw_connector_response: None,
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
        // WHICH VAULT HOLDS THE CREDENTIAL is what selects the request shape, and the answer is
        // the *variant* of `MandateReferenceId`, not the payment method:
        //
        //   ConnectorMandateId  -> Braintree holds it. Charge its multi-use token; send no
        //                          external-vault object (the SDL forbids it for Braintree
        //                          multi-use payment methods).
        //   NetworkMandateId    -> we hold only a scheme NTID.
        //   NetworkTokenWithNTI -> we hold a network token plus a scheme NTID.
        //
        // The latter two are the same regime as far as Braintree is concerned — an externally
        // vaulted credential, replayed with `options.externalVault.verifyingNetworkTransactionId`
        // — and differ only in where the credential itself came from. Both still need a
        // Braintree `paymentMethodId`: it is NON-NULL on `ChargeCreditCardInput` /
        // `AuthorizeCreditCardInput`, and Braintree accepts no raw PAN on any transaction
        // mutation, so the credential must already have been exchanged for a *single-use* token
        // by `PaymentMethodService/Tokenize` — the same ingress Authorize uses. That is why
        // `CardDetailsForNetworkTransactionId` (a raw PAN) stays unsupported: there is no
        // mutation that would take it.
        let credential = match item.router_data.request.get_mandate_reference() {
            connector_types::MandateReferenceId::ConnectorMandateId(_) => {
                // Preserve the existing contract for this regime: the Braintree vault token is
                // the entire credential, so the request must not also carry a payment method
                // that would be silently ignored.
                if !matches!(
                    item.router_data.request.payment_method_data,
                    PaymentMethodData::MandatePayment
                ) {
                    return Err(error_stack::report!(IntegrationError::NotSupported {
                        message: utils::get_unimplemented_payment_method_error_message("braintree"),
                        connector: "Braintree",
                        context: Default::default(),
                    }));
                }
                let connector_mandate_id = item.router_data.request.connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    },
                )?;
                BraintreeMandateCredential::BraintreeVaulted {
                    payment_method_id: Secret::new(connector_mandate_id),
                }
            }
            connector_types::MandateReferenceId::NetworkMandateId(network_mandate) => {
                BraintreeMandateCredential::ExternallyVaulted {
                    payment_method_id: braintree_single_use_token(
                        &item.router_data.request.payment_method_data,
                        BraintreeTokenConsumer::RepeatPayment,
                    )?,
                    network_transaction_id: Secret::new(
                        network_mandate.network_transaction_id.clone(),
                    ),
                }
            }
            connector_types::MandateReferenceId::NetworkTokenWithNTI(network_token) => {
                BraintreeMandateCredential::ExternallyVaulted {
                    payment_method_id: braintree_single_use_token(
                        &item.router_data.request.payment_method_data,
                        BraintreeTokenConsumer::RepeatPayment,
                    )?,
                    network_transaction_id: Secret::new(
                        network_token.network_transaction_id.clone(),
                    ),
                }
            }
        };
        Ok(Self::Mandate(MandatePaymentRequest::try_from((
            item, credential, metadata,
        ))?))
    }
}

/// The Braintree single-use token an externally-vaulted merchant-initiated charge must spend as
/// `paymentMethodId`.
///
/// Fails closed. Braintree's card mutations declare `paymentMethodId` non-null and accept no raw
/// card data, so there is no fallback to degrade to: without a token the charge cannot be built
/// at all, and quietly dropping the external-vault assertion instead would produce a transaction
/// that settles but breaks the stored-credential chain — the one failure mode the gateway gives
/// no signal for.
///
/// A token tagged Apple Pay or Google Pay is NOT a card token: those wallets are served by
/// `chargePaymentMethod` / `authorizePaymentMethod`, which have no `options` field and therefore
/// cannot carry `externalVault` at all, so they are rejected here rather than silently charged
/// as a credit card.
fn braintree_single_use_token<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
    consumer: BraintreeTokenConsumer,
) -> Result<Secret<String>, Report<IntegrationError>> {
    match payment_method_data {
        PaymentMethodData::PaymentMethodToken(token_data)
            if token_data.token_payment_method_type.is_none() =>
        {
            Ok(token_data.token.clone())
        }
        _ => Err(error_stack::report!(
            IntegrationError::MissingRequiredField {
                field_name: "payment_method.token",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(consumer.suggested_action().to_string()),
                    doc_url: Some(consumer.doc_url().to_string()),
                    additional_context: Some(consumer.additional_context().to_string()),
                },
            }
        )),
    }
}

/// Which flow is asking for the single-use token.
///
/// The extraction is identical for both, but the remediation a caller needs is not — a repeat
/// payment must re-tokenize a stored credential, while a mandate setup must tokenize the card
/// the cardholder just entered — so the shared helper is parameterised rather than copied.
#[derive(Debug, Clone, Copy)]
enum BraintreeTokenConsumer {
    /// `RecurringPaymentService/Charge` replaying an externally vaulted credential.
    RepeatPayment,
    /// `PaymentService/SetupRecurring` — the zero-amount verification that creates the mandate.
    SetupMandate,
}

impl BraintreeTokenConsumer {
    fn suggested_action(self) -> &'static str {
        match self {
            Self::RepeatPayment => {
                "Exchange the stored credential for a Braintree single-use token with \
                 PaymentMethodService/Tokenize and send it as payment_method.token on the \
                 repeat payment. Braintree requires a non-null paymentMethodId on \
                 chargeCreditCard / authorizeCreditCard and accepts no raw card data on any \
                 transaction mutation."
            }
            Self::SetupMandate => {
                "Tokenize the card with PaymentMethodService/Tokenize first, then call \
                 PaymentService/TokenSetupRecurring with the token as connector_token. \
                 Braintree's vaultCreditCard takes a paymentMethodId and accepts no raw card \
                 data, and the two mutations cannot be combined into one request because \
                 GraphQL root fields cannot consume each other's output."
            }
        }
    }

    fn doc_url(self) -> &'static str {
        match self {
            Self::RepeatPayment => {
                "https://graphql.braintreepayments.com/reference/#Input--ChargeCreditCardInput"
            }
            Self::SetupMandate => {
                "https://graphql.braintreepayments.com/reference/#Input--VaultCreditCardInput"
            }
        }
    }

    fn additional_context(self) -> &'static str {
        match self {
            Self::RepeatPayment => {
                "network-transaction-id mandate replayed without a Braintree card token"
            }
            Self::SetupMandate => {
                "zero-amount card verification requested without a Braintree card token"
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
    /// `CreditCardTransactionOptionsInput.externalVault`. Populated ONLY on an
    /// externally-vaulted merchant-initiated charge — see `TransactionExternalVaultOptions`.
    ///
    /// This is also why the merchant-initiated path must stay on
    /// `chargeCreditCard` / `authorizeCreditCard`: `ChargePaymentMethodInput` and
    /// `AuthorizePaymentMethodInput` have four fields each and no `options` at all, so
    /// `externalVault` is structurally unreachable through them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_vault: Option<TransactionExternalVaultOptions>,
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
                // Read before the terminal branch moves `transaction_data`: the MIT mints its
                // own NTID, and a caller chaining MIT-on-MIT needs the latest one.
                let network_txn_id = transaction_data.network_transaction_id();
                let network_txn_link_id = transaction_data.network_transaction_link_id();
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
                        network_txn_id,
                        network_txn_link_id,
                        // The MIT echoes the same `transaction.orderId` Authorize sends, so both
                        // flows report one reference id for the same series (checklist Theme 11).
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
// A REAL zero-amount card verification, not a tokenization relabelled.
//
// What this used to be, and why it was a bug: SetupMandate called
// `tokenizeCreditCard` and reported `AttemptStatus::Charged`. `tokenizeCreditCard`
// stores card data in Braintree's vault and never contacts the card network, so
// nothing was verified, there was no AVS or CVV verdict to report, and a dead card
// came back indistinguishable from a live one. A merchant relying on a $0 auth to
// validate a card before vaulting got a false positive every time.
//
// What it is now: `vaultCreditCard`, which verifies against the network AND vaults
// in a single mutation (tech spec §SM.2/§SM.4, live-SDL verified). The attempt
// status is mapped from `verification.status` — never hardcoded — the AVS/CVV
// verdicts land on `PaymentFlowData.connector_response` through the same builder
// Authorize uses, and the network transaction id is captured off the same
// `paymentMethodSnapshot` selection Authorize splices.
//
// Braintree accepts no raw PAN on `vaultCreditCard`; it takes a `paymentMethodId`.
// The card must therefore be tokenized first, and the two mutations CANNOT be
// spliced into one document the way `PRE_AUTHENTICATE_MUTATION` splices its pair —
// GraphQL executes root mutation fields serially but gives no way for one to consume
// another's output, and `vaultCreditCard` needs the token `tokenizeCreditCard`
// returns. So SetupMandate takes the token as input, exactly as Authorize does, and
// fails closed when it is absent.

/// `VaultCreditCardVerificationOptionsInput` — the verification half of `VaultCreditCardInput`.
///
/// Carries ONLY the merchant account. Two omissions are deliberate and load-bearing:
///
/// * **`skip` is never sent.** Verification is on by default (confirmed live: a `vaultCreditCard`
///   with no `verification` key at all still returned `status: VERIFIED`), and `skip: true`
///   returns `verification: null` while still vaulting — i.e. exactly the unverified vault this
///   change exists to remove. The field is not modelled, so it cannot be set by accident.
/// * **`amount` is never sent.** Braintree then chooses the verification amount itself, which is
///   what tech spec §SM.7 resolves the connector's long-standing UNDECIDED #4 in favour of.
///   Forcing `"0.00"` risks processor code `2031` ("Bank doesn't support $0.00 verifications")
///   on issuers that reject zero-amount authorizations — the very escalation Braintree's own
///   logic exists to perform — and forcing `"1.00"` puts a real (auto-voided) hold on the
///   cardholder. Every live call on this sandbox returned `0.00`, but the documented escalation
///   to $1 was NOT reproducible here and is marked UNVERIFIED in §SM.15, so the amount is read
///   back off the response instead of being assumed.
/// * `fraudTools` is not modelled: the SDL exposes it here, yet its own field descriptions say
///   `skipCvv`/`skipAvs` apply only to `chargeCreditCard`/`authorizeCreditCard`. That
///   contradiction is unresolved upstream (§SM.15 item 7), and skipping AVS/CVV would defeat the
///   point of this flow.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCreditCardVerificationOptionsInput {
    /// The merchant account the verification is performed against. On `vaultCreditCard` this
    /// lives NESTED under `verification` — `verifyPaymentMethod` takes it at the top level and
    /// `verifyCreditCard` cannot target one at all, so the placement is not interchangeable.
    merchant_account_id: Secret<String>,
}

/// Braintree GraphQL `VaultCreditCardInput`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCreditCardInput {
    /// The single-use token from `tokenizeCreditCard`. Consumed by this call: replaying it
    /// returns `legacyCode 93107`, "Cannot use a single-use payment method more than once".
    payment_method_id: Secret<String>,
    /// Echoed back verbatim by Braintree; carries the UCS request reference so a merchant can
    /// line the verification up with the attempt that produced it. Capped at 255 characters by
    /// the SDL.
    #[serde(skip_serializing_if = "Option::is_none")]
    client_mutation_id: Option<String>,
    verification: VaultCreditCardVerificationOptionsInput,
    /// `VaultCreditCardInput.billingAddress` is a TOP-LEVEL sibling of `verification` here —
    /// unlike Authorize, where the billing address hangs off `options.billingAddress`. Sent
    /// whenever UCS holds one, because without it the processor answers `NOT_PROVIDED` for both
    /// AVS checks and the verification tells the merchant nothing about the address.
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<BraintreeAddressInput>,
}

pub type BraintreeSetupMandateRequest =
    GenericBraintreeRequest<GenericVariableInput<VaultCreditCardInput>>;

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
    > for BraintreeSetupMandateRequest
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
        // Fails closed rather than falling back to a default merchant account. The verification
        // must run on the SAME merchant account the later merchant-initiated charge will run on,
        // or the stored-credential chain it bootstraps is attached to the wrong account; and a
        // bad value here is answered by Braintree with `legacyCode 91728`, "Verification merchant
        // account ID is invalid", which is far harder to diagnose after the fact than a local
        // configuration error. `SetupMandateRequestData` carries no per-request merchant account
        // override, so unlike Authorize and Refund there is only the connector config to read.
        let merchant_account_id = BraintreeAuthType::try_from(&item.router_data.connector_config)?
            .merchant_account_id
            .ok_or_else(|| IntegrationError::InvalidConnectorConfig {
                config: "merchant_account_id",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Set merchant_account_id in the Braintree connector configuration. \
                         Braintree performs the zero-amount verification against a specific \
                         merchant account and the mandate it creates is only usable from that \
                         same account."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://graphql.braintreepayments.com/reference/#Input--VaultCreditCardVerificationOptionsInput"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "zero-amount card verification requested with no Braintree merchant \
                         account configured"
                            .to_string(),
                    ),
                },
            })?;

        Ok(Self {
            query: constants::VAULT_CREDIT_CARD_MUTATION.to_string(),
            variables: GenericVariableInput {
                input: VaultCreditCardInput {
                    // Shared with RepeatPayment: the same extraction, the same fail-closed
                    // `MissingRequiredField`, flow-specific remediation text.
                    payment_method_id: braintree_single_use_token(
                        &item.router_data.request.payment_method_data,
                        BraintreeTokenConsumer::SetupMandate,
                    )?,
                    client_mutation_id: Some(
                        item.router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    ),
                    verification: VaultCreditCardVerificationOptionsInput {
                        merchant_account_id,
                    },
                    billing_address: build_billing_address(&item.router_data.resource_common_data),
                },
            },
        })
    }
}

/// Braintree GraphQL `VerificationStatus` — SIX values, not four.
///
/// `PENDING` and `VERIFYING` are non-terminal and are routinely missed: a four-value enum built
/// from the older documentation fails to deserialize them outright, and any "every verification
/// is terminal" assumption built on top is wrong. `Unknown` catches a seventh value Braintree may
/// add, so a new upstream status degrades to "not yet resolved" instead of failing the response.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreeVerificationStatus {
    /// "Indicates that the verification was successful."
    Verified,
    /// "Indicates that the verification was unsuccessful based on the response from the
    /// processor." An explicit decline.
    ProcessorDeclined,
    /// "Indicates that the verification was unsuccessful because the payment method failed one or
    /// more fraud checks." An explicit gateway decline; `gatewayRejectionReason` says which rule.
    GatewayRejected,
    /// "Indicates the verification was unsuccessful because of an issue communicating with the
    /// processor." NOT a decline — a transport failure between Braintree and the processor.
    Failed,
    /// "Indicates that the verification is pending."
    Pending,
    /// "Indicates that the verification is in the process of verifying."
    Verifying,
    #[serde(other)]
    Unknown,
}

impl From<BraintreeVerificationStatus> for enums::AttemptStatus {
    /// The whole point of this change: the attempt status comes from the verification result and
    /// from nothing else. A failed verification can never report success, and only an EXPLICIT
    /// decline is terminal.
    fn from(status: BraintreeVerificationStatus) -> Self {
        match status {
            // The card was verified against the network and vaulted. `Charged` is UCS's terminal
            // success status for a completed mandate setup; no money moved and none was captured
            // — a Verification is not a Transaction and can never be captured, voided or
            // refunded.
            BraintreeVerificationStatus::Verified => Self::Charged,
            // The issuer or the gateway said no. These are the only two arms that may be
            // terminal failures: the answer came back, and it was a refusal.
            BraintreeVerificationStatus::ProcessorDeclined
            | BraintreeVerificationStatus::GatewayRejected => Self::Failure,
            // Braintree could not reach the processor. Per the SDL this is a COMMUNICATION
            // failure, not a decision — exactly the ambiguous outcome that must not be stamped
            // `Failure`, because "we could not ask" is not "the issuer refused". `Unresolved`
            // leaves it open for a human or a retry instead of closing it against the merchant.
            BraintreeVerificationStatus::Failed => Self::Unresolved,
            // Still in flight. Braintree's own live behaviour for these two is UNVERIFIED
            // (§SM.15 item 3) — neither was ever observed on the sandbox and no sync query for a
            // verification has been exercised — so they are handled defensively as non-terminal
            // rather than assumed away.
            BraintreeVerificationStatus::Pending | BraintreeVerificationStatus::Verifying => {
                Self::Pending
            }
            // A value Braintree added after this enum was written. Unknown is not failure.
            BraintreeVerificationStatus::Unknown => Self::Pending,
        }
    }
}

/// Braintree GraphQL `PaymentMethodUsage`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, strum::Display, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BraintreePaymentMethodUsage {
    /// The vaulted, reusable payment method a successful vault produces. Multi-use methods do
    /// not expire.
    MultiUse,
    /// The nonce `tokenizeCreditCard` mints: one use, three-hour lifetime.
    SingleUse,
    #[serde(other)]
    Unknown,
}

/// `VaultPaymentMethodPayload.paymentMethod` — the vaulted card.
///
/// Present ONLY when the card was actually vaulted; a declined verification returns
/// `paymentMethod: null` alongside a fully populated `verification`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultedPaymentMethod {
    /// The OPAQUE GLOBAL id, and the only value that works as a mandate reference. Proven live
    /// in both directions: `chargePaymentMethod` with this id succeeds, and with `legacyId`
    /// fails with `legacyCode 91565`, "Unknown or expired single-use payment method" — a
    /// message that sends the reader hunting for an expiry problem that does not exist.
    pub id: Secret<String>,
    /// The classic (non-GraphQL) API's *token*. Useful for control-panel cross-reference and
    /// logging, and never usable as a GraphQL id.
    pub legacy_id: Option<Secret<String>>,
    pub usage: Option<BraintreePaymentMethodUsage>,
}

/// `MonetaryAmount` as selected under `paymentMethodVerificationDetails`.
///
/// Note the field name: `currencyIsoCode`, not the `currencyCode` the transaction amount uses.
/// Both exist on the SDL type; the selection set asks for the former.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeVerificationAmount {
    /// Major-unit decimal string, e.g. `"0.00"`.
    pub value: Option<String>,
    pub currency_iso_code: Option<String>,
}

/// `Verification.paymentMethodVerificationDetails` narrowed to its
/// `CreditCardVerificationDetails` member.
///
/// `VerificationDetails` is a union (`UsBankAccountVerificationDetails |
/// CreditCardVerificationDetails`), so a non-card member arrives as `{}` and must parse to
/// "amount absent". `CreditCardVerificationDetails` has exactly ONE field — no status, no
/// AVS/CVV, no NTID live here.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeVerificationDetails {
    #[serde(default)]
    pub amount: Option<BraintreeVerificationAmount>,
}

/// Braintree GraphQL `Verification` — the result of the zero-amount check.
///
/// A `Verification` is not a `Transaction`: different type, different id space, different status
/// enum, and it can never be captured, voided or refunded.
///
/// Every field is optional on purpose. `Verification` itself is nullable on the payload, the SDL
/// declares all of these nullable, and on a decline Braintree populates this object while leaving
/// `paymentMethod` null — so partial data is the normal case, not an error case.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeVerification {
    /// Opaque global id of the verification event.
    pub id: Option<String>,
    pub legacy_id: Option<String>,
    /// The single source of the attempt status. `None` (the field absent or null) is treated
    /// exactly like `Unknown`: non-terminal.
    pub status: Option<BraintreeVerificationStatus>,
    /// `GatewayRejectionReason` — non-null only when `status == GATEWAY_REJECTED`. Modelled as a
    /// string rather than an enum because it is surfaced verbatim as the error `reason` and is
    /// never branched on; a live sample was NOT reproducible on this sandbox merchant (§SM.15
    /// item 2), which is another reason not to pin a shape to it.
    pub gateway_rejection_reason: Option<String>,
    /// `VerificationProcessorResponse`. Declares the same AVS/CVV triple and legacyCode/message
    /// as the transaction shape, so it deserializes into the shared
    /// [`BraintreeProcessorResponse`]; `authorizationId` and `retrievalReferenceNumber` simply do
    /// not exist here and arrive as `None`.
    pub processor_response: Option<BraintreeProcessorResponse>,
    /// Sandbox returns the stub `{"code":"XX","message":"sample network response text"}` on every
    /// call; production values are UNVERIFIED (§SM.15 item 4). Read only as an error fallback.
    pub network_response: Option<BraintreeCodeMessage>,
    /// The only path to the NTID on a verification — see [`CreditCardTransactionSnapshot`] and
    /// the shared `payment_method_snapshot_fields!` selection.
    #[serde(default)]
    pub payment_method_snapshot: Option<CreditCardTransactionSnapshot>,
    /// Carries the amount Braintree actually authorized. Read back rather than assumed.
    #[serde(default)]
    pub payment_method_verification_details: Option<BraintreeVerificationDetails>,
}

impl BraintreeVerification {
    /// The attempt status this verification implies. A missing or null `status` is ambiguous,
    /// never a failure and never a success.
    fn attempt_status(&self) -> enums::AttemptStatus {
        self.status
            .unwrap_or(BraintreeVerificationStatus::Unknown)
            .into()
    }

    /// The scheme network transaction id, for `PaymentsResponseData::network_txn_id`.
    ///
    /// Gated on `VERIFIED` deliberately. Braintree assigns and returns an NTID on DECLINED
    /// verifications too — a `PROCESSOR_DECLINED` sandbox call returned
    /// `networkTransactionId: "020260916004852"` — and an NTID from a declined verification is
    /// not a usable stored-credential bootstrap: replaying it on a later merchant-initiated
    /// charge asserts a prior approval that never happened.
    fn network_transaction_id(&self) -> Option<String> {
        if self.status != Some(BraintreeVerificationStatus::Verified) {
            return None;
        }
        self.payment_method_snapshot
            .as_ref()?
            .network_transaction_id
            .clone()
    }

    /// The Mastercard TLID, for `PaymentsResponseData::network_txn_link_id`. Read only from
    /// `processorResponse` — never from the snapshot, which carries the NTID and not this
    /// (RULE M-1). `None` on every non-Mastercard scheme, which is the normal case.
    fn network_transaction_link_id(&self) -> Option<String> {
        self.processor_response
            .as_ref()?
            .mastercard_transaction_link_id
            .clone()
    }

    /// AVS and CVV verdicts on `PaymentFlowData.connector_response`, through the SAME builder
    /// Authorize uses. This is the one place a merchant can see that a card verified while its
    /// postal code did not match — which on a merchant with no gateway AVS/CVV rules configured
    /// still yields `status: VERIFIED` and still vaults the card, so the status alone is not a
    /// sufficient signal for a mandate.
    fn build_connector_response_data(&self) -> Option<ConnectorResponseData> {
        Some(build_card_payment_checks_response(
            self.processor_response.as_ref()?,
        ))
    }

    /// What Braintree actually verified, and what it vaulted — surfaced as `connector_metadata`
    /// so the amount is observable rather than assumed. The documented escalation from $0.00 to
    /// $1.00 on issuers that reject zero-amount authorizations was never reproduced on this
    /// sandbox (§SM.15 item 1), so the amount is reported, never hard-coded.
    fn build_connector_metadata(
        &self,
        payment_method: Option<&VaultedPaymentMethod>,
    ) -> Option<serde_json::Value> {
        let amount = self
            .payment_method_verification_details
            .as_ref()
            .and_then(|details| details.amount.as_ref());
        Some(serde_json::json!({
            "verification_id": self.id,
            "verification_legacy_id": self.legacy_id,
            "verification_status": self.status.map(|status| status.to_string()),
            "verification_amount": amount.and_then(|amount| amount.value.clone()),
            "verification_currency": amount.and_then(|amount| amount.currency_iso_code.clone()),
            "vaulted_payment_method_legacy_id": payment_method
                .and_then(|method| method.legacy_id.as_ref())
                .map(|legacy_id| legacy_id.peek().clone()),
            "vaulted_payment_method_usage": payment_method
                .and_then(|method| method.usage)
                .map(|usage| usage.to_string()),
        }))
    }

    /// Builds the error for a verification that did not succeed.
    ///
    /// `attempt_status` is passed in rather than recomputed, so the error can never disagree with
    /// the status the caller just recorded, and it is ALWAYS set (review Theme 9) — a `None` here
    /// would let a hard decline be re-read as "still pending" and retried forever.
    fn build_verification_error_response(
        &self,
        attempt_status: enums::AttemptStatus,
        envelope_errors: &[ErrorDetails],
        http_code: u16,
    ) -> domain_types::router_data::ErrorResponse {
        // Seeds code/message/reason from the verification status, then refines them below with
        // the processor's own values — the same shape Authorize's decline path produces.
        let mut error_response = create_failure_error_response(
            self.status.unwrap_or(BraintreeVerificationStatus::Unknown),
            self.id.clone(),
            http_code,
        );
        let processor_response = self.processor_response.as_ref();

        // The processor's authorization response code, e.g. `2000` "Do Not Honor", falling back
        // to the card network's own code. This is what Hyperswitch's GSM keys smart retry off.
        error_response.network_decline_code = processor_response
            .and_then(|response| response.legacy_code.clone())
            .or_else(|| {
                self.network_response
                    .as_ref()
                    .and_then(|network| network.code.clone())
            });
        error_response.network_error_message = processor_response
            .and_then(|response| response.message.clone())
            .or_else(|| {
                self.network_response
                    .as_ref()
                    .and_then(|network| network.message.clone())
            });
        // `network_advice_code` stays None on purpose: the Mastercard merchant advice code lives
        // on `ProcessorDeclinedEvent` / `GatewayRejectedEvent` / `FailedEvent`, which are
        // `Transaction.statusHistory` types. A Verification has no status history at all, so
        // there is nowhere to read one from — inventing a fallback would misreport the scheme's
        // retry guidance.

        // Prefer the processor's own code and text over the bare status string, so the merchant
        // sees "2000 / Do Not Honor" rather than just "PROCESSOR_DECLINED".
        if let Some(code) = processor_response.and_then(|response| response.legacy_code.clone()) {
            error_response.code = code;
        }
        if let Some(message) = processor_response.and_then(|response| response.message.clone()) {
            error_response.message = message;
        }
        // Reason, most specific first: the fraud rule that rejected it, then the processor's
        // expanded text, then whatever the GraphQL error envelope said. A declined verification
        // arrives as HTTP 200 with BOTH `data` and `errors` populated, so the envelope is a real
        // fallback here and not a theoretical one.
        if let Some(reason) = self
            .gateway_rejection_reason
            .clone()
            .or_else(|| {
                processor_response.and_then(|response| response.additional_information.clone())
            })
            .or_else(|| join_error_messages(envelope_errors))
        {
            error_response.reason = Some(reason);
        }

        error_response.attempt_status = Some(FlowStatus::Payment(attempt_status));
        error_response
    }
}

/// `VaultPaymentMethodPayload`, returned by `vaultCreditCard`.
///
/// Both members are nullable and their combination is the whole decision:
/// `paymentMethod` present means the card was vaulted, `verification` carries the outcome.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCreditCardPayload {
    #[serde(default)]
    pub payment_method: Option<VaultedPaymentMethod>,
    /// `null` only when verification was skipped. This connector never sends `skip`, so a null
    /// here is an unmodelled response and is treated as non-terminal rather than as success.
    #[serde(default)]
    pub verification: Option<BraintreeVerification>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCreditCardResponseData {
    #[serde(default)]
    pub vault_credit_card: Option<VaultCreditCardPayload>,
}

/// The `vaultCreditCard` response envelope.
///
/// Modelled as ONE struct with two optional halves rather than as an untagged
/// `Success | Error` enum, because on this flow a single HTTP 200 routinely carries BOTH. A
/// declined verification returns `errors: [{ "message": "Payment method failed verification." }]`
/// AND `data.vaultCreditCard.verification` fully populated with `legacyCode 2000` / "Do Not
/// Honor". An untagged enum picks one arm and discards the other: short-circuiting on `errors`
/// throws away the decline reason and reports a useless generic error, while reading only `data`
/// dereferences a null `paymentMethod`. Both halves must be readable at once.
///
/// `data` is optional because a document-level validation error (a bad selection set) returns no
/// `data` key whatsoever.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeSetupMandateResponse {
    #[serde(default)]
    pub data: Option<VaultCreditCardResponseData>,
    #[serde(default)]
    pub errors: Option<Vec<ErrorDetails>>,
}

impl BraintreeSetupMandateResponse {
    fn payload(&self) -> Option<&VaultCreditCardPayload> {
        self.data.as_ref()?.vault_credit_card.as_ref()
    }

    fn envelope_errors(&self) -> &[ErrorDetails] {
        self.errors.as_deref().unwrap_or_default()
    }
}

/// Joins every message in a GraphQL error envelope, or `None` when the envelope is empty.
fn join_error_messages(errors: &[ErrorDetails]) -> Option<String> {
    (!errors.is_empty()).then(|| {
        errors
            .iter()
            .map(|error| error.message.clone())
            .collect::<Vec<String>>()
            .join(" ")
    })
}

/// The attempt status for a HARD error — `data.vaultCreditCard` is null, or there is no `data`
/// key at all, so no verification was performed and nothing was vaulted.
///
/// Branches on Braintree's own `errorClass` rather than stamping one status on every failure. A
/// `VALIDATION` rejection (a spent token, a bad merchant account id) will never succeed as sent
/// and is safely terminal; an `INTERNAL` or `SERVICE_AVAILABILITY` fault, or a class this enum
/// does not recognise, is ambiguous and must stay non-terminal so a retry is still possible
/// (review Theme 1). An empty envelope is likewise ambiguous.
fn hard_error_attempt_status(errors: &[ErrorDetails]) -> enums::AttemptStatus {
    let terminal = errors
        .iter()
        .filter_map(|error| error.extensions.as_ref())
        .filter_map(|extensions| extensions.error_class)
        .any(BraintreeErrorClass::is_terminal_rejection);
    if terminal {
        enums::AttemptStatus::Failure
    } else {
        enums::AttemptStatus::Pending
    }
}

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
    /// Implements the four-step decision order the live response shapes force (tech spec §SM.9.1).
    /// The order matters: `errors` being present does NOT mean there is no usable data, and
    /// `data` being present does NOT mean the card was verified.
    fn try_from(
        item: ResponseRouterData<BraintreeSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let envelope_errors = item.response.envelope_errors();

        // STEP 1 — `data.vaultCreditCard` is null, or the whole `data` key is missing. The
        // mutation never ran: no verification, no vault. Everything knowable is in `errors[0]`.
        let Some(payload) = item.response.payload() else {
            let status = hard_error_attempt_status(envelope_errors);
            let response = build_error_response(envelope_errors, item.http_code).map_err(
                |mut error_response| {
                    error_response.attempt_status = Some(FlowStatus::Payment(status));
                    *error_response
                },
            );
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response,
                ..item.router_data
            });
        };

        let verification = payload.verification.as_ref();
        let vaulted_payment_method = payload.payment_method.as_ref();
        // Mapped from the verification result and from nothing else. A null `verification` can
        // only happen if `skip` were sent, which this connector never does, so it is treated as
        // an unmodelled response: non-terminal, never success.
        let status = verification.map_or(enums::AttemptStatus::Pending, |verification| {
            verification.attempt_status()
        });
        // AVS/CVV are reported on approvals and declines alike, so this is computed once, before
        // the branch, and attached to every outcome.
        let connector_response =
            verification.and_then(|verification| verification.build_connector_response_data());
        let connector_metadata = verification
            .and_then(|verification| verification.build_connector_metadata(vaulted_payment_method));

        let (status, response) = match (status, vaulted_payment_method) {
            // STEP 4 — verified AND vaulted. The only path that reports success.
            (enums::AttemptStatus::Charged, Some(payment_method)) => (
                status,
                Ok(PaymentsResponseData::TransactionResponse {
                    // The vaulted global id. Kept as the resource id (rather than the
                    // verification id) because it is what every later call anchors on; the
                    // verification's own id is reported as the response reference below.
                    resource_id: ResponseId::ConnectorTransactionId(
                        payment_method.id.peek().clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: Some(Box::new(MandateReference {
                        // `paymentMethod.id`, NEVER `legacyId` — see `VaultedPaymentMethod::id`.
                        connector_mandate_id: Some(payment_method.id.peek().clone()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    })),
                    connector_metadata,
                    // Captured off the verification's own `paymentMethodSnapshot` so a later
                    // merchant-initiated charge can replay the stored-credential chain without
                    // first having to make a chargeable customer-initiated payment. Gated on
                    // VERIFIED inside the accessor.
                    network_txn_id: verification
                        .and_then(|verification| verification.network_transaction_id()),
                    network_txn_link_id: verification
                        .and_then(|verification| verification.network_transaction_link_id()),
                    connector_response_reference_id: verification
                        .and_then(|verification| verification.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
            ),
            // STEP 3 — Braintree says VERIFIED but returned no vaulted payment method. This
            // should not happen. It is NOT reported as success: there is no mandate reference to
            // hand back, so a caller told "Charged" would later replay a mandate that does not
            // exist. It is not reported as a decline either — nothing was declined — so it lands
            // on `Unresolved` for a human to look at.
            (enums::AttemptStatus::Charged, None) => {
                let status = enums::AttemptStatus::Unresolved;
                let mut error_response = create_failure_error_response(
                    BraintreeVerificationStatus::Verified,
                    verification.and_then(|verification| verification.id.clone()),
                    item.http_code,
                );
                if let Some(first_error) = envelope_errors.first() {
                    error_response.code = first_error
                        .extensions
                        .as_ref()
                        .and_then(|extensions| extensions.legacy_code.clone())
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string());
                    error_response.message = first_error.message.clone();
                }
                error_response.reason = join_error_messages(envelope_errors).or(Some(
                    "Braintree reported the card as VERIFIED but returned no vaulted payment \
                     method, so no mandate reference exists"
                        .to_string(),
                ));
                error_response.attempt_status = Some(FlowStatus::Payment(status));
                (status, Err(error_response))
            }
            // STEP 2 — an explicit decline. `is_payment_failure` is the shared predicate, so this
            // arm is reached only for PROCESSOR_DECLINED and GATEWAY_REJECTED; FAILED, PENDING,
            // VERIFYING and an unrecognised status are non-terminal and fall through below.
            (status, _) if domain_types::utils::is_payment_failure(status) => (
                status,
                Err(verification.map_or_else(
                    // Unreachable in practice — a non-Charged, failing status can only come from
                    // a verification that exists — but written out rather than unwrapped so the
                    // decline still carries the envelope's own message if it ever is.
                    || {
                        let mut error_response = create_failure_error_response(
                            BraintreeVerificationStatus::Unknown,
                            None,
                            item.http_code,
                        );
                        error_response.reason = join_error_messages(envelope_errors);
                        error_response.attempt_status = Some(FlowStatus::Payment(status));
                        error_response
                    },
                    |verification| {
                        verification.build_verification_error_response(
                            status,
                            envelope_errors,
                            item.http_code,
                        )
                    },
                )),
            ),
            // Non-terminal: PENDING, VERIFYING, an unrecognised status, a missing verification,
            // or FAILED (Braintree could not reach the processor). The attempt stays open. No
            // mandate reference and no NTID are reported, because neither is established yet —
            // reporting either would let a caller act on a verification that has not happened.
            (status, payment_method) => (
                status,
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: payment_method.map_or(ResponseId::NoResponseId, |method| {
                        ResponseId::ConnectorTransactionId(method.id.peek().clone())
                    }),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: verification
                        .and_then(|verification| verification.id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
            ),
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
                            credit_card: BraintreeTokenizeCard::Raw(CreditCardData {
                                number: card_data.card_number.clone(),
                                expiration_year: card_data.card_exp_year.clone(),
                                expiration_month: card_data.card_exp_month.clone(),
                                cvv: Some(card_data.card_cvc.clone()),
                                cardholder_name: item
                                    .router_data
                                    .resource_common_data
                                    .get_optional_billing_full_name()
                                    .unwrap_or_else(|| Secret::new(String::new())),
                            }),
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
