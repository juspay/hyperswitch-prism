use crate::{connectors::braintree::BraintreeRouterData, types::ResponseRouterData, utils};
use base64::Engine;
use common_enums::enums;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::XmlExt,
    pii,
    types::{AmountConvertor, MinorUnit, StringMajorUnit},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PaymentMethodToken, RSync,
        RepeatPayment, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        self, AmountInfo, ApplePayPaymentRequest, ApplePaySessionResponse,
        ApplepayClientAuthenticationResponse, BillingDescriptor, ClientAuthenticationTokenData,
        ClientAuthenticationTokenRequestData, GooglePaySessionResponse,
        GpayAllowedMethodsParameters, GpayAllowedPaymentMethods, GpayClientAuthenticationResponse,
        GpayMerchantInfo, GpayShippingAddressParameters, GpayTokenParameters,
        GpayTokenizationSpecification, GpayTransactionInfo, L2L3Data, MandateReference,
        NextActionCall, PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentRequestMetadata, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        PaypalClientAuthenticationResponse, PaypalTransactionInfo, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SdkNextAction,
        SecretInfoToInitiateSdk, SetupMandateRequestData, ThirdPartySdkSessionResponse,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::{Address, AddressDetails, OrderDetailsWithAmount, PhoneDetails},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types,
    router_response_types::RedirectForm,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use strum::Display;
use time::PrimitiveDateTime;
use tracing::{info, warn};

pub const BRAINTREE_CONNECTOR_NAME: &str = "braintree";

pub mod constants {
    /// The response-and-error surface selected by every card Authorize / Capture mutation:
    /// AVS and CVV check results, the processor's own authorization and settlement codes,
    /// the Mastercard Merchant Advice Code, the raw card-network response and the gateway
    /// rejection reason. Without it a decline comes back as a bare `PROCESSOR_DECLINED`
    /// with no reason attached.
    ///
    /// Three things about this selection are easy to get wrong:
    ///
    /// * **The GraphQL names are not the REST names.** The REST/server-SDK docs describe
    ///   `processor_response_code`, `cvv_response_code`, `avs_postal_code_response_code`.
    ///   In GraphQL those are `legacyCode`, `cvvResponse` and `avsPostalCodeResponse` — no
    ///   `Code` suffix. There is no `avsErrorResponseCode` at all; its `S`/`E` outcomes
    ///   fold into the two AVS fields as `ISSUER_DOES_NOT_PARTICIPATE` / `SYSTEM_ERROR`
    ///   (confirmed against sandbox with billing postal codes `30001` / `30000`).
    /// * **`declineType`, `merchantAdviceCodeResponse`, `networkResponse` and
    ///   `gatewayRejectionReason` are not on `Transaction`.** They exist only on typed
    ///   members of `statusHistory` (there is no `statusEvents` field) and are therefore
    ///   reachable only through inline fragments.
    /// * **AVS/CVV cannot be read off a capture.** The equivalent members of
    ///   `TransactionSettlementProcessorResponse` are `@deprecated` because the checks only
    ///   happen at authorization time, so they are not selected here; a capture response
    ///   simply repeats the authorization's values.
    ///
    /// The event-level `processorResponse` is deliberately not selected: it duplicates
    /// `Transaction.processorAuthorizationResponse`, and selecting it on both the decline
    /// and settlement fragments would collide two differently-typed fields of the same name.
    ///
    /// Every field below was confirmed to resolve against the Braintree sandbox under the
    /// pinned `Braintree-Version: 2019-01-01` header before being added here — a selection
    /// on a field the versioned schema does not expose is a hard GraphQL validation error
    /// that would break every Authorize call, not just the decline path.
    ///
    /// Kept as a macro rather than a `const` so `concat!` can splice it into each query
    /// literal while the constants stay `&'static str`.
    macro_rules! txn_response_surface {
        () => {
            "processorAuthorizationResponse { legacyCode message cvvResponse avsPostalCodeResponse avsStreetAddressResponse authorizationId additionalInformation } \
             processorSettlementResponse { legacyCode message } \
             statusHistory { status terminal \
               ... on AuthorizedEvent { riskDecision networkResponse { code message } } \
               ... on ProcessorDeclinedEvent { declineType riskDecision networkResponse { code message } merchantAdviceCodeResponse { code message } } \
               ... on GatewayRejectedEvent { gatewayRejectionReason riskDecision networkResponse { code message } merchantAdviceCodeResponse { code message } } \
               ... on FailedEvent { riskDecision networkResponse { code message } merchantAdviceCodeResponse { code message } } }"
        };
    }

    pub const CHANNEL_CODE: &str = "HyperSwitchBT_Ecom";
    pub const CLIENT_TOKEN_MUTATION: &str = "mutation createClientToken($input: CreateClientTokenInput!) { createClientToken(input: $input) { clientToken}}";
    pub const TOKENIZE_CREDIT_CARD: &str = "mutation  tokenizeCreditCard($input: TokenizeCreditCardInput!) { tokenizeCreditCard(input: $input) { clientMutationId paymentMethod { id } } }";
    pub const CHARGE_CREDIT_CARD_MUTATION: &str = concat!(
        "mutation ChargeCreditCard($input: ChargeCreditCardInput!) { chargeCreditCard(input: $input) { transaction { id legacyId createdAt amount { value currencyCode } status ",
        txn_response_surface!(),
        " } } }"
    );
    pub const AUTHORIZE_CREDIT_CARD_MUTATION: &str = concat!(
        "mutation authorizeCreditCard($input: AuthorizeCreditCardInput!) { authorizeCreditCard(input: $input) {  transaction { id legacyId amount { value currencyCode } status ",
        txn_response_surface!(),
        " } } }"
    );
    pub const CAPTURE_TRANSACTION_MUTATION: &str = concat!(
        "mutation captureTransaction($input: CaptureTransactionInput!) { captureTransaction(input: $input) { clientMutationId transaction { id legacyId amount { value currencyCode } status ",
        txn_response_surface!(),
        " } } }"
    );
    // `reverseTransaction` returns the union `TransactionReversal = Refund | Transaction`:
    // an unsettled transaction is voided (Transaction branch), a settled one is refunded in
    // full (Refund branch). Selecting only `... on Transaction` makes the settled case come
    // back as `"reversal": {}` — the reversal really happened but the response cannot be
    // deserialized and the new refund id is lost, so both branches must be selected.
    pub const VOID_TRANSACTION_MUTATION: &str = "mutation voidTransaction($input:  ReverseTransactionInput!) { reverseTransaction(input: $input) { clientMutationId reversal { __typename ...  on Transaction { id legacyId amount { value currencyCode } status } ... on Refund { id legacyId amount { value currencyCode } status } } } }";
    pub const REFUND_TRANSACTION_MUTATION: &str = "mutation refundTransaction($input:  RefundTransactionInput!) { refundTransaction(input: $input) {clientMutationId refund { id legacyId amount { value currencyCode } status } } }";
    pub const AUTHORIZE_AND_VAULT_CREDIT_CARD_MUTATION: &str = concat!(
        "mutation authorizeCreditCard($input: AuthorizeCreditCardInput!) { authorizeCreditCard(input: $input) { transaction { id status createdAt paymentMethod { id } ",
        txn_response_surface!(),
        " } } }"
    );
    pub const CHARGE_AND_VAULT_TRANSACTION_MUTATION: &str = concat!(
        "mutation ChargeCreditCard($input: ChargeCreditCardInput!) { chargeCreditCard(input: $input) { transaction { id status createdAt paymentMethod { id } ",
        txn_response_surface!(),
        " } } }"
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
    /// Wallets go out on `chargePaymentMethod` / `authorizePaymentMethod`, which still take a
    /// full `TransactionInput` — so every transaction-level enrichment field is available
    /// here. Only the billing address is not: those two inputs have no `options` member.
    #[serde(flatten)]
    enrichment: TransactionEnrichment,
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
    /// This is the customer-initiated transaction that establishes the stored credential,
    /// so it must be flagged `RECURRING_FIRST` for the later MIT to be scheme-compliant.
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

#[derive(Debug, Serialize)]
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
            // Braintree's `chargeCreditCard` / `authorizeCreditCard` mutations take a
            // `paymentMethodId`, never raw PAN, so a card authorize always arrives here as
            // the `PaymentMethodToken` produced by the PaymentMethodToken (tokenizeCreditCard)
            // flow — that is what `should_do_payment_method_token` requests for
            // `PaymentMethod::Card`, and what `PaymentService/TokenAuthorize` sends
            // (`tokenized_authorize_to_base` maps `connector_token` onto this variant).
            // `Card` is kept on the same arm so the 3DS client-token branch still triggers
            // and so a raw-card caller gets the explicit "payment_method_token" error from
            // `CardPaymentRequest` rather than an opaque "payment method not supported".
            PaymentMethodData::Card(_) | PaymentMethodData::PaymentMethodToken(_) => {
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
                let enrichment = TransactionEnrichment::build(EnrichmentInputs {
                    amount_converter: item.connector.amount_converter,
                    l2_l3_data: item.router_data.resource_common_data.l2_l3_data.as_deref(),
                    order_details: item.router_data.resource_common_data.order_details.as_ref(),
                    shipping: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping(),
                    billing_descriptor: item.router_data.request.billing_descriptor.as_ref(),
                    surcharge_amount: item
                        .router_data
                        .request
                        .surcharge_amount
                        .as_ref()
                        .map(|surcharge| surcharge.amount),
                    shipping_cost: item.router_data.request.shipping_cost,
                    amount: item.router_data.request.minor_amount,
                    currency: item.router_data.request.currency,
                })?;

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
                                        enrichment: enrichment.clone(),
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
                                        enrichment: enrichment.clone(),
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
                                        enrichment: enrichment.clone(),
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
    /// AVS / CVV, processor codes and the typed status events. Flattened because the
    /// mutation selects them directly on `transaction`, alongside `id` and `status`.
    #[serde(flatten)]
    response_surface: TransactionResponseSurface,
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
                let surface = &transaction_data.response_surface;
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_declined_error_response(
                        &transaction_data.status,
                        surface,
                        Some(transaction_data.id.clone()),
                        status,
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
                        // AVS / CVV outcome and the acquirer auth code travel on the
                        // success path too — an approved transaction can still carry an
                        // AVS mismatch when the merchant has no AVS rule enabled.
                        connector_response: build_card_connector_response(surface),
                        raw_connector_status: build_raw_connector_status(surface),
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

/// Build the `ErrorResponse` for a transaction Braintree *accepted* — HTTP 200, no
/// `errors[]` entry — but refused: `PROCESSOR_DECLINED`, `GATEWAY_REJECTED`, `FAILED`,
/// `SETTLEMENT_DECLINED` or `AUTHORIZATION_EXPIRED`. Braintree answers 200 for everything,
/// including declines, so this is the only place the refusal reason can be recovered.
///
/// The field choices are dictated by what the Hyperswitch Gateway Status Map keys on, so
/// that smart retry can make a per-decline-reason decision instead of treating every
/// Braintree failure alike:
///
/// * `code` / `message` — the processor's own code and text (`2001` / `Insufficient
///   Funds`), which the GSM looks up as connector_error_code / connector_error_message. For
///   a settlement refusal these come from the 4000-class settlement response instead, and
///   for a gateway rejection from the rejection reason, because the processor never saw it.
/// * `network_advice_code` — the Mastercard Merchant Advice Code (`01`), which Hyperswitch
///   looks up in `merchant_advice_codes.<network>.<code>` to obtain a recommended action.
/// * `network_decline_code` — the raw card-network response code, which Hyperswitch uses as
///   the GSM `issuer_error_code` (`Network:{brand}|IssuerCode:{code}`).
/// * `network_error_message` — the network's own text.
/// * `reason` — the human-readable remediation. `ErrorResponse` has no `suggested_action`
///   or `doc_url` field, so the per-decline-class guidance (hard vs soft, the merchant
///   advice code, the gateway rejection reason) has nowhere else to travel.
///
/// `attempt_status` is set explicitly rather than left to the 2xx fallback: Braintree's
/// refusals are terminal, and a terminal connector state that reports as anything but a
/// terminal UCS state leaves the attempt polling forever.
fn create_declined_error_response(
    status: &BraintreePaymentStatus,
    surface: &TransactionResponseSurface,
    connector_transaction_id: Option<String>,
    attempt_status: enums::AttemptStatus,
    http_code: u16,
) -> domain_types::router_data::ErrorResponse {
    let processor = surface.processor_authorization_response.as_ref();
    let settlement = surface.processor_settlement_response.as_ref();
    let gateway_rejection = surface.gateway_rejection_reason();

    // A capture-time refusal reports in the 4000-class settlement response; an
    // authorization-time refusal reports in the 1000/2000/3000-class authorization response.
    let settlement_first = matches!(status, BraintreePaymentStatus::SettlementDeclined);
    let (primary_code, primary_message) = if settlement_first {
        (
            settlement.and_then(|s| s.legacy_code.clone()),
            settlement.and_then(|s| s.message.clone()),
        )
    } else {
        (
            processor.and_then(|p| p.legacy_code.clone()),
            processor.and_then(|p| p.message.clone()),
        )
    };
    let (fallback_code, fallback_message) = if settlement_first {
        (
            processor.and_then(|p| p.legacy_code.clone()),
            processor.and_then(|p| p.message.clone()),
        )
    } else {
        (
            settlement.and_then(|s| s.legacy_code.clone()),
            settlement.and_then(|s| s.message.clone()),
        )
    };

    let code = primary_code
        .or(fallback_code)
        // A gateway rejection is blocked by the merchant's own gateway rules before the
        // processor is reached, so it has no processor code. The rejection reason is the
        // most specific identifier available and distinguishes the causes from each other.
        .or_else(|| gateway_rejection.map(|reason| reason.to_string()))
        // Never NO_ERROR_CODE: the transaction status is always known and is more useful
        // than a blank field reaching the merchant.
        .unwrap_or_else(|| status.wire_value().to_string());
    let message = primary_message
        .or(fallback_message)
        .unwrap_or_else(|| status.wire_value().to_string());

    // Everything that explains *why* and what may be done about it, most specific first.
    let mut reason_parts: Vec<String> = Vec::new();
    if let Some(additional) = processor.and_then(|p| p.additional_information.clone()) {
        reason_parts.push(additional);
    }
    if let Some(decline_type) = surface.decline_type() {
        if let Some(guidance) = decline_type.guidance() {
            reason_parts.push(guidance.to_string());
        }
    }
    if let Some(reason) = gateway_rejection {
        reason_parts.push(format!(
            "gateway rejection ({reason}): {}",
            reason.guidance()
        ));
    }
    if let Some(advice_code) = surface.merchant_advice_code() {
        let advice_text = surface
            .merchant_advice_message()
            .or_else(|| merchant_advice_code_guidance(&advice_code).map(str::to_string));
        reason_parts.push(match advice_text {
            Some(text) => format!("merchant advice code {advice_code}: {text}"),
            None => format!("merchant advice code {advice_code}"),
        });
    }
    if let Some(RiskDecision::Review) = surface.risk_decision() {
        reason_parts.push("flagged for manual review by risk rules".to_string());
    }

    domain_types::router_data::ErrorResponse {
        code,
        message,
        reason: Some(if reason_parts.is_empty() {
            status.wire_value().to_string()
        } else {
            reason_parts.join("; ")
        }),
        status_code: http_code,
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
            attempt_status,
        )),
        connector_transaction_id,
        network_advice_code: surface.merchant_advice_code(),
        network_decline_code: surface.network_code(),
        network_error_message: surface.network_message(),
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
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
    /// Braintree's SDL spells this `AUTHORIZATION_EXPIRED` (legacy REST:
    /// `authorization_expired`). The variant used to be `AuthorizedExpired`, which
    /// `SCREAMING_SNAKE_CASE` renders as `AUTHORIZED_EXPIRED` — a value the gateway never
    /// sends, so an aged-out authorization failed to deserialize instead of mapping to
    /// `AuthorizationFailed`.
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
}

// ---------------------------------------------------------------------------
// Response & error surface: AVS / CVV, processor codes, Mastercard advice codes.
//
// None of this data hangs off `Transaction` directly. The processor's authorization and
// settlement responses do; everything else (hard/soft decline type, merchant advice code,
// raw network response, gateway rejection reason) lives on typed members of
// `Transaction.statusHistory` and is only reachable through inline fragments. See
// `constants::txn_response_surface!` for the selection set that fetches it.
// ---------------------------------------------------------------------------

/// Braintree's AVS / CVV check outcome. A single SDL enum (`AvsCvvResponseCode`) types all
/// three of `cvvResponse`, `avsPostalCodeResponse` and `avsStreetAddressResponse`.
///
/// `#[serde(other)]` is deliberate: these fields are nullable and an outcome we do not
/// recognise must degrade to `Unknown` rather than fail the whole Authorize response.
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
    /// The single-letter REST / server-SDK representation of the same outcome. Braintree's
    /// own AVS and CVV reference tables — and every downstream AVS/CVV consumer, including
    /// the `payment_checks` bag the Cybersource and Bank of America connectors already
    /// populate — speak in these letters; the long names exist only in GraphQL.
    ///
    /// `Unknown` has no letter to map to, so the absence is reported rather than guessed.
    pub fn as_rest_code(self) -> Option<&'static str> {
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

    /// Whether the value provided by the shopper failed to match the issuer's record.
    /// `NOT_VERIFIED` / `NOT_PROVIDED` / `ISSUER_DOES_NOT_PARTICIPATE` are explicitly not
    /// mismatches — the check simply did not happen.
    pub fn is_mismatch(self) -> bool {
        matches!(self, Self::DoesNotMatch)
    }
}

/// `ProcessorDeclinedEvent.declineType` — Braintree's own hard/soft discriminator, and the
/// authoritative answer to "may this be retried with the same credential?". It is on the
/// status event only, never on the processor-response object, which is why the inline
/// fragment in the selection set is not optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessorDeclineType {
    Hard,
    Soft,
    #[serde(other)]
    Unknown,
}

impl ProcessorDeclineType {
    /// Remediation text, taken from the SDL's own docstrings for the two values.
    pub fn guidance(self) -> Option<&'static str> {
        match self {
            Self::Hard => Some(
                "hard decline: the issue is not temporary, do not retry with the same payment method",
            ),
            Self::Soft => Some("soft decline: temporary issue, a later retry may succeed"),
            Self::Unknown => None,
        }
    }
}

/// `GatewayRejectedEvent.gatewayRejectionReason`. All fourteen SDL values are matched — the
/// prose documentation lists only nine, and the five it omits (`EXCESSIVE_RETRY`,
/// `MANUAL_TRANSACTIONS_DISABLED`, `TOO_MANY_CONFIRMATION_ATTEMPTS`,
/// `UNION_PAY_ENROLLMENT_REQUIRED`, `AVS_AND_CVV`) are real and must not fail parsing.
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

impl GatewayRejectionReason {
    /// Per-reason remediation. A gateway rejection is not an issuer decline: the merchant's
    /// own gateway settings blocked it, so the advice is about the merchant's configuration
    /// or the data supplied, not about the card.
    ///
    /// Note for any future Void work: a `GATEWAY_REJECTED` transaction that had already been
    /// authorized is voided automatically by the gateway, so the connector must never issue
    /// a Void against one.
    pub fn guidance(self) -> &'static str {
        match self {
            Self::Avs => "rejected by the merchant's AVS rules: correct the billing address before retrying",
            Self::Cvv => "rejected by the merchant's CVV rules: correct the card verification value before retrying",
            Self::AvsAndCvv => "rejected by the merchant's AVS and CVV rules: correct both the billing address and the card verification value before retrying",
            Self::Duplicate => "rejected as a duplicate of an earlier transaction: reconcile against the original rather than retrying",
            Self::Fraud | Self::RiskThreshold => "rejected by the merchant's fraud or risk rules: do not retry",
            Self::ThreeDSecure => "rejected by the merchant's 3D Secure rules: retry only after a successful authentication",
            Self::ApplicationIncomplete => "the merchant account application is incomplete: this is a provisioning issue, not a payment issue",
            Self::PaymentMethodBlocked => "the payment method is blocked for this merchant: do not retry",
            Self::TokenIssuance => "the payment method token could not be issued: do not retry with the same token",
            Self::ExcessiveRetry | Self::TooManyConfirmationAttempts => "too many attempts against this payment: back off before retrying",
            Self::ManualTransactionsDisabled => "manual transactions are disabled for this merchant account: enable them in the Control Panel",
            Self::UnionPayEnrollmentRequired => "UnionPay enrolment is required for this card: do not retry as-is",
            Self::Unknown => "rejected by the merchant's gateway settings",
        }
    }
}

/// `riskDecision` on the authorization / decline / rejection events. `REVIEW` is the value
/// that matters operationally: the transaction may be authorized yet flagged for manual
/// review, which is not a clean success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskDecision {
    Approve,
    Decline,
    NotEvaluated,
    Review,
    #[serde(other)]
    Unknown,
}

/// Remediation for a Mastercard Merchant Advice Code.
///
/// The published list is `01`–`04`, `21`, `24`–`30`, `40`, `41` and `43`; there is no code
/// `42`, and `05`–`20`, `22`–`23` and `31`–`39` are not published either. An unlisted code
/// returns `None` rather than being guessed — the raw code still travels to the caller in
/// `ErrorResponse::network_advice_code`, where the Hyperswitch Gateway Status Map looks it
/// up per card network.
fn merchant_advice_code_guidance(code: &str) -> Option<&'static str> {
    // Braintree sends the code zero-padded ("01"), but pad defensively so a bare "1" from a
    // future response still resolves to the same advice.
    let padded = if code.len() == 1 {
        format!("0{code}")
    } else {
        code.to_string()
    };
    match padded.as_str() {
        "01" => Some("new account information available: retry only with updated card details"),
        "02" => Some("cannot approve at this time: retry later"),
        "03" => Some("do not try again"),
        "04" => Some("token not supported: retry only with a different credential form"),
        "21" => Some("stop the recurring payment: cancel the series"),
        "24" => Some("retry after 1 hour"),
        "25" => Some("retry after 24 hours"),
        "26" => Some("retry after 2 days"),
        "27" => Some("retry after 4 days"),
        "28" => Some("retry after 6 days"),
        "29" => Some("retry after 8 days"),
        "30" => Some("retry after 10 days"),
        "40" => Some("non-reloadable prepaid card: do not retry"),
        "41" => Some("single-use virtual card number already spent: do not retry"),
        "43" => Some("multi-use virtual card number"),
        _ => None,
    }
}

/// `Transaction.processorAuthorizationResponse` — the authorization-time processor result.
/// This replaces the `@deprecated` `Transaction.processorResponse`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorAuthorizationResponse {
    /// The processor's own response code, e.g. `"2001"`. A `String` in the SDL, not an
    /// integer — compare as text.
    pub legacy_code: Option<String>,
    pub message: Option<String>,
    pub cvv_response: Option<AvsCvvResponseCode>,
    pub avs_postal_code_response: Option<AvsCvvResponseCode>,
    pub avs_street_address_response: Option<AvsCvvResponseCode>,
    /// The acquirer authorization code (REST `processor_authorization_code`).
    pub authorization_id: Option<String>,
    pub additional_information: Option<String>,
}

/// `Transaction.processorSettlementResponse` — the 4000-class settlement result. Null until
/// a settlement is attempted, which is why a Capture that is refused reports here rather
/// than in the authorization response.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorSettlementResponse {
    pub legacy_code: Option<String>,
    pub message: Option<String>,
}

/// The shape shared by `MerchantAdviceCodeResponse` and `PaymentNetworkResponse`.
///
/// `code` stays a `String` rather than becoming an enum on purpose: the SDL types it as a
/// bare nullable `String`, Braintree does not constrain it, and a network response code is
/// only interpretable together with the card brand (Visa `05` and Amex `000` mean opposite
/// things). Turning it into an enum would either fail on unknown values or invent meaning.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BraintreeCodeMessage {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// One member of `Transaction.statusHistory`, flattened across the `PaymentStatusEvent`
/// implementations: every extra member is optional and only the inline fragment that
/// matched populates it. `declineType` therefore identifies a `ProcessorDeclinedEvent` and
/// `gatewayRejectionReason` a `GatewayRejectedEvent`.
///
/// `status` is deliberately an untyped `String` and not `BraintreePaymentStatus`: the
/// history carries every status the transaction has ever held, so a single value Braintree
/// adds later would otherwise fail the entire Authorize response rather than just this one
/// history entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreeStatusEvent {
    pub status: Option<String>,
    /// SDL: "Whether this is the final state for the payment."
    pub terminal: Option<bool>,
    pub decline_type: Option<ProcessorDeclineType>,
    pub gateway_rejection_reason: Option<GatewayRejectionReason>,
    pub risk_decision: Option<RiskDecision>,
    pub network_response: Option<BraintreeCodeMessage>,
    pub merchant_advice_code_response: Option<BraintreeCodeMessage>,
}

/// The Authorize / Capture response surface, flattened into each transaction body so the
/// card charge, card authorize and capture selections all share one shape. Every member is
/// optional: a mutation whose selection set has not been widened simply yields `None`
/// throughout and the connector falls back to the transaction status.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponseSurface {
    pub processor_authorization_response: Option<ProcessorAuthorizationResponse>,
    pub processor_settlement_response: Option<ProcessorSettlementResponse>,
    /// Returned in reverse chronological order, most recent event first.
    pub status_history: Option<Vec<BraintreeStatusEvent>>,
}

impl TransactionResponseSurface {
    /// Scan the history most-recent-first for the first event that carries `f`.
    ///
    /// A plain `statusHistory[0]` lookup is not enough: on an approved auto-capture the
    /// most recent event is `SUBMITTED_FOR_SETTLEMENT`, which carries no network response —
    /// the `AuthorizedEvent` that does is the entry behind it.
    fn find_in_history<'a, R>(
        &'a self,
        f: impl Fn(&'a BraintreeStatusEvent) -> Option<R>,
    ) -> Option<R> {
        self.status_history.iter().flatten().find_map(f)
    }

    /// The Mastercard Merchant Advice Code, e.g. `"01"`. Populated only for Mastercard and
    /// only on a declined / rejected / failed event.
    pub fn merchant_advice_code(&self) -> Option<String> {
        self.find_in_history(|event| {
            event
                .merchant_advice_code_response
                .as_ref()
                .and_then(|mac| mac.code.clone())
        })
    }

    pub fn merchant_advice_message(&self) -> Option<String> {
        self.find_in_history(|event| {
            event
                .merchant_advice_code_response
                .as_ref()
                .and_then(|mac| mac.message.clone())
        })
    }

    /// The raw card-network response code. Supplemental to the processor response code —
    /// never the source of truth, and never something to branch retry logic on.
    pub fn network_code(&self) -> Option<String> {
        self.find_in_history(|event| {
            event
                .network_response
                .as_ref()
                .and_then(|network| network.code.clone())
        })
    }

    pub fn network_message(&self) -> Option<String> {
        self.find_in_history(|event| {
            event
                .network_response
                .as_ref()
                .and_then(|network| network.message.clone())
        })
    }

    pub fn decline_type(&self) -> Option<ProcessorDeclineType> {
        self.find_in_history(|event| event.decline_type)
    }

    pub fn gateway_rejection_reason(&self) -> Option<GatewayRejectionReason> {
        self.find_in_history(|event| event.gateway_rejection_reason)
    }

    pub fn risk_decision(&self) -> Option<RiskDecision> {
        self.find_in_history(|event| event.risk_decision)
    }

    /// Whether Braintree itself considers the current status final. Preferred over
    /// inferring terminality from the status enum, though the status map stays the
    /// authority for which UCS state to report.
    pub fn is_terminal(&self) -> Option<bool> {
        self.status_history
            .as_ref()
            .and_then(|history| history.first())
            .and_then(|event| event.terminal)
    }
}

/// The AVS / CVV outcome, the acquirer authorization code and the risk decision, in the
/// shape UCS's `AdditionalPaymentMethodConnectorResponse::Card` expects. The `payment_checks`
/// keys mirror what the Cybersource and Bank of America connectors already emit so that
/// downstream consumers see one vocabulary, and the values are the single-letter REST codes
/// those consumers speak.
///
/// Returns `None` when the response carried nothing worth reporting, so a connector that has
/// not widened its selection set does not emit an empty bag.
fn build_card_connector_response(
    surface: &TransactionResponseSurface,
) -> Option<domain_types::router_data::ConnectorResponseData> {
    let processor = surface.processor_authorization_response.as_ref();

    let mut payment_checks = serde_json::Map::new();
    let mut insert_check = |key: &str, value: Option<AvsCvvResponseCode>| {
        if let Some(code) = value {
            payment_checks.insert(
                key.to_string(),
                // Fall back to the GraphQL spelling when the value is outside the SDL enum
                // and therefore has no REST letter.
                serde_json::json!(code
                    .as_rest_code()
                    .map_or_else(|| code.to_string(), str::to_string)),
            );
        }
    };
    insert_check(
        "avs_postal_code_response",
        processor.and_then(|p| p.avs_postal_code_response),
    );
    insert_check(
        "avs_street_address_response",
        processor.and_then(|p| p.avs_street_address_response),
    );
    insert_check("card_verification", processor.and_then(|p| p.cvv_response));
    if let Some(risk_decision) = surface.risk_decision() {
        payment_checks.insert(
            "risk_decision".to_string(),
            serde_json::json!(risk_decision.to_string()),
        );
    }

    let auth_code = processor.and_then(|p| p.authorization_id.clone());

    if payment_checks.is_empty() && auth_code.is_none() {
        return None;
    }

    Some(
        domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: (!payment_checks.is_empty())
                    .then(|| serde_json::Value::Object(payment_checks)),
                card_network: None,
                domestic_network: None,
                auth_code,
            },
        ),
    )
}

/// The processor's own code / text for a transaction Braintree accepted, so the caller can
/// see `1000 / Approved` on a success as well as `2001 / Insufficient Funds` on a decline.
fn build_raw_connector_status(
    surface: &TransactionResponseSurface,
) -> Option<connector_types::RawConnectorStatus> {
    let processor = surface.processor_authorization_response.as_ref()?;
    if processor.legacy_code.is_none() && processor.message.is_none() {
        return None;
    }
    Some(connector_types::RawConnectorStatus {
        code: processor.legacy_code.clone(),
        message: processor.message.clone(),
        reason: processor.additional_information.clone(),
    })
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

impl BraintreePaymentStatus {
    /// The status exactly as Braintree spells it on the wire, e.g. `PROCESSOR_DECLINED`.
    ///
    /// The derived `strum::Display` renders the *Rust* variant name (`ProcessorDeclined`),
    /// which is not a value the gateway ever sends. When a status has to stand in as an
    /// error code — a refusal that carried no processor code at all — it must be the value
    /// the gateway actually used, so the code is greppable against Braintree's own logs and
    /// survives a rename of the Rust variant. Matched exhaustively so a new status cannot
    /// silently inherit another one's spelling.
    pub fn wire_value(&self) -> &'static str {
        match self {
            Self::Authorized => "AUTHORIZED",
            Self::Authorizing => "AUTHORIZING",
            Self::AuthorizationExpired => "AUTHORIZATION_EXPIRED",
            Self::Failed => "FAILED",
            Self::ProcessorDeclined => "PROCESSOR_DECLINED",
            Self::GatewayRejected => "GATEWAY_REJECTED",
            Self::Voided => "VOIDED",
            Self::Settling => "SETTLING",
            Self::Settled => "SETTLED",
            Self::SettlementPending => "SETTLEMENT_PENDING",
            Self::SettlementDeclined => "SETTLEMENT_DECLINED",
            Self::SettlementConfirmed => "SETTLEMENT_CONFIRMED",
            Self::SubmittedForSettlement => "SUBMITTED_FOR_SETTLEMENT",
        }
    }

    /// The values that are a terminal refusal whichever resource carries them — the shared
    /// `PaymentStatus` enum types both `Transaction.status` and `Refund.status`.
    pub fn is_terminal_failure(&self) -> bool {
        matches!(
            self,
            Self::Failed
                | Self::GatewayRejected
                | Self::ProcessorDeclined
                | Self::SettlementDeclined
                | Self::AuthorizationExpired
        )
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
            BraintreePaymentStatus::AuthorizationExpired => Self::AuthorizationFailed,
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
                let surface = &transaction_data.response_surface;
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_declined_error_response(
                        &transaction_data.status,
                        surface,
                        Some(transaction_data.id.clone()),
                        status,
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
                        // AVS / CVV outcome and the acquirer auth code travel on the
                        // success path too — an approved transaction can still carry an
                        // AVS mismatch when the merchant has no AVS rule enabled.
                        connector_response: build_card_connector_response(surface),
                        raw_connector_status: build_raw_connector_status(surface),
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
/// `Refund.status` is typed `PaymentStatus` in the Braintree GraphQL schema — the *same*
/// 13-member enum as `Transaction.status`, not a refund-specific one. The five
/// settlement-side values below are the happy path, but a refund can legitimately come back
/// `SETTLEMENT_DECLINED`, `GATEWAY_REJECTED`, `PROCESSOR_DECLINED` or `VOIDED` (the last when
/// the refund itself was reversed with `reverseRefund`), and an `AUTHORIZATION_EXPIRED` /
/// `AUTHORIZED` / `AUTHORIZING` / `SETTLEMENT_CONFIRMED` value is reachable through the
/// shared enum. Every member is modelled so the response deserializes; `Unknown` catches a
/// value added upstream after this was written.
pub enum BraintreeRefundStatus {
    SettlementPending,
    Settling,
    Settled,
    SubmittedForSettlement,
    SettlementConfirmed,
    Authorized,
    Authorizing,
    Failed,
    GatewayRejected,
    ProcessorDeclined,
    SettlementDeclined,
    AuthorizationExpired,
    Voided,
    #[serde(other)]
    Unknown,
}

impl From<BraintreeRefundStatus> for enums::RefundStatus {
    fn from(item: BraintreeRefundStatus) -> Self {
        match item {
            BraintreeRefundStatus::Settled
            | BraintreeRefundStatus::Settling
            | BraintreeRefundStatus::SubmittedForSettlement
            | BraintreeRefundStatus::SettlementPending => Self::Success,
            // Reachable only through the shared PaymentStatus enum: the refund has been
            // accepted but has not reached a settlement state yet, so keep syncing.
            BraintreeRefundStatus::SettlementConfirmed
            | BraintreeRefundStatus::Authorized
            | BraintreeRefundStatus::Authorizing => Self::Pending,
            // Terminal refusals. A terminal connector state must map to a terminal UCS state,
            // otherwise the refund polls forever.
            BraintreeRefundStatus::Failed
            | BraintreeRefundStatus::GatewayRejected
            | BraintreeRefundStatus::ProcessorDeclined
            | BraintreeRefundStatus::SettlementDeclined
            | BraintreeRefundStatus::AuthorizationExpired
            | BraintreeRefundStatus::Voided => Self::Failure,
            // An unrecognised value must not be guessed into success or failure.
            BraintreeRefundStatus::Unknown => Self::Unknown,
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
#[serde(rename_all = "camelCase")]
pub struct CaptureResponseTransactionBody {
    id: String,
    status: BraintreePaymentStatus,
    /// A capture repeats the authorization's AVS / CVV values — no new check happens at
    /// capture time — but the 4000-class settlement response is only ever populated here.
    #[serde(flatten)]
    response_surface: TransactionResponseSurface,
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
                let surface = &transaction_data.response_surface;
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_declined_error_response(
                        &transaction_data.status,
                        surface,
                        Some(transaction_data.id.clone()),
                        status,
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
                        // No new AVS/CVV check happens at capture; these values repeat the
                        // authorization's, which is still worth surfacing so a capture-only
                        // caller sees them.
                        connector_response: build_card_connector_response(surface),
                        raw_connector_status: build_raw_connector_status(surface),
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

/// Which arm of the `TransactionReversal = Refund | Transaction` union came back, read from
/// the `__typename` GraphQL meta-field. An unsettled transaction is voided (`Transaction`);
/// a settled one is reversed by a new, always-full-amount refund (`Refund`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ReversalKind {
    Transaction,
    Refund,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CancelResponseTransactionBody {
    #[serde(rename = "__typename")]
    typename: Option<ReversalKind>,
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
                // On the `Refund` arm the transaction had already settled, so Braintree
                // reversed it with a new full-amount refund rather than a void. The money is
                // on its way back either way, so the attempt is Voided; the refund's own id
                // is surfaced as the response reference id so the new resource is traceable.
                // Its `status` is a refund status and must not be read through the payment
                // status map, which would report `SUBMITTED_FOR_SETTLEMENT` as `Charged`.
                let is_refund_arm = void_data.typename == Some(ReversalKind::Refund);
                let status = if is_refund_arm {
                    if void_data.status.is_terminal_failure() {
                        enums::AttemptStatus::VoidFailed
                    } else {
                        enums::AttemptStatus::Voided
                    }
                } else {
                    enums::AttemptStatus::from(void_data.status.clone())
                };
                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(create_failure_error_response(
                        void_data.status,
                        Some(void_data.id),
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
                        connector_response_reference_id: is_refund_arm.then_some(void_data.id),
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
        let three_ds_data =
            item.router_data
                .request
                .authentication_data
                .as_ref()
                .map(|auth_data| ThreeDSecureAuthenticationInput {
                    pass_through: Some(convert_external_three_ds_data(auth_data)),
                });

        // `options.billingAddress` is the only enrichment field that does NOT live on
        // `TransactionInput`; see `CreditCardTransactionOptions`.
        let billing_address = item
            .router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing| {
                build_address_input(billing.address.as_ref(), billing.phone.as_ref())
            });
        let options = (three_ds_data.is_some() || billing_address.is_some()).then_some(
            CreditCardTransactionOptions {
                three_d_secure_authentication: three_ds_data,
                billing_address,
            },
        );
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
        let enrichment = TransactionEnrichment::build(EnrichmentInputs {
            amount_converter: item.connector.amount_converter,
            l2_l3_data: item.router_data.resource_common_data.l2_l3_data.as_deref(),
            order_details: item.router_data.resource_common_data.order_details.as_ref(),
            shipping: item
                .router_data
                .resource_common_data
                .get_optional_shipping(),
            billing_descriptor: item.router_data.request.billing_descriptor.as_ref(),
            surcharge_amount: item
                .router_data
                .request
                .surcharge_amount
                .as_ref()
                .map(|surcharge| surcharge.amount),
            shipping_cost: item.router_data.request.shipping_cost,
            amount: item.router_data.request.minor_amount,
            currency: item.router_data.request.currency,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure_authentication: Option<ThreeDSecureAuthenticationInput>,
    /// `billingAddress` sits on `CreditCardTransactionOptionsInput`, **not** on
    /// `TransactionInput`. `ChargePaymentMethodInput` / `AuthorizePaymentMethodInput` (the
    /// wallet and MIT mutations) have no `options` member at all, so a per-attempt billing
    /// address can only ever be sent on the credit-card mutations; elsewhere AVS runs against
    /// the address stored on the vaulted payment method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<BraintreeAddressInput>,
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
    pub eci_flag: Option<String>,
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

fn convert_external_three_ds_data(
    auth_data: &router_request_types::AuthenticationData,
) -> ThreeDSecurePassThroughInput {
    ThreeDSecurePassThroughInput {
        eci_flag: auth_data.eci.clone(),
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
    }
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
    /// A manual-capture MIT is sent as `authorizeCreditCard`, so the payload is keyed
    /// `data.authorizeCreditCard`, not `data.chargeCreditCard`. Without this arm the
    /// response of every manual-capture repeat payment failed to deserialize even though
    /// the authorization had succeeded at the gateway.
    AuthResponse(Box<AuthResponse>),
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
        // Auto-capture MITs go through `chargeCreditCard` and manual-capture MITs through
        // `authorizeCreditCard`; the two payloads differ only in the key under `data` and
        // carry an identical transaction body.
        let transaction_data = match item.response {
            BraintreeRepeatPaymentResponse::ErrorResponse(error_response) => {
                return Ok(Self {
                    response: build_error_response(&error_response.errors, item.http_code)
                        .map_err(|err| *err),
                    ..item.router_data
                })
            }
            BraintreeRepeatPaymentResponse::PaymentsResponse(payment_response) => {
                payment_response.data.charge_credit_card.transaction
            }
            BraintreeRepeatPaymentResponse::AuthResponse(auth_response) => {
                auth_response.data.authorize_credit_card.transaction
            }
        };
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
    #[serde(rename = "__typename")]
    typename: Option<ReversalKind>,
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
                // Post-capture is precisely the case where `reverseTransaction` answers on the
                // `Refund` arm of the union: the capture has settled, so Braintree issues a
                // new full-amount refund whose `status` is a refund status. Reading it through
                // the transaction table below would call `SUBMITTED_FOR_SETTLEMENT` "Pending"
                // and keep polling a reversal that has already been accepted.
                let post_capture_void_status =
                    if reversal_data.typename == Some(ReversalKind::Refund) {
                        if reversal_data.status.is_terminal_failure() {
                            common_enums::PostCaptureVoidStatus::Failed
                        } else {
                            common_enums::PostCaptureVoidStatus::Succeeded
                        }
                    } else {
                        match reversal_data.status {
                            BraintreePaymentStatus::Voided => {
                                common_enums::PostCaptureVoidStatus::Succeeded
                            }
                            BraintreePaymentStatus::Failed
                            | BraintreePaymentStatus::GatewayRejected
                            | BraintreePaymentStatus::ProcessorDeclined
                            | BraintreePaymentStatus::SettlementDeclined
                            | BraintreePaymentStatus::AuthorizationExpired => {
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

// ---------------------------------------------------------------------------------------
// Request-side data enrichment for the Braintree GraphQL transaction input
// ---------------------------------------------------------------------------------------
//
// Everything below decorates `TransactionInput` (and, for the billing address only,
// `CreditCardTransactionOptionsInput`) beyond `amount` / `paymentMethodId`. Field names,
// types and nullability come from the Braintree SDL
// (`grace/rulesbook/codegen/references/braintree/source_10.md`).
//
// Placement is the part that is easy to get wrong, because the fields are split across two
// sibling objects and the split is not the intuitive one:
//   * `billingAddress`  lives ONLY on `CreditCardTransactionOptionsInput` (`input.options`).
//   * `shipping`, `tax`, `descriptor`, `lineItems`, `purchaseOrderNumber`, `discountAmount`,
//     `surchargeAmount` and `merchantAccountId` live on `TransactionInput`.
//   * `ChargePaymentMethodInput` / `AuthorizePaymentMethodInput` (the wallet mutations) have
//     no `options` member at all, so a per-attempt billing address cannot be sent on the
//     wallet path — for those, AVS runs against whatever address is on the vaulted payment
//     method.
//   * `merchantAccountId` is NOT accepted on either capture input; a capture always settles
//     against the authorization's merchant account.
//
// Every field is `Option` + `skip_serializing_if`, so a request carrying no enrichment data
// serialises byte-for-byte as it did before this change.

/// `lineItems[].name` — "Maximum 35 characters".
const L3_NAME_MAX_LEN: usize = 35;
/// `lineItems[].unitOfMeasure` / `.productCode` / `.commodityCode` — "Maximum 12 characters".
const L3_CODE_MAX_LEN: usize = 12;
/// `lineItems[].description` — "Item description. Maximum 127 characters".
const L3_DESCRIPTION_MAX_LEN: usize = 127;
/// `purchaseOrderNumber` — "Up to 12 ASCII characters for AIB and 17 ASCII characters for all
/// other processors."
const PURCHASE_ORDER_NUMBER_MAX_LEN: usize = 17;
/// `TransactionInput.lineItems` — "Up to 249 line items may be specified."
const MAX_LINE_ITEMS: usize = 249;

/// Level 3 free-text fields (`lineItems[].name`, `.unitOfMeasure`, `.productCode`,
/// `.commodityCode`) are documented as accepting only `a-z`, `A-Z`, `0-9`, `'`, `.`, `-` and
/// spaces. Merchant-supplied product text routinely carries `&`, `/`, commas and non-ASCII,
/// so sanitise first and truncate second — truncating first would spend the character budget
/// on characters that are about to be removed.
fn sanitize_l3_text(value: &str, max_len: usize) -> Option<String> {
    let sanitized = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '\'' | '.' | '-'))
        .collect::<String>();
    let truncated = sanitized
        .trim()
        .chars()
        .take(max_len)
        .collect::<String>()
        .trim_end()
        .to_string();
    (!truncated.is_empty()).then_some(truncated)
}

/// Plain character-count truncation for the fields with a length limit but no documented
/// charset restriction (`lineItems[].description`).
fn truncate_text(value: &str, max_len: usize) -> Option<String> {
    let truncated = value.chars().take(max_len).collect::<String>();
    (!truncated.is_empty()).then_some(truncated)
}

/// `purchaseOrderNumber` is documented as ASCII only. Take the 17-character limit that
/// applies to every processor other than AIB.
fn sanitize_purchase_order_number(value: &str) -> Option<String> {
    let sanitized = value
        .chars()
        .filter(char::is_ascii_graphic)
        .take(PURCHASE_ORDER_NUMBER_MAX_LEN)
        .collect::<String>();
    (!sanitized.is_empty()).then_some(sanitized)
}

/// `AddressInput.countryCode` is `scalar CountryCode`, whose wire format is chosen by the
/// `Braintree-Version` request header rather than by the field:
///   * `Braintree-Version >= 2021-02-01` → ISO 3166-1 **alpha-2** (`US`)
///   * `Braintree-Version <  2021-02-01` → ISO 3166-1 **alpha-3** (`USA`)
///
/// UCS pins `BRAINTREE_VERSION_VALUE = "2019-01-01"` (see `braintree.rs`), which is before
/// the cutoff, so every country has to be widened from the alpha-2 UCS stores to alpha-3
/// here. Emitting the raw alpha-2 under the pinned version is a silent-wrong-value bug:
/// Braintree either rejects the country outright or quietly degrades AVS and Level 3
/// qualification. Bumping the version header instead would change schema behaviour for every
/// Braintree flow and is deliberately out of scope (tech spec UNDECIDED #8, option (a)).
fn to_braintree_country_code(country: enums::CountryAlpha2) -> enums::CountryAlpha3 {
    enums::CountryAlpha2::from_alpha2_to_alpha3(country)
}

/// Every money field on these inputs is a **major-unit decimal string**, while every UCS
/// Level 2/3 amount is `MinorUnit`. The conversion always goes through the connector's own
/// `amount_converter` (`StringMajorUnit`, declared in `braintree.rs`) — never
/// `MinorUnit::to_string()`, which would send `"1234"` where `"12.34"` was meant, a 100x
/// overcharge Braintree cannot detect because `"1234"` is itself a valid `Amount`.
fn convert_major(
    amount_converter: &(dyn AmountConvertor<Output = StringMajorUnit> + Sync),
    amount: MinorUnit,
    currency: enums::Currency,
) -> Result<StringMajorUnit, Report<IntegrationError>> {
    amount_converter.convert(amount, currency).change_context(
        IntegrationError::AmountConversionFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Check that the Level 2/3 amount and the payment currency are consistent"
                        .to_string(),
                ),
                doc_url: None,
                additional_context: Some(
                    "Braintree requires every Level 2/3 amount as a major-unit decimal string"
                        .to_string(),
                ),
            },
        },
    )
}

fn convert_optional_major(
    amount_converter: &(dyn AmountConvertor<Output = StringMajorUnit> + Sync),
    amount: Option<MinorUnit>,
    currency: enums::Currency,
) -> Result<Option<StringMajorUnit>, Report<IntegrationError>> {
    amount
        .map(|amount| convert_major(amount_converter, amount, currency))
        .transpose()
}

/// `input PhoneInput` — both members are non-null in the SDL, so the pair is all-or-nothing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BraintreePhoneInput {
    country_phone_code: Secret<String>,
    phone_number: Secret<String>,
}

/// `input AddressInput`. Every member is optional.
///
/// The SDL carries four alias pairs for the same concepts — `streetAddress`/`addressLine1`,
/// `extendedAddress`/`addressLine2`, `locality`/`adminArea2`, `region`/`adminArea1` — and it
/// does not say what happens when both members of a pair are sent. This struct commits to the
/// legacy set and never emits the PayPal-style aliases.
///
/// `AddressDetails.line3` has no counterpart on `AddressInput` (there is no line-3 field) and
/// is dropped; email is not an address field on Braintree and is carried on
/// `TransactionInput.customerDetails` instead.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    country_code: Option<enums::CountryAlpha3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<BraintreePhoneInput>,
}

impl BraintreeAddressInput {
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

/// Builds an `AddressInput` from the UCS address pair. Returns `None` rather than an empty
/// object, because Braintree distinguishes "absent" from "present and empty".
fn build_address_input(
    address: Option<&AddressDetails>,
    phone: Option<&PhoneDetails>,
) -> Option<BraintreeAddressInput> {
    let phone = phone.and_then(|phone| {
        match (phone.number.clone(), phone.country_code.clone()) {
            (Some(number), Some(country_code)) => Some(BraintreePhoneInput {
                // Braintree wants the bare E.164 calling code; UCS stores it with the
                // leading `+`.
                country_phone_code: Secret::new(country_code.trim_start_matches('+').to_string()),
                phone_number: number,
            }),
            // `countryPhoneCode` and `phoneNumber` are both non-null, so a half-populated
            // phone is dropped instead of being partially sent.
            _ => None,
        }
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
            .map(to_braintree_country_code),
        phone,
    };
    (!built.is_empty()).then_some(built)
}

/// `input TransactionShippingInput`.
///
/// `shippingMethod` is deliberately absent: the SDL enum `TransactionShippingMethod` has
/// exactly seven members and no `OTHER`/`UNKNOWN` fallback, and UCS has no field that maps
/// onto it, so there is nothing to send and nothing to coerce.
#[derive(Debug, Clone, Default, Serialize)]
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

impl TransactionShippingInput {
    fn is_empty(&self) -> bool {
        self.shipping_address.is_none()
            && self.shipping_amount.is_none()
            && self.shipping_tax_amount.is_none()
            && self.ships_from_postal_code.is_none()
    }
}

/// `input TransactionTaxInput` — two members, and the amount is named `taxAmount`, not
/// `amount`. `taxAmount` is required for Level 2 unless `taxExempt` is true.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionTaxInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    tax_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax_exempt: Option<bool>,
}

impl TransactionTaxInput {
    fn is_empty(&self) -> bool {
        self.tax_amount.is_none() && self.tax_exempt.is_none()
    }
}

/// `input TransactionDescriptorInput`.
///
/// `url` is absent because `BillingDescriptor` has no URL field — there is no UCS source, and
/// inventing one would trip validation code 92206 (`url` must be 13 characters or shorter).
/// `BillingDescriptor.city`, `.statement_descriptor`, `.statement_descriptor_suffix` and
/// `.reference` likewise have no counterpart on this input and are dropped (tech spec
/// UNDECIDED #10, option (a)). The two mapped values are passed through verbatim: Braintree's
/// documented descriptor form is `<company prefix>*<product descriptor>`, so a
/// letters-and-digits sanitiser would destroy the separator; format violations come back as
/// validation codes 92201 / 92204 and are surfaced to the caller.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDescriptorInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<Secret<String>>,
}

/// `enum TransactionLineItemType`. The SDL members are `DEBIT` and `CREDIT`; a sale line is
/// `DEBIT` (validation code 97308 fires on a sale carrying `CREDIT`). Only the sale direction
/// is ever produced here, so `CREDIT` is not modelled.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionLineItemType {
    Debit,
}

/// `input TransactionLineItemInput`. `name`, `kind`, `quantity`, `unitAmount` and
/// `totalAmount` are non-null.
///
/// `unitTaxAmount` and `upc` are deliberately unmapped: UCS has no per-unit tax field (and
/// deriving one by division would break the reconciliation rule through rounding), and
/// `LineItemUpcInput.upcType` is non-null with no UCS source, so a UPC cannot be sent from
/// domain data alone.
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
}

/// Builds one `TransactionLineItemInput`, returning the line total in `MinorUnit` alongside it
/// so the caller can run the Level 3 reconciliation on exact integers.
///
/// `Ok(None)` means the line cannot be represented (its name did not survive Level 3
/// sanitisation) and the whole `lineItems` array must be dropped — a Level 3 line item without
/// a `name` is not sendable, and inventing a placeholder name would be fabricating merchant
/// data.
fn build_line_item(
    amount_converter: &(dyn AmountConvertor<Output = StringMajorUnit> + Sync),
    detail: &OrderDetailsWithAmount,
    currency: enums::Currency,
) -> Result<Option<(TransactionLineItemInput, MinorUnit)>, Report<IntegrationError>> {
    let Some(name) = sanitize_l3_text(&detail.product_name, L3_NAME_MAX_LEN) else {
        return Ok(None);
    };
    let quantity = i64::from(detail.quantity);
    // `totalAmount` is `String!` — non-null and required for Level 3 — but the gRPC proto
    // carries no `total_amount` field for a line item and the proto -> domain conversion
    // hardcodes `total_amount: None`, so it has to be derived whenever the caller does not
    // supply it. Derive it in MinorUnit *before* the major-unit conversion: multiplying an
    // already-rounded decimal string would reintroduce the rounding error the reconciliation
    // rule exists to catch.
    let total_amount_minor = match detail.total_amount {
        Some(total_amount) => total_amount,
        None => MinorUnit::new(
            detail
                .amount
                .get_amount_as_i64()
                .checked_mul(quantity)
                .ok_or_else(|| IntegrationError::InvalidDataFormat {
                    field_name: "order_details.amount",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Lower order_details.quantity or order_details.amount, or send order_details.total_amount explicitly"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Braintree requires lineItems[].totalAmount; UCS derives it as quantity x unit amount when the caller omits order_details.total_amount, and the product overflowed a 64-bit minor-unit amount"
                                .to_string(),
                        ),
                    },
                })?,
        ),
    };
    let line_item = TransactionLineItemInput {
        name,
        kind: TransactionLineItemType::Debit,
        quantity: quantity.to_string(),
        unit_amount: convert_major(amount_converter, detail.amount, currency)?,
        total_amount: convert_major(amount_converter, total_amount_minor, currency)?,
        tax_amount: convert_optional_major(amount_converter, detail.total_tax_amount, currency)?,
        // UCS calls this a *unit* discount while Braintree's line-item `discountAmount` is the
        // discount on the whole line; the two coincide at quantity 1. The value is passed
        // through unscaled rather than multiplied by quantity, because scaling it would be an
        // assumption the caller never made (tech spec UNDECIDED #12).
        discount_amount: convert_optional_major(
            amount_converter,
            detail.unit_discount_amount,
            currency,
        )?,
        unit_of_measure: detail
            .unit_of_measure
            .as_deref()
            .and_then(|value| sanitize_l3_text(value, L3_CODE_MAX_LEN)),
        product_code: detail
            .product_id
            .as_deref()
            .or(detail.sku.as_deref())
            .or(detail.upc.as_deref())
            .and_then(|value| sanitize_l3_text(value, L3_CODE_MAX_LEN)),
        commodity_code: detail
            .commodity_code
            .as_deref()
            .and_then(|value| sanitize_l3_text(value, L3_CODE_MAX_LEN)),
        description: detail
            .description
            .as_deref()
            .and_then(|value| truncate_text(value, L3_DESCRIPTION_MAX_LEN)),
    };
    Ok(Some((line_item, total_amount_minor)))
}

/// The enrichment half of `input TransactionInput`, flattened into each transaction body so
/// the existing, live-validated fields keep their exact serialised shape.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionEnrichment {
    #[serde(skip_serializing_if = "Option::is_none")]
    purchase_order_number: Option<String>,
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

/// Everything `TransactionEnrichment::build` reads, gathered so the builder stays a pure
/// function of domain data and can be unit-tested without a `RouterDataV2`.
pub struct EnrichmentInputs<'a> {
    /// The connector's own `StringMajorUnit` converter, threaded in so this stays a pure
    /// function of domain data and stays unit-testable.
    pub amount_converter: &'a (dyn AmountConvertor<Output = StringMajorUnit> + Sync),
    pub l2_l3_data: Option<&'a L2L3Data>,
    pub order_details: Option<&'a Vec<OrderDetailsWithAmount>>,
    pub shipping: Option<&'a Address>,
    pub billing_descriptor: Option<&'a BillingDescriptor>,
    pub surcharge_amount: Option<MinorUnit>,
    pub shipping_cost: Option<MinorUnit>,
    pub amount: MinorUnit,
    pub currency: enums::Currency,
}

impl TransactionEnrichment {
    fn build(inputs: EnrichmentInputs<'_>) -> Result<Self, Report<IntegrationError>> {
        let EnrichmentInputs {
            amount_converter,
            l2_l3_data,
            order_details,
            shipping,
            billing_descriptor,
            surcharge_amount,
            shipping_cost,
            amount,
            currency,
        } = inputs;

        let merchant_order_reference_id =
            l2_l3_data.and_then(L2L3Data::get_merchant_order_reference_id);
        let purchase_order_number = merchant_order_reference_id
            .as_deref()
            .and_then(sanitize_purchase_order_number);

        let descriptor = billing_descriptor.and_then(|billing_descriptor| {
            let descriptor = TransactionDescriptorInput {
                name: billing_descriptor.name.clone(),
                phone: billing_descriptor.phone.clone(),
            };
            // Never send `descriptor: {}` — an empty object can trip validation code 92204.
            (descriptor.name.is_some() || descriptor.phone.is_some()).then_some(descriptor)
        });

        let tax_amount_minor = l2_l3_data.and_then(L2L3Data::get_order_tax_amount);
        let tax_exempt = l2_l3_data
            .and_then(L2L3Data::get_tax_status)
            .map(|tax_status| matches!(tax_status, enums::TaxStatus::Exempt));
        let tax = {
            let tax = TransactionTaxInput {
                tax_amount: convert_optional_major(amount_converter, tax_amount_minor, currency)?,
                tax_exempt,
            };
            (!tax.is_empty()).then_some(tax)
        };

        let discount_amount_minor = l2_l3_data.and_then(L2L3Data::get_discount_amount);

        let l2_l3_shipping_details = l2_l3_data.and_then(|data| data.shipping_details.clone());
        let shipping_address_details = shipping
            .and_then(|shipping| shipping.address.clone())
            .or(l2_l3_shipping_details);
        let shipping_amount_minor = l2_l3_data
            .and_then(L2L3Data::get_shipping_cost)
            .or(shipping_cost);
        let shipping_tax_amount_minor = l2_l3_data.and_then(L2L3Data::get_shipping_amount_tax);
        let shipping_block = {
            let shipping_block = TransactionShippingInput {
                shipping_address: build_address_input(
                    shipping_address_details.as_ref(),
                    shipping.and_then(|shipping| shipping.phone.as_ref()),
                ),
                shipping_amount: convert_optional_major(
                    amount_converter,
                    shipping_amount_minor,
                    currency,
                )?,
                shipping_tax_amount: convert_optional_major(
                    amount_converter,
                    shipping_tax_amount_minor,
                    currency,
                )?,
                // `origin_zip` is the only UCS field that models the *source* postcode.
                ships_from_postal_code: l2_l3_data
                    .and_then(L2L3Data::get_shipping_origin_zip)
                    .or_else(|| {
                        shipping_address_details
                            .as_ref()
                            .and_then(|details| details.origin_zip.clone())
                    }),
            };
            (!shipping_block.is_empty()).then_some(shipping_block)
        };

        let order_details = l2_l3_data
            .and_then(L2L3Data::get_order_details)
            .or_else(|| order_details.cloned());
        let (line_items, line_items_total_minor) = match order_details {
            Some(order_details) if !order_details.is_empty() => {
                let mut items = Vec::new();
                let mut total = 0_i64;
                let mut usable = true;
                for detail in order_details.iter().take(MAX_LINE_ITEMS) {
                    match build_line_item(amount_converter, detail, currency)? {
                        Some((line_item, line_total)) => {
                            total = total.saturating_add(line_total.get_amount_as_i64());
                            items.push(line_item);
                        }
                        None => {
                            usable = false;
                            break;
                        }
                    }
                }
                if usable && !items.is_empty() {
                    (Some(items), Some(total))
                } else {
                    warn!(
                        "BRAINTREE: dropping lineItems - a product name did not survive Level 3 sanitisation"
                    );
                    (None, None)
                }
            }
            // Never send `lineItems: []` — an empty array is not the same as omitting it.
            _ => (None, None),
        };

        let mut enrichment = Self {
            purchase_order_number,
            descriptor,
            tax,
            discount_amount: convert_optional_major(
                amount_converter,
                discount_amount_minor,
                currency,
            )?,
            surcharge_amount: convert_optional_major(amount_converter, surcharge_amount, currency)?,
            shipping: shipping_block,
            line_items,
        };

        // Level 3 reconciliation rule. Braintree rejects — or silently drops to Level 1 — a
        // transaction whose breakdown does not balance:
        //
        //   amount = SUM(lineItem.totalAmount) + tax.taxAmount + shipping.shippingAmount
        //            + shipping.shippingTaxAmount - discountAmount
        //
        // The breakdown *describes* `amount`; it never adds to what the payment method is
        // charged, so `amount` is never recomputed from it. `surchargeAmount` is not part of
        // the formula. The check runs on exact `MinorUnit` integers, and only when line items
        // are present: without them there is no sum to reconcile and a bare Level 2
        // `tax.taxAmount` is legitimate on its own. When it does not balance the monetary
        // breakdown is dropped rather than sent unbalanced, keeping the non-monetary Level
        // 2/3 fields (purchase order number, addresses, ships-from postcode, descriptor).
        if let Some(line_items_total_minor) = line_items_total_minor {
            let reconciled = line_items_total_minor
                .saturating_add(tax_amount_minor.map_or(0, MinorUnit::get_amount_as_i64))
                .saturating_add(shipping_amount_minor.map_or(0, MinorUnit::get_amount_as_i64))
                .saturating_add(shipping_tax_amount_minor.map_or(0, MinorUnit::get_amount_as_i64))
                .saturating_sub(discount_amount_minor.map_or(0, MinorUnit::get_amount_as_i64));
            if reconciled != amount.get_amount_as_i64() {
                warn!(
                    reconciled_total = reconciled,
                    transaction_amount = amount.get_amount_as_i64(),
                    "BRAINTREE: Level 2/3 breakdown does not reconcile with the transaction amount - omitting the monetary breakdown"
                );
                enrichment.line_items = None;
                enrichment.discount_amount = None;
                if let Some(tax) = enrichment.tax.as_mut() {
                    tax.tax_amount = None;
                }
                if let Some(shipping) = enrichment.shipping.as_mut() {
                    shipping.shipping_amount = None;
                    shipping.shipping_tax_amount = None;
                }
                enrichment.tax = enrichment.tax.filter(|tax| !tax.is_empty());
                enrichment.shipping = enrichment.shipping.filter(|shipping| !shipping.is_empty());
            }
        }

        Ok(enrichment)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// Every member of Braintree's `PaymentStatus` enum, verbatim from the GraphQL SDL
    /// (`query { __type(name: "PaymentStatus") { enumValues { name } } }`). The wire
    /// spelling is the contract: `AUTHORIZATION_EXPIRED`, not `AUTHORIZED_EXPIRED`.
    const PAYMENT_STATUS_SDL_VALUES: [&str; 13] = [
        "AUTHORIZATION_EXPIRED",
        "AUTHORIZED",
        "AUTHORIZING",
        "FAILED",
        "GATEWAY_REJECTED",
        "PROCESSOR_DECLINED",
        "SETTLED",
        "SETTLEMENT_CONFIRMED",
        "SETTLEMENT_DECLINED",
        "SETTLEMENT_PENDING",
        "SETTLING",
        "SUBMITTED_FOR_SETTLEMENT",
        "VOIDED",
    ];

    fn payment_status(value: &str) -> BraintreePaymentStatus {
        serde_json::from_value(serde_json::Value::String(value.to_string()))
            .expect("Braintree PaymentStatus value must deserialize")
    }

    fn refund_status(value: &str) -> BraintreeRefundStatus {
        serde_json::from_value(serde_json::Value::String(value.to_string()))
            .expect("Braintree Refund status value must deserialize")
    }

    #[test]
    fn every_sdl_payment_status_deserializes() {
        for value in PAYMENT_STATUS_SDL_VALUES {
            let _ = payment_status(value);
        }
    }

    /// Regression guard for the variant rename: the schema value is
    /// `AUTHORIZATION_EXPIRED` and it must map to a terminal authorization failure, while
    /// the old misspelling must not be accepted as if it were a real Braintree value.
    #[test]
    fn authorization_expired_maps_to_authorization_failed() {
        assert!(matches!(
            payment_status("AUTHORIZATION_EXPIRED"),
            BraintreePaymentStatus::AuthorizationExpired
        ));
        assert_eq!(
            enums::AttemptStatus::from(payment_status("AUTHORIZATION_EXPIRED")),
            enums::AttemptStatus::AuthorizationFailed
        );
        assert!(
            serde_json::from_value::<BraintreePaymentStatus>(serde_json::json!(
                "AUTHORIZED_EXPIRED"
            ))
            .is_err()
        );
    }

    #[test]
    fn payment_status_attempt_status_map() {
        for (value, expected) in [
            ("AUTHORIZED", enums::AttemptStatus::Authorized),
            ("AUTHORIZING", enums::AttemptStatus::Authorizing),
            ("SUBMITTED_FOR_SETTLEMENT", enums::AttemptStatus::Charged),
            ("SETTLING", enums::AttemptStatus::Charged),
            ("SETTLED", enums::AttemptStatus::Charged),
            ("SETTLEMENT_CONFIRMED", enums::AttemptStatus::Charged),
            ("SETTLEMENT_PENDING", enums::AttemptStatus::Charged),
            ("SETTLEMENT_DECLINED", enums::AttemptStatus::Failure),
            ("PROCESSOR_DECLINED", enums::AttemptStatus::Failure),
            ("GATEWAY_REJECTED", enums::AttemptStatus::Failure),
            ("FAILED", enums::AttemptStatus::Failure),
            ("VOIDED", enums::AttemptStatus::Voided),
            (
                "AUTHORIZATION_EXPIRED",
                enums::AttemptStatus::AuthorizationFailed,
            ),
        ] {
            assert_eq!(
                enums::AttemptStatus::from(payment_status(value)),
                expected,
                "unexpected AttemptStatus for {value}"
            );
        }
    }

    /// `Refund.status` is typed `PaymentStatus` in the SDL, so every one of the 13 values
    /// has to deserialize on a refund too — the previous five-member enum failed outright
    /// on a declined or rejected refund.
    #[test]
    fn every_sdl_status_deserializes_as_a_refund_status() {
        for value in PAYMENT_STATUS_SDL_VALUES {
            let _ = refund_status(value);
        }
    }

    #[test]
    fn refund_status_map() {
        for (value, expected) in [
            ("SUBMITTED_FOR_SETTLEMENT", enums::RefundStatus::Success),
            ("SETTLING", enums::RefundStatus::Success),
            ("SETTLED", enums::RefundStatus::Success),
            ("SETTLEMENT_PENDING", enums::RefundStatus::Success),
            ("SETTLEMENT_CONFIRMED", enums::RefundStatus::Pending),
            ("AUTHORIZED", enums::RefundStatus::Pending),
            ("AUTHORIZING", enums::RefundStatus::Pending),
            ("FAILED", enums::RefundStatus::Failure),
            ("GATEWAY_REJECTED", enums::RefundStatus::Failure),
            ("PROCESSOR_DECLINED", enums::RefundStatus::Failure),
            ("SETTLEMENT_DECLINED", enums::RefundStatus::Failure),
            ("AUTHORIZATION_EXPIRED", enums::RefundStatus::Failure),
            ("VOIDED", enums::RefundStatus::Failure),
        ] {
            assert_eq!(
                enums::RefundStatus::from(refund_status(value)),
                expected,
                "unexpected RefundStatus for {value}"
            );
        }
        // An unrecognised upstream value must parse and stay unresolved rather than being
        // guessed into success or failure.
        assert_eq!(
            enums::RefundStatus::from(refund_status("SOME_FUTURE_STATUS")),
            enums::RefundStatus::Unknown
        );
    }

    /// `reverseTransaction` answers with the `TransactionReversal = Refund | Transaction`
    /// union. Both arms must deserialize, and the arm has to be readable from `__typename`
    /// so the refund arm is not scored through the transaction status table.
    #[test]
    fn reversal_union_branches_deserialize() {
        let voided: CancelResponseTransactionBody = serde_json::from_value(serde_json::json!({
            "__typename": "Transaction",
            "id": "dHJhbnNhY3Rpb25fN3YyZjYyeTY",
            "legacyId": "7v2f62y6",
            "status": "VOIDED"
        }))
        .expect("Transaction arm must deserialize");
        assert_eq!(voided.typename, Some(ReversalKind::Transaction));

        let refunded: CancelResponseTransactionBody = serde_json::from_value(serde_json::json!({
            "__typename": "Refund",
            "id": "cmVmdW5kXzRnaG5xZ2Fr",
            "legacyId": "4ghnqgak",
            "status": "SUBMITTED_FOR_SETTLEMENT"
        }))
        .expect("Refund arm must deserialize");
        assert_eq!(refunded.typename, Some(ReversalKind::Refund));
        assert!(!refunded.status.is_terminal_failure());
    }

    #[test]
    fn terminal_failure_predicate() {
        for value in [
            "FAILED",
            "GATEWAY_REJECTED",
            "PROCESSOR_DECLINED",
            "SETTLEMENT_DECLINED",
            "AUTHORIZATION_EXPIRED",
        ] {
            assert!(
                payment_status(value).is_terminal_failure(),
                "{value} must be a terminal failure"
            );
        }
        for value in [
            "AUTHORIZED",
            "AUTHORIZING",
            "SUBMITTED_FOR_SETTLEMENT",
            "SETTLING",
            "SETTLED",
            "SETTLEMENT_PENDING",
            "SETTLEMENT_CONFIRMED",
            "VOIDED",
        ] {
            assert!(
                !payment_status(value).is_terminal_failure(),
                "{value} must not be a terminal failure"
            );
        }
    }

    // -----------------------------------------------------------------------------------
    // Request-side data enrichment
    // -----------------------------------------------------------------------------------

    use common_utils::types::StringMajorUnitForConnector;
    use domain_types::connector_types::{OrderInfo, TaxInfo};

    /// The same `StringMajorUnit` converter `braintree.rs` declares as the connector's
    /// `amount_converter`.
    const AMOUNT_CONVERTER: &StringMajorUnitForConnector = &StringMajorUnitForConnector;

    fn order_detail(product_name: &str, quantity: u16, unit_minor: i64) -> OrderDetailsWithAmount {
        OrderDetailsWithAmount {
            product_name: product_name.to_string(),
            quantity,
            amount: MinorUnit::new(unit_minor),
            ..Default::default()
        }
    }

    fn l2_l3(order_info: Option<OrderInfo>, tax_info: Option<TaxInfo>) -> L2L3Data {
        L2L3Data {
            order_info,
            tax_info,
            ..Default::default()
        }
    }

    fn order_info(
        order_details: Option<Vec<OrderDetailsWithAmount>>,
        merchant_order_reference_id: Option<String>,
        discount_amount: Option<i64>,
        shipping_cost: Option<i64>,
    ) -> OrderInfo {
        OrderInfo {
            order_date: None,
            order_details,
            merchant_order_reference_id,
            discount_amount: discount_amount.map(MinorUnit::new),
            shipping_cost: shipping_cost.map(MinorUnit::new),
            duty_amount: None,
        }
    }

    fn tax_info(
        order_tax_amount: Option<i64>,
        shipping_amount_tax: Option<i64>,
        tax_status: Option<enums::TaxStatus>,
    ) -> TaxInfo {
        TaxInfo {
            tax_status,
            customer_tax_registration_id: None,
            merchant_tax_registration_id: None,
            shipping_amount_tax: shipping_amount_tax.map(MinorUnit::new),
            order_tax_amount: order_tax_amount.map(MinorUnit::new),
        }
    }

    fn enrichment_inputs<'a>(
        l2_l3_data: Option<&'a L2L3Data>,
        shipping: Option<&'a Address>,
        billing_descriptor: Option<&'a BillingDescriptor>,
        amount_minor: i64,
    ) -> EnrichmentInputs<'a> {
        EnrichmentInputs {
            amount_converter: AMOUNT_CONVERTER,
            l2_l3_data,
            order_details: None,
            shipping,
            billing_descriptor,
            surcharge_amount: None,
            shipping_cost: None,
            amount: MinorUnit::new(amount_minor),
            currency: enums::Currency::USD,
        }
    }

    fn to_json(value: &impl Serialize) -> serde_json::Value {
        serde_json::to_value(value).expect("enrichment must serialize")
    }

    /// `Braintree-Version` is pinned to `2019-01-01`, which is *before* the 2021-02-01 cutoff
    /// at which `scalar CountryCode` switched to alpha-2. Every address country must therefore
    /// go out as ISO 3166-1 alpha-3 — the exact silent-wrong-value bug this conversion exists
    /// to prevent.
    #[test]
    fn address_country_code_is_alpha3_at_the_pinned_braintree_version() {
        assert_eq!(
            crate::connectors::braintree::BRAINTREE_VERSION_VALUE,
            "2019-01-01"
        );
        for (alpha2, alpha3) in [
            (enums::CountryAlpha2::US, "USA"),
            (enums::CountryAlpha2::GB, "GBR"),
            (enums::CountryAlpha2::DE, "DEU"),
            (enums::CountryAlpha2::IN, "IND"),
        ] {
            let address = AddressDetails {
                country: Some(alpha2),
                ..Default::default()
            };
            let built = build_address_input(Some(&address), None)
                .expect("an address carrying a country is not empty");
            assert_eq!(
                to_json(&built),
                serde_json::json!({ "countryCode": alpha3 })
            );
        }
    }

    /// `PhoneInput.countryPhoneCode` and `.phoneNumber` are both non-null, so a half-populated
    /// phone must be dropped rather than partially sent, and the `+` UCS stores must be
    /// stripped.
    #[test]
    fn address_phone_is_all_or_nothing_and_strips_the_plus() {
        let complete = PhoneDetails {
            number: Some(Secret::new("3125551212".to_string())),
            country_code: Some("+1".to_string()),
        };
        let built = build_address_input(None, Some(&complete)).expect("a full phone is not empty");
        assert_eq!(
            to_json(&built),
            serde_json::json!({ "phone": { "countryPhoneCode": "1", "phoneNumber": "3125551212" } })
        );

        for partial in [
            PhoneDetails {
                number: Some(Secret::new("3125551212".to_string())),
                country_code: None,
            },
            PhoneDetails {
                number: None,
                country_code: Some("+1".to_string()),
            },
        ] {
            assert!(build_address_input(None, Some(&partial)).is_none());
        }
    }

    /// Exactly one member of each `AddressInput` alias pair may be sent; this connector commits
    /// to the legacy set, so the PayPal-style aliases must never appear.
    #[test]
    fn address_uses_only_the_legacy_alias_set() {
        let address = AddressDetails {
            city: Some(Secret::new("Chicago".to_string())),
            country: Some(enums::CountryAlpha2::US),
            line1: Some(Secret::new("1 E Main St".to_string())),
            line2: Some(Secret::new("Suite 403".to_string())),
            line3: Some(Secret::new("dropped".to_string())),
            zip: Some(Secret::new("60622".to_string())),
            state: Some(Secret::new("IL".to_string())),
            first_name: Some(Secret::new("Jane".to_string())),
            last_name: Some(Secret::new("Doe".to_string())),
            origin_zip: None,
        };
        let built = to_json(&build_address_input(Some(&address), None).expect("not empty"));
        assert_eq!(
            built,
            serde_json::json!({
                "firstName": "Jane",
                "lastName": "Doe",
                "streetAddress": "1 E Main St",
                "extendedAddress": "Suite 403",
                "locality": "Chicago",
                "region": "IL",
                "postalCode": "60622",
                "countryCode": "USA"
            })
        );
        for alias in ["addressLine1", "addressLine2", "adminArea1", "adminArea2"] {
            assert!(built.get(alias).is_none(), "{alias} must not be emitted");
        }
    }

    /// Every Braintree money field is a major-unit decimal string; the UCS side is `MinorUnit`.
    #[test]
    fn minor_unit_amounts_convert_to_major_unit_strings() {
        for (minor, expected) in [(1_i64, "0.01"), (1234, "12.34"), (2603, "26.03")] {
            assert_eq!(
                to_json(
                    &convert_major(
                        AMOUNT_CONVERTER,
                        MinorUnit::new(minor),
                        enums::Currency::USD
                    )
                    .expect("conversion must succeed")
                ),
                serde_json::json!(expected)
            );
        }
    }

    /// `lineItems[].totalAmount` is `String!` and required for Level 3, but the proto has no
    /// `total_amount` and the proto -> domain conversion hardcodes `None`. It must therefore be
    /// derived as quantity x unit amount, in MinorUnit, and `total_amount` must win when the
    /// caller does supply it.
    #[test]
    fn line_item_total_amount_is_derived_when_absent_and_honoured_when_present() {
        let derived = order_detail("Blue Widget", 3, 999);
        let (line_item, total) = build_line_item(AMOUNT_CONVERTER, &derived, enums::Currency::USD)
            .expect("line item builds")
            .expect("line item is representable");
        assert_eq!(total, MinorUnit::new(2997));
        assert_eq!(
            to_json(&line_item),
            serde_json::json!({
                "name": "Blue Widget",
                "kind": "DEBIT",
                "quantity": "3",
                "unitAmount": "9.99",
                "totalAmount": "29.97"
            })
        );

        let supplied = OrderDetailsWithAmount {
            total_amount: Some(MinorUnit::new(1898)),
            ..order_detail("Blue Widget", 2, 999)
        };
        let (line_item, total) = build_line_item(AMOUNT_CONVERTER, &supplied, enums::Currency::USD)
            .expect("line item builds")
            .expect("line item is representable");
        assert_eq!(total, MinorUnit::new(1898));
        assert_eq!(
            to_json(&line_item).get("totalAmount"),
            Some(&serde_json::json!("18.98"))
        );
    }

    /// Level 3 text fields accept only `a-z`, `A-Z`, `0-9`, `'`, `.`, `-` and spaces, and the
    /// sanitisation has to happen before the length truncation.
    #[test]
    fn line_item_text_is_sanitised_then_truncated() {
        assert_eq!(
            sanitize_l3_text("Café & Crème / Deluxe", L3_NAME_MAX_LEN).as_deref(),
            Some("Caf  Crme  Deluxe")
        );
        assert_eq!(sanitize_l3_text("&&&///", L3_NAME_MAX_LEN), None);
        assert_eq!(
            sanitize_l3_text("ABCDEFGHIJKLMNOPQRSTUVWXYZ", L3_CODE_MAX_LEN).as_deref(),
            Some("ABCDEFGHIJKL")
        );
        // ASCII-only, 17 characters, for `purchaseOrderNumber`.
        assert_eq!(
            sanitize_purchase_order_number("PO-2026-0042").as_deref(),
            Some("PO-2026-0042")
        );
        assert_eq!(
            sanitize_purchase_order_number("PO-2026-0042-EXTRA-TAIL").as_deref(),
            Some("PO-2026-0042-EXTR")
        );
    }

    /// A line whose name does not survive sanitisation cannot be sent as Level 3, and no
    /// placeholder name may be invented — the whole array is dropped instead.
    #[test]
    fn unrepresentable_line_item_drops_the_whole_array() {
        let data = l2_l3(
            Some(order_info(
                Some(vec![order_detail("&&&", 1, 1000)]),
                None,
                None,
                None,
            )),
            None,
        );
        let enrichment =
            TransactionEnrichment::build(enrichment_inputs(Some(&data), None, None, 1000))
                .expect("build must succeed");
        assert_eq!(to_json(&enrichment), serde_json::json!({}));
    }

    /// The full Level 2/3 mapping, on a body that satisfies the reconciliation rule:
    ///   19.98 (line items) + 1.66 (tax) + 4.99 (shipping) + 0.40 (shipping tax)
    ///   - 1.00 (discount) = 26.03 = amount
    #[test]
    fn full_l2_l3_mapping_reconciles_and_serializes() {
        let line = OrderDetailsWithAmount {
            total_tax_amount: Some(MinorUnit::new(166)),
            unit_of_measure: Some("EA".to_string()),
            product_id: Some("WIDGET-BLU".to_string()),
            commodity_code: Some("44121700".to_string()),
            description: Some("Blue widget, medium".to_string()),
            ..order_detail("Blue Widget", 2, 999)
        };
        let data = L2L3Data {
            shipping_details: None,
            ..l2_l3(
                Some(order_info(
                    Some(vec![line]),
                    Some("PO-2026-0042".to_string()),
                    Some(100),
                    Some(499),
                )),
                Some(tax_info(
                    Some(166),
                    Some(40),
                    Some(enums::TaxStatus::Taxable),
                )),
            )
        };
        let shipping = Address {
            address: Some(AddressDetails {
                city: Some(Secret::new("Chicago".to_string())),
                country: Some(enums::CountryAlpha2::US),
                line1: Some(Secret::new("500 W Madison St".to_string())),
                zip: Some(Secret::new("60661".to_string())),
                state: Some(Secret::new("IL".to_string())),
                origin_zip: Some(Secret::new("60622".to_string())),
                ..Default::default()
            }),
            phone: None,
            email: None,
        };
        let descriptor = BillingDescriptor {
            name: Some(Secret::new("ACME*WIDGETS".to_string())),
            city: None,
            phone: Some(Secret::new("3125551212".to_string())),
            statement_descriptor: None,
            statement_descriptor_suffix: None,
            reference: None,
        };
        let enrichment = TransactionEnrichment::build(enrichment_inputs(
            Some(&data),
            Some(&shipping),
            Some(&descriptor),
            2603,
        ))
        .expect("build must succeed");

        assert_eq!(
            to_json(&enrichment),
            serde_json::json!({
                "purchaseOrderNumber": "PO-2026-0042",
                "descriptor": { "name": "ACME*WIDGETS", "phone": "3125551212" },
                "tax": { "taxAmount": "1.66", "taxExempt": false },
                "discountAmount": "1.00",
                "shipping": {
                    "shippingAddress": {
                        "streetAddress": "500 W Madison St",
                        "locality": "Chicago",
                        "region": "IL",
                        "postalCode": "60661",
                        "countryCode": "USA"
                    },
                    "shippingAmount": "4.99",
                    "shippingTaxAmount": "0.40",
                    "shipsFromPostalCode": "60622"
                },
                "lineItems": [{
                    "name": "Blue Widget",
                    "kind": "DEBIT",
                    "quantity": "2",
                    "unitAmount": "9.99",
                    "totalAmount": "19.98",
                    "taxAmount": "1.66",
                    "unitOfMeasure": "EA",
                    "productCode": "WIDGET-BLU",
                    "commodityCode": "44121700",
                    "description": "Blue widget, medium"
                }]
            })
        );
    }

    /// The reconciliation rule is a hard Braintree requirement, and the classic way to break it
    /// is to subtract the same discount twice — once per line, once at transaction level. An
    /// unbalanced breakdown must be dropped rather than sent, keeping the non-monetary fields.
    #[test]
    fn unbalanced_l3_breakdown_drops_only_the_monetary_fields() {
        let data = l2_l3(
            Some(order_info(
                Some(vec![order_detail("Blue Widget", 2, 999)]),
                Some("PO-1".to_string()),
                Some(100),
                Some(499),
            )),
            Some(tax_info(
                Some(166),
                Some(40),
                Some(enums::TaxStatus::Exempt),
            )),
        );
        // 19.98 + 1.66 + 4.99 + 0.40 - 1.00 = 26.03, but the transaction is for 30.00.
        let enrichment =
            TransactionEnrichment::build(enrichment_inputs(Some(&data), None, None, 3000))
                .expect("build must succeed");
        assert_eq!(
            to_json(&enrichment),
            serde_json::json!({
                "purchaseOrderNumber": "PO-1",
                "tax": { "taxExempt": true }
            })
        );
    }

    /// Level 2 on its own — a bare `tax.taxAmount` with no line items — carries no sum to
    /// reconcile and must survive untouched.
    #[test]
    fn level_two_only_is_not_reconciled() {
        let data = l2_l3(
            Some(order_info(None, Some("PO-2".to_string()), None, None)),
            Some(tax_info(Some(166), None, Some(enums::TaxStatus::Taxable))),
        );
        let enrichment =
            TransactionEnrichment::build(enrichment_inputs(Some(&data), None, None, 2603))
                .expect("build must succeed");
        assert_eq!(
            to_json(&enrichment),
            serde_json::json!({
                "purchaseOrderNumber": "PO-2",
                "tax": { "taxAmount": "1.66", "taxExempt": false }
            })
        );
    }

    /// Regression guard for the already-validated flows: with no enrichment data the flattened
    /// block must add nothing at all to the serialized transaction body, and neither `tax: {}`,
    /// `shipping: {}`, `descriptor: {}` nor `lineItems: []` may ever be emitted.
    #[test]
    fn empty_enrichment_serializes_to_nothing() {
        let enrichment = TransactionEnrichment::build(enrichment_inputs(None, None, None, 1000))
            .expect("build must succeed");
        assert_eq!(to_json(&enrichment), serde_json::json!({}));

        let body = RegularTransactionBody {
            amount: convert_major(AMOUNT_CONVERTER, MinorUnit::new(1000), enums::Currency::USD)
                .expect("conversion must succeed"),
            merchant_account_id: Secret::new("juspay".to_string()),
            channel: constants::CHANNEL_CODE.to_string(),
            customer_details: None,
            order_id: "ref_1".to_string(),
            enrichment,
        };
        assert_eq!(
            to_json(&body),
            serde_json::json!({
                "amount": "10.00",
                "merchantAccountId": "juspay",
                "channel": constants::CHANNEL_CODE,
                "orderId": "ref_1"
            })
        );
    }

    // -----------------------------------------------------------------------
    // Response & error surface: AVS / CVV, processor codes, advice codes.
    // The payloads below are copied verbatim from live Braintree sandbox
    // responses captured under `Braintree-Version: 2019-01-01`.
    // -----------------------------------------------------------------------

    fn surface(value: serde_json::Value) -> TransactionResponseSurface {
        serde_json::from_value(value).expect("response surface must deserialize")
    }

    /// A forced processor decline: sandbox amount `2001.00`, CVV `200` and billing postal
    /// code `20000` / street `200 N Main St` to drive every check to a mismatch.
    fn declined_surface() -> serde_json::Value {
        serde_json::json!({
            "processorAuthorizationResponse": {
                "legacyCode": "2001",
                "message": "Insufficient Funds",
                "cvvResponse": "DOES_NOT_MATCH",
                "avsPostalCodeResponse": "DOES_NOT_MATCH",
                "avsStreetAddressResponse": "DOES_NOT_MATCH",
                "authorizationId": null,
                "additionalInformation": "2001 : Insufficient Funds"
            },
            "processorSettlementResponse": { "legacyCode": null, "message": null },
            "statusHistory": [{
                "status": "PROCESSOR_DECLINED",
                "terminal": true,
                "declineType": "SOFT",
                "riskDecision": null,
                "networkResponse": { "code": "05", "message": "Do not honor" },
                "merchantAdviceCodeResponse": { "code": "01", "message": null }
            }]
        })
    }

    /// An approved auto-capture. Note the event order: the most recent event is
    /// `SUBMITTED_FOR_SETTLEMENT`, which carries no network response — the `AuthorizedEvent`
    /// that does is the entry behind it.
    fn approved_surface() -> serde_json::Value {
        serde_json::json!({
            "processorAuthorizationResponse": {
                "legacyCode": "1000",
                "message": "Approved",
                "cvvResponse": "MATCHES",
                "avsPostalCodeResponse": "SYSTEM_ERROR",
                "avsStreetAddressResponse": "MATCHES",
                "authorizationId": "2C8XV5",
                "additionalInformation": null
            },
            "processorSettlementResponse": { "legacyCode": null, "message": null },
            "statusHistory": [
                { "status": "SUBMITTED_FOR_SETTLEMENT", "terminal": false },
                {
                    "status": "AUTHORIZED",
                    "terminal": false,
                    "riskDecision": "REVIEW",
                    "networkResponse": { "code": "00", "message": "Approved" }
                }
            ]
        })
    }

    /// A gateway rejection: sandbox amount `5001.00`. The processor is never reached, so
    /// there is no processor `legacyCode` at all.
    fn gateway_rejected_surface() -> serde_json::Value {
        serde_json::json!({
            "processorAuthorizationResponse": {
                "legacyCode": null,
                "message": "Unavailable",
                "cvvResponse": null,
                "avsPostalCodeResponse": null,
                "avsStreetAddressResponse": null,
                "authorizationId": null,
                "additionalInformation": null
            },
            "processorSettlementResponse": { "legacyCode": null, "message": null },
            "statusHistory": [{
                "status": "GATEWAY_REJECTED",
                "terminal": true,
                "gatewayRejectionReason": "APPLICATION_INCOMPLETE",
                "riskDecision": null,
                "merchantAdviceCodeResponse": { "code": null, "message": null },
                "networkResponse": { "code": null, "message": null }
            }]
        })
    }

    /// Every value of the SDL's `AvsCvvResponseCode` must parse, and each must map to the
    /// single-letter REST code Braintree's own reference tables use.
    #[test]
    fn avs_cvv_response_code_rest_letters() {
        for (value, letter) in [
            ("MATCHES", "M"),
            ("DOES_NOT_MATCH", "N"),
            ("NOT_VERIFIED", "U"),
            ("NOT_PROVIDED", "I"),
            ("ISSUER_DOES_NOT_PARTICIPATE", "S"),
            ("SYSTEM_ERROR", "E"),
            ("NOT_APPLICABLE", "A"),
            ("BYPASS", "B"),
        ] {
            let parsed: AvsCvvResponseCode =
                serde_json::from_value(serde_json::json!(value)).expect("must deserialize");
            assert_eq!(
                parsed.as_rest_code(),
                Some(letter),
                "unexpected REST letter for {value}"
            );
        }
    }

    /// An outcome outside the SDL enum must degrade to `Unknown` rather than fail the whole
    /// Authorize response, and must not be given a letter it does not have.
    #[test]
    fn unknown_avs_cvv_value_degrades_instead_of_failing() {
        let parsed: AvsCvvResponseCode =
            serde_json::from_value(serde_json::json!("SOME_FUTURE_OUTCOME"))
                .expect("unknown value must still deserialize");
        assert_eq!(parsed, AvsCvvResponseCode::Unknown);
        assert_eq!(parsed.as_rest_code(), None);
        assert!(!parsed.is_mismatch());
        // Only an explicit mismatch counts as one — a check that did not run does not.
        assert!(AvsCvvResponseCode::DoesNotMatch.is_mismatch());
        for not_a_mismatch in [
            AvsCvvResponseCode::NotVerified,
            AvsCvvResponseCode::NotProvided,
            AvsCvvResponseCode::IssuerDoesNotParticipate,
            AvsCvvResponseCode::Bypass,
            AvsCvvResponseCode::NotApplicable,
            AvsCvvResponseCode::SystemError,
            AvsCvvResponseCode::Matches,
        ] {
            assert!(
                !not_a_mismatch.is_mismatch(),
                "{not_a_mismatch} is not a mismatch"
            );
        }
    }

    /// All fourteen SDL values must parse, including the five the prose documentation does
    /// not list, and an unlisted one must not fail the response.
    #[test]
    fn every_gateway_rejection_reason_parses() {
        for value in [
            "APPLICATION_INCOMPLETE",
            "AVS",
            "AVS_AND_CVV",
            "CVV",
            "DUPLICATE",
            "EXCESSIVE_RETRY",
            "FRAUD",
            "MANUAL_TRANSACTIONS_DISABLED",
            "PAYMENT_METHOD_BLOCKED",
            "RISK_THRESHOLD",
            "THREE_D_SECURE",
            "TOKEN_ISSUANCE",
            "TOO_MANY_CONFIRMATION_ATTEMPTS",
            "UNION_PAY_ENROLLMENT_REQUIRED",
        ] {
            let parsed: GatewayRejectionReason =
                serde_json::from_value(serde_json::json!(value)).expect("must deserialize");
            assert_ne!(
                parsed,
                GatewayRejectionReason::Unknown,
                "{value} must map to a named variant"
            );
            // Round-trips to the same wire spelling, which is what lands in `ErrorResponse.code`.
            assert_eq!(parsed.to_string(), value);
        }
        let unknown: GatewayRejectionReason =
            serde_json::from_value(serde_json::json!("SOME_FUTURE_REASON"))
                .expect("unknown value must still deserialize");
        assert_eq!(unknown, GatewayRejectionReason::Unknown);
    }

    /// The published Merchant Advice Code list. There is no code `42`, and `05`-`20`,
    /// `22`-`23` and `31`-`39` are not published either — an unlisted code must return no
    /// guidance rather than an invented one.
    #[test]
    fn merchant_advice_code_guidance_map() {
        for code in [
            "01", "02", "03", "04", "21", "24", "25", "26", "27", "28", "29", "30", "40", "41",
            "43",
        ] {
            assert!(
                merchant_advice_code_guidance(code).is_some(),
                "MAC {code} must carry guidance"
            );
        }
        for code in ["42", "05", "22", "31", "99", ""] {
            assert_eq!(
                merchant_advice_code_guidance(code),
                None,
                "MAC {code} is not published and must not be given guidance"
            );
        }
        // Do-not-retry codes must say so; the timed ones must name their interval.
        assert_eq!(
            merchant_advice_code_guidance("03"),
            Some("do not try again")
        );
        assert_eq!(
            merchant_advice_code_guidance("24"),
            Some("retry after 1 hour")
        );
        // A bare, un-padded code resolves to the same advice as its zero-padded form.
        assert_eq!(
            merchant_advice_code_guidance("1"),
            merchant_advice_code_guidance("01")
        );
    }

    #[test]
    fn processor_decline_type_map() {
        for (value, expected) in [
            ("HARD", ProcessorDeclineType::Hard),
            ("SOFT", ProcessorDeclineType::Soft),
            ("SOMETHING_ELSE", ProcessorDeclineType::Unknown),
        ] {
            let parsed: ProcessorDeclineType =
                serde_json::from_value(serde_json::json!(value)).expect("must deserialize");
            assert_eq!(parsed, expected);
        }
        assert!(ProcessorDeclineType::Hard
            .guidance()
            .is_some_and(|text| text.contains("do not retry")));
        assert!(ProcessorDeclineType::Soft
            .guidance()
            .is_some_and(|text| text.contains("retry may succeed")));
        assert_eq!(ProcessorDeclineType::Unknown.guidance(), None);
    }

    /// A `statusHistory` lookup must not stop at entry zero: on an approved auto-capture the
    /// most recent event carries no network response and the one behind it does.
    #[test]
    fn status_history_lookup_scans_past_the_most_recent_event() {
        let approved = surface(approved_surface());
        assert_eq!(approved.network_code(), Some("00".to_string()));
        assert_eq!(approved.network_message(), Some("Approved".to_string()));
        assert_eq!(approved.risk_decision(), Some(RiskDecision::Review));
        // `terminal` is read off the current event only, and an approved auto-capture is
        // not terminal.
        assert_eq!(approved.is_terminal(), Some(false));
        assert_eq!(approved.decline_type(), None);
        assert_eq!(approved.gateway_rejection_reason(), None);
        assert_eq!(approved.merchant_advice_code(), None);
    }

    /// The decline that this whole surface exists for: it must reach the caller as the
    /// processor's own code and text plus the values the Gateway Status Map keys on, never
    /// as an opaque `PROCESSOR_DECLINED`.
    #[test]
    fn processor_decline_surfaces_specific_gsm_fields() {
        let declined = surface(declined_surface());
        let error = create_declined_error_response(
            &BraintreePaymentStatus::ProcessorDeclined,
            &declined,
            Some("dHJhbnNhY3Rpb25fOXhwN201bmQ".to_string()),
            enums::AttemptStatus::Failure,
            200,
        );

        assert_eq!(error.code, "2001");
        assert_eq!(error.message, "Insufficient Funds");
        // The Mastercard advice code is what Hyperswitch looks up in
        // `merchant_advice_codes.<network>.<code>` to pick a recommended action.
        assert_eq!(error.network_advice_code, Some("01".to_string()));
        // The raw network code becomes the GSM `issuer_error_code`.
        assert_eq!(error.network_decline_code, Some("05".to_string()));
        assert_eq!(
            error.network_error_message,
            Some("Do not honor".to_string())
        );
        assert_eq!(
            error.connector_transaction_id,
            Some("dHJhbnNhY3Rpb25fOXhwN201bmQ".to_string())
        );
        // A terminal refusal must report a terminal payment status, or the attempt polls
        // forever. It must be a `Payment` status: this builder is only ever reached from a
        // payment flow, never from the flow-agnostic `ConnectorCommon::build_error_response`.
        assert!(matches!(
            error.attempt_status,
            Some(domain_types::router_data::FlowStatus::Payment(
                enums::AttemptStatus::Failure
            ))
        ));

        let reason = error.reason.expect("a decline must carry a reason");
        assert!(reason.contains("2001 : Insufficient Funds"), "{reason}");
        assert!(reason.contains("soft decline"), "{reason}");
        assert!(reason.contains("merchant advice code 01"), "{reason}");
        assert!(
            reason.contains("retry only with updated card details"),
            "{reason}"
        );
    }

    /// A gateway rejection never reaches the processor, so it has no processor code. The
    /// rejection reason has to become the error code, otherwise every rejection cause
    /// collapses into one opaque `GATEWAY_REJECTED`.
    #[test]
    fn gateway_rejection_reports_its_reason_as_the_code() {
        let rejected = surface(gateway_rejected_surface());
        let error = create_declined_error_response(
            &BraintreePaymentStatus::GatewayRejected,
            &rejected,
            Some("dHJhbnNhY3Rpb25fZTNuZHY0MmI".to_string()),
            enums::AttemptStatus::Failure,
            200,
        );

        assert_eq!(error.code, "APPLICATION_INCOMPLETE");
        assert_eq!(error.message, "Unavailable");
        // A null advice / network code must not become an empty string.
        assert_eq!(error.network_advice_code, None);
        assert_eq!(error.network_decline_code, None);
        let reason = error.reason.expect("a rejection must carry a reason");
        assert!(
            reason.contains("gateway rejection (APPLICATION_INCOMPLETE)"),
            "{reason}"
        );
        assert!(reason.contains("provisioning issue"), "{reason}");
    }

    /// A capture refusal reports in the 4000-class settlement response, not in the
    /// authorization response — reading the authorization code there would report the
    /// original approval as the failure reason.
    #[test]
    fn settlement_decline_prefers_the_settlement_code() {
        let declined = surface(serde_json::json!({
            "processorAuthorizationResponse": { "legacyCode": "1000", "message": "Approved" },
            "processorSettlementResponse": {
                "legacyCode": "4001",
                "message": "Settlement Declined"
            },
            "statusHistory": [{ "status": "SETTLEMENT_DECLINED", "terminal": true }]
        }));
        let error = create_declined_error_response(
            &BraintreePaymentStatus::SettlementDeclined,
            &declined,
            None,
            enums::AttemptStatus::Failure,
            200,
        );
        assert_eq!(error.code, "4001");
        assert_eq!(error.message, "Settlement Declined");
    }

    /// A refusal with nothing selected — a mutation whose selection set was never widened —
    /// must still produce a usable code rather than a blank one.
    #[test]
    fn empty_surface_falls_back_to_the_transaction_status() {
        let error = create_declined_error_response(
            &BraintreePaymentStatus::Failed,
            &TransactionResponseSurface::default(),
            None,
            enums::AttemptStatus::Failure,
            200,
        );
        assert_eq!(error.code, "FAILED");
        assert_eq!(error.message, "FAILED");
        assert_eq!(error.reason, Some("FAILED".to_string()));
        assert_ne!(error.code, NO_ERROR_CODE);
    }

    /// AVS / CVV results travel on the success path too: an approved transaction still
    /// carries them, and a merchant without AVS rules enabled sees a mismatch without a
    /// rejection.
    #[test]
    fn approved_transaction_surfaces_avs_cvv_and_auth_code() {
        let approved = surface(approved_surface());
        let connector_response =
            build_card_connector_response(&approved).expect("an approval must report its checks");
        let json = serde_json::to_value(&connector_response).expect("must serialize");
        let card = json
            .get("additional_payment_method_data")
            .and_then(|value| value.get("Card"))
            .expect("card response must be present");
        assert_eq!(card.get("auth_code"), Some(&serde_json::json!("2C8XV5")));
        let checks = card.get("payment_checks").expect("payment checks present");
        assert_eq!(
            checks.get("card_verification"),
            Some(&serde_json::json!("M"))
        );
        assert_eq!(
            checks.get("avs_street_address_response"),
            Some(&serde_json::json!("M"))
        );
        // The REST `avs_error_response_code` has no GraphQL equivalent; its `E` outcome
        // folds into the AVS fields as `SYSTEM_ERROR`, confirmed against sandbox with
        // billing postal code `30000`.
        assert_eq!(
            checks.get("avs_postal_code_response"),
            Some(&serde_json::json!("E"))
        );
        assert_eq!(
            checks.get("risk_decision"),
            Some(&serde_json::json!("REVIEW"))
        );

        let raw_status =
            build_raw_connector_status(&approved).expect("an approval reports its processor code");
        assert_eq!(raw_status.code, Some("1000".to_string()));
        assert_eq!(raw_status.message, Some("Approved".to_string()));
    }

    /// Nothing to report means nothing is emitted, rather than an empty bag.
    #[test]
    fn empty_surface_emits_no_connector_response() {
        let empty = TransactionResponseSurface::default();
        assert!(build_card_connector_response(&empty).is_none());
        assert!(build_raw_connector_status(&empty).is_none());
    }

    /// The widened selection sets must actually ask for the fields the response types read,
    /// and must keep asking for the identifiers the rest of the connector depends on.
    #[test]
    fn authorize_and_capture_selections_carry_the_response_surface() {
        for query in [
            constants::CHARGE_CREDIT_CARD_MUTATION,
            constants::AUTHORIZE_CREDIT_CARD_MUTATION,
            constants::CAPTURE_TRANSACTION_MUTATION,
            constants::AUTHORIZE_AND_VAULT_CREDIT_CARD_MUTATION,
            constants::CHARGE_AND_VAULT_TRANSACTION_MUTATION,
        ] {
            for field in [
                "processorAuthorizationResponse",
                "legacyCode",
                "cvvResponse",
                "avsPostalCodeResponse",
                "avsStreetAddressResponse",
                "authorizationId",
                "processorSettlementResponse",
                "statusHistory",
                "... on ProcessorDeclinedEvent",
                "declineType",
                "... on GatewayRejectedEvent",
                "gatewayRejectionReason",
                "merchantAdviceCodeResponse",
                "networkResponse",
                "id",
                "status",
            ] {
                assert!(
                    query.contains(field),
                    "{field} missing from selection: {query}"
                );
            }
            // The REST spellings are not GraphQL field names and would be a hard validation
            // error that breaks every call, not just the decline path.
            for wrong in [
                "processorResponseCode",
                "cvvResponseCode",
                "avsPostalCodeResponseCode",
                "avsErrorResponseCode",
                "statusEvents",
            ] {
                assert!(
                    !query.contains(wrong),
                    "{wrong} must not be selected: {query}"
                );
            }
        }
    }
}
