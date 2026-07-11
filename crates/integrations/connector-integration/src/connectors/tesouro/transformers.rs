use common_enums::{AttemptStatus, CaptureMethod, Currency};
use common_utils::{consts::NO_ERROR_CODE, consts::NO_ERROR_MESSAGE, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, RepeatPayment, ServerAuthenticationToken, SetupMandate},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RepeatPaymentData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{TesouroAmountConvertor, TesouroRouterData};
use crate::types::ResponseRouterData;
use domain_types::errors::{ConnectorError, IntegrationError};

type Error = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

pub const MAX_PAYMENT_REFERENCE_ID_LENGTH: usize = 28;

// =============================================================================
// GraphQL query strings (source of truth, matching the reference connector)
// =============================================================================

pub const SETUP_MANDATE: &str = "mutation VerifyAccount(
    $verifyAccountInput: VerifyAccountInput!
) {
    verifyAccount(
        verifyAccountInput: $verifyAccountInput
    ) {
        verifyAccountResponse {
            paymentId
            transactionId
            tokenDetails {
                token
            }
            activityDate
        }
        errors {
            ... on InternalServiceError { message transactionId processorResponseCode }
            ... on AcceptorNotFoundError { message transactionId processorResponseCode }
            ... on RuleInViolationError { message transactionId processorResponseCode }
            ... on SyntaxOnNetworkResponseError { message transactionId processorResponseCode }
            ... on TimeoutOnNetworkResponseError { message transactionId processorResponseCode }
            ... on ValidationFailureError { message processorResponseCode transactionId }
            ... on UnknownCardError { message processorResponseCode transactionId }
            ... on TokenNotFoundError { message processorResponseCode transactionId }
            ... on InvalidTokenError { message processorResponseCode transactionId }
            ... on RouteNotFoundError { message processorResponseCode transactionId }
        }
    }
}";

pub const AUTHORIZE_TRANSACTION: &str = "mutation AuthorizeCustomerInitiatedTransaction(
    $authorizeCustomerInitiatedTransactionInput: AuthorizeCustomerInitiatedTransactionInput!
) {
    authorizeCustomerInitiatedTransaction(
        authorizeCustomerInitiatedTransactionInput: $authorizeCustomerInitiatedTransactionInput
    ) {
        authorizationResponse {
            paymentId
            transactionId
            tokenDetails { token }
            activityDate
            __typename
            ... on AuthorizationApproval {
                __typename paymentId transactionId tokenDetails { token } activityDate
            }
            ... on AuthorizationDecline {
                __typename transactionId paymentId message tokenDetails { token }
            }
        }
        errors {
            ... on InternalServiceError { message transactionId processorResponseCode }
            ... on AcceptorNotFoundError { message transactionId processorResponseCode }
            ... on RuleInViolationError { message transactionId processorResponseCode }
            ... on SyntaxOnNetworkResponseError { message transactionId processorResponseCode }
            ... on TimeoutOnNetworkResponseError { message transactionId processorResponseCode }
            ... on ValidationFailureError { message processorResponseCode transactionId }
            ... on UnknownCardError { message processorResponseCode transactionId }
            ... on TokenNotFoundError { message processorResponseCode transactionId }
            ... on InvalidTokenError { message processorResponseCode transactionId }
            ... on RouteNotFoundError { message processorResponseCode transactionId }
        }
    }
}";

pub const AUTHORIZE_RECURRING: &str = "mutation AuthorizeRecurring(
    $authorizeRecurringInput: AuthorizeRecurringInput!
) {
    authorizeRecurring(
        authorizeRecurringInput: $authorizeRecurringInput
    ) {
        authorizationResponse {
            paymentId
            transactionId
            tokenDetails { token }
            activityDate
            __typename
            ... on AuthorizationApproval {
                __typename paymentId transactionId tokenDetails { token } activityDate
            }
            ... on AuthorizationDecline {
                __typename transactionId paymentId message
            }
        }
        errors {
            ... on InternalServiceError { message transactionId processorResponseCode }
            ... on AcceptorNotFoundError { message transactionId processorResponseCode }
            ... on RuleInViolationError { message transactionId processorResponseCode }
            ... on SyntaxOnNetworkResponseError { message transactionId processorResponseCode }
            ... on TimeoutOnNetworkResponseError { message transactionId processorResponseCode }
            ... on ValidationFailureError { message processorResponseCode transactionId }
            ... on UnknownCardError { message processorResponseCode transactionId }
            ... on TokenNotFoundError { message processorResponseCode transactionId }
            ... on InvalidTokenError { message processorResponseCode transactionId }
            ... on RouteNotFoundError { message processorResponseCode transactionId }
            ... on PriorPaymentNotFoundError { message processorResponseCode transactionId }
        }
    }
}";

pub const SYNC_TRANSACTION: &str = "query PaymentTransaction($paymentTransactionId: UUID!) {
  paymentTransaction(id: $paymentTransactionId) {
    __typename
    responseType
    reference
    id
    paymentId
    ... on AcceptedSale { __typename id processorResponseCode processorResponseMessage }
    ... on ApprovedAuthorization { __typename id processorResponseCode processorResponseMessage }
    ... on ApprovedCapture { __typename id processorResponseCode processorResponseMessage }
    ... on ApprovedReversal { __typename id processorResponseCode processorResponseMessage }
    ... on DeclinedAuthorization { __typename id processorResponseCode processorResponseMessage }
    ... on DeclinedCapture { __typename id processorResponseCode processorResponseMessage }
    ... on DeclinedReversal { __typename id processorResponseCode processorResponseMessage }
    ... on GenericPaymentTransaction { __typename id processorResponseCode processorResponseMessage }
    ... on Authorization { __typename id processorResponseCode processorResponseMessage }
    ... on Capture { __typename id processorResponseCode processorResponseMessage }
    ... on Reversal { __typename id processorResponseCode processorResponseMessage }
    ... on Sale { __typename id processorResponseCode processorResponseMessage }
  }
}";

// =============================================================================
// Auth
// =============================================================================

pub struct TesouroAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub acceptor_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TesouroAuthType {
    type Error = Error;
    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Tesouro {
                api_key,
                key1,
                api_secret,
                ..
            } => Ok(Self {
                client_id: api_key.clone(),
                client_secret: api_secret.clone(),
                acceptor_id: key1.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// =============================================================================
// Shared request structures
// =============================================================================

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TesouroPaymentEntryMode {
    PaymentMethodNotOnFile,
    PaymentMethodOnFile,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TesouroStorageIntent {
    StoreOnFile,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TesouroAutomaticCapture {
    Never,
    OnApproval,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TesouroAuthorizationIntent {
    FinalAuthorization,
    PreAuthorization,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TesouroOmissionReason {
    VerificationNotRequested,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum TesouroSecurityCode {
    Value {
        value: Secret<String>,
    },
    #[serde(rename_all = "camelCase")]
    OmissionReason {
        omission_reason: TesouroOmissionReason,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroCardWithPanDetails<T: PaymentMethodDataTypes> {
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub account_number: RawCardNumber<T>,
    pub payment_entry_mode: TesouroPaymentEntryMode,
    pub security_code: TesouroSecurityCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_intent: Option<TesouroStorageIntent>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroAcquirerTokenDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_month: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_year: Option<Secret<String>>,
    pub token: Secret<String>,
    pub security_code: TesouroSecurityCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_type: Option<String>,
}

/// Externally-tagged payment method details for CIT/verifyAccount (card only).
/// Variant name is the JSON key; `camelCase` yields `cardWithPanDetails`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TesouroPaymentMethodDetails<T: PaymentMethodDataTypes> {
    CardWithPanDetails(TesouroCardWithPanDetails<T>),
}

/// Externally-tagged payment method details for recurring/MIT (acquirer token).
/// Variant name is the JSON key; `camelCase` yields `acquirerTokenDetails`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TesouroRecurringPaymentMethodDetails {
    AcquirerTokenDetails(TesouroAcquirerTokenDetails),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAmountDetails {
    pub total_amount: FloatMajorUnit,
    pub currency: Currency,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillToAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<common_enums::CountryAlpha3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
}

impl BillToAddress {
    fn from_payment_flow(data: &PaymentFlowData) -> Self {
        Self {
            address1: data.get_optional_billing_line1(),
            address2: data.get_optional_billing_line2(),
            address3: data.get_optional_billing_line3(),
            city: data.get_optional_billing_city(),
            country_code: data
                .get_optional_billing_country()
                .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
            first_name: data.get_optional_billing_first_name(),
            last_name: data.get_optional_billing_last_name(),
            postal_code: data.get_optional_billing_zip(),
            state: data.get_optional_billing_state(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitReference {
    pub cit_payment_id: Secret<String>,
}

// -------- helpers --------

fn get_valid_transaction_id(id: String) -> Result<String, Error> {
    if id.len() <= MAX_PAYMENT_REFERENCE_ID_LENGTH {
        Ok(id)
    } else {
        Err(IntegrationError::MaxFieldLengthViolated {
            connector: "Tesouro".to_string(),
            field_name: "transaction_reference".to_string(),
            max_length: MAX_PAYMENT_REFERENCE_ID_LENGTH,
            received_length: id.len(),
            context: Default::default(),
        }
        .into())
    }
}

fn is_auto_capture(capture_method: Option<CaptureMethod>) -> bool {
    matches!(capture_method, Some(CaptureMethod::Automatic) | None)
}

fn current_date() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

fn get_card_payment_method<T: PaymentMethodDataTypes>(
    card: &Card<T>,
    is_mandate_payment: bool,
) -> Result<TesouroPaymentMethodDetails<T>, Error> {
    let card_data = TesouroCardWithPanDetails {
        expiration_month: card.get_card_expiry_month_2_digit().change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )?,
        expiration_year: card.get_expiry_year_4_digit(),
        account_number: card.card_number.clone(),
        payment_entry_mode: if is_mandate_payment {
            TesouroPaymentEntryMode::PaymentMethodOnFile
        } else {
            TesouroPaymentEntryMode::PaymentMethodNotOnFile
        },
        security_code: TesouroSecurityCode::Value {
            value: card.card_cvc.clone(),
        },
        storage_intent: if is_mandate_payment {
            Some(TesouroStorageIntent::StoreOnFile)
        } else {
            None
        },
    };
    Ok(TesouroPaymentMethodDetails::CardWithPanDetails(card_data))
}

// =============================================================================
// AccessToken (ServerAuthenticationToken) flow — OAuth2 client_credentials
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TesouroTokenRequest {
    grant_type: String,
    client_id: Secret<String>,
    client_secret: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TesouroRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for TesouroTokenRequest
{
    type Error = Error;
    fn try_from(
        item: TesouroRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TesouroAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            grant_type: "client_credentials".to_string(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TesouroAccessTokenResponse {
    access_token: Secret<String>,
    token_type: Option<String>,
    expires_in: Option<i64>,
}

impl<F> TryFrom<ResponseRouterData<TesouroAccessTokenResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<TesouroAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                token_type: item.response.token_type,
                expires_in: item.response.expires_in,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// SetupMandate (verifyAccount) flow
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroVerifyAccountInput<T: PaymentMethodDataTypes> {
    pub acceptor_id: Secret<String>,
    pub transaction_reference: String,
    pub bill_to_address: BillToAddress,
    pub payment_method_details: TesouroPaymentMethodDetails<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroVerifyAccountVariables<T: PaymentMethodDataTypes> {
    pub verify_account_input: TesouroVerifyAccountInput<T>,
}

#[derive(Debug, Serialize)]
pub struct TesouroSetupMandateRequest<T: PaymentMethodDataTypes> {
    query: String,
    variables: TesouroVerifyAccountVariables<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TesouroRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TesouroSetupMandateRequest<T>
{
    type Error = Error;
    fn try_from(
        item: TesouroRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TesouroAuthType::try_from(&router_data.connector_config)?;
        let transaction_reference = get_valid_transaction_id(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )?;
        let bill_to_address = BillToAddress::from_payment_flow(&router_data.resource_common_data);
        let payment_method_details = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => get_card_payment_method(card, true)?,
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Only Card payment method is supported for Tesouro SetupMandate".to_string(),
                    Default::default(),
                )
                .into())
            }
        };
        Ok(Self {
            query: SETUP_MANDATE.to_string(),
            variables: TesouroVerifyAccountVariables {
                verify_account_input: TesouroVerifyAccountInput {
                    acceptor_id: auth.acceptor_id,
                    transaction_reference,
                    bill_to_address,
                    payment_method_details,
                },
            },
        })
    }
}

// =============================================================================
// Authorize (customer-initiated) flow
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeCustomerInitiatedTransactionInput<T: PaymentMethodDataTypes> {
    pub acceptor_id: Secret<String>,
    pub transaction_reference: String,
    pub payment_method_details: TesouroPaymentMethodDetails<T>,
    pub transaction_amount_details: TransactionAmountDetails,
    pub automatic_capture: TesouroAutomaticCapture,
    pub authorization_intent: TesouroAuthorizationIntent,
    pub bill_to_address: BillToAddress,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroAuthorizeVariables<T: PaymentMethodDataTypes> {
    pub authorize_customer_initiated_transaction_input:
        AuthorizeCustomerInitiatedTransactionInput<T>,
}

#[derive(Debug, Serialize)]
pub struct TesouroAuthorizeRequest<T: PaymentMethodDataTypes> {
    query: String,
    variables: TesouroAuthorizeVariables<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TesouroRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for TesouroAuthorizeRequest<T>
{
    type Error = Error;
    fn try_from(
        item: TesouroRouterData<
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
        let auth = TesouroAuthType::try_from(&router_data.connector_config)?;
        let transaction_reference = get_valid_transaction_id(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )?;
        let auto_capture = is_auto_capture(router_data.request.capture_method);
        let is_mandate = router_data.request.setup_future_usage
            == Some(common_enums::FutureUsage::OffSession)
            || router_data.request.customer_acceptance.is_some();

        let payment_method_details = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                if router_data.resource_common_data.auth_type
                    == common_enums::AuthenticationType::ThreeDs
                {
                    return Err(IntegrationError::NotSupported {
                        message: "Cards 3DS".to_string(),
                        connector: "Tesouro",
                        context: Default::default(),
                    }
                    .into());
                }
                get_card_payment_method(card, is_mandate)?
            }
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Only Card payment method is supported for Tesouro Authorize".to_string(),
                    Default::default(),
                )
                .into())
            }
        };

        let amount = TesouroAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            query: AUTHORIZE_TRANSACTION.to_string(),
            variables: TesouroAuthorizeVariables {
                authorize_customer_initiated_transaction_input:
                    AuthorizeCustomerInitiatedTransactionInput {
                        acceptor_id: auth.acceptor_id,
                        transaction_reference,
                        payment_method_details,
                        transaction_amount_details: TransactionAmountDetails {
                            total_amount: amount,
                            currency: router_data.request.currency,
                        },
                        automatic_capture: if auto_capture {
                            TesouroAutomaticCapture::OnApproval
                        } else {
                            TesouroAutomaticCapture::Never
                        },
                        authorization_intent: if auto_capture {
                            TesouroAuthorizationIntent::FinalAuthorization
                        } else {
                            TesouroAuthorizationIntent::PreAuthorization
                        },
                        bill_to_address: BillToAddress::from_payment_flow(
                            &router_data.resource_common_data,
                        ),
                    },
            },
        })
    }
}

// =============================================================================
// RepeatPayment (authorizeRecurring / MIT) flow
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeRecurringTransactionInput {
    pub acceptor_id: Secret<String>,
    pub transaction_reference: String,
    pub payment_method_details: TesouroRecurringPaymentMethodDetails,
    pub transaction_amount_details: TransactionAmountDetails,
    pub automatic_capture: TesouroAutomaticCapture,
    pub bill_to_address: BillToAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cit_reference: Option<CitReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_purchase_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroRecurringVariables {
    pub authorize_recurring_input: AuthorizeRecurringTransactionInput,
}

#[derive(Debug, Serialize)]
pub struct TesouroRepeatPaymentRequest {
    query: String,
    variables: TesouroRecurringVariables,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroMandateMetadata {
    pub activity_date: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TesouroRouterData<
            RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
            T,
        >,
    > for TesouroRepeatPaymentRequest
{
    type Error = Error;
    fn try_from(
        item: TesouroRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TesouroAuthType::try_from(&router_data.connector_config)?;
        let transaction_reference = get_valid_transaction_id(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        )?;

        let connector_mandate_ref = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_ref) => mandate_ref,
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Only ConnectorMandateId is supported for Tesouro recurring payments"
                        .to_string(),
                    Default::default(),
                )
                .into())
            }
        };
        let connector_mandate_id = connector_mandate_ref.get_connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            },
        )?;

        // Card expiry (optional): taken from the stored card details carried in the request.
        let (expiration_month, expiration_year) = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => (
                Some(card.get_card_expiry_month_2_digit().change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    },
                )?),
                Some(card.get_expiry_year_4_digit()),
            ),
            _ => (None, None),
        };

        let cit_reference = connector_mandate_ref
            .get_connector_mandate_request_reference_id()
            .map(|cit_payment_id| CitReference {
                cit_payment_id: cit_payment_id.into(),
            });

        let original_purchase_date = match &router_data.request.recurring_mandate_payment_data {
            Some(recurring_data) => match &recurring_data.mandate_metadata {
                Some(metadata) => {
                    let tesouro_metadata: TesouroMandateMetadata =
                        serde_json::from_value(metadata.clone().expose()).change_context(
                            IntegrationError::RequestEncodingFailed {
                                context: Default::default(),
                            },
                        )?;
                    Some(tesouro_metadata.activity_date)
                }
                None => Some(current_date()),
            },
            None => Some(current_date()),
        };

        let amount = TesouroAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            query: AUTHORIZE_RECURRING.to_string(),
            variables: TesouroRecurringVariables {
                authorize_recurring_input: AuthorizeRecurringTransactionInput {
                    acceptor_id: auth.acceptor_id,
                    transaction_reference,
                    payment_method_details:
                        TesouroRecurringPaymentMethodDetails::AcquirerTokenDetails(
                            TesouroAcquirerTokenDetails {
                                expiration_month,
                                expiration_year,
                                token: Secret::new(connector_mandate_id),
                                security_code: TesouroSecurityCode::OmissionReason {
                                    omission_reason: TesouroOmissionReason::VerificationNotRequested,
                                },
                                wallet_type: None,
                            },
                        ),
                    transaction_amount_details: TransactionAmountDetails {
                        total_amount: amount,
                        currency: router_data.request.currency,
                    },
                    automatic_capture: if is_auto_capture(router_data.request.capture_method) {
                        TesouroAutomaticCapture::OnApproval
                    } else {
                        TesouroAutomaticCapture::Never
                    },
                    bill_to_address: BillToAddress::from_payment_flow(
                        &router_data.resource_common_data,
                    ),
                    cit_reference,
                    original_purchase_date,
                },
            },
        })
    }
}

// =============================================================================
// PSync (paymentTransaction) flow
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroSyncVariables {
    pub payment_transaction_id: String,
}

#[derive(Debug, Serialize)]
pub struct TesouroSyncRequest {
    query: String,
    variables: TesouroSyncVariables,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TesouroRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for TesouroSyncRequest
{
    type Error = Error;
    fn try_from(
        item: TesouroRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_transaction_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;
        Ok(Self {
            query: SYNC_TRANSACTION.to_string(),
            variables: TesouroSyncVariables {
                payment_transaction_id,
            },
        })
    }
}

// =============================================================================
// Response types (shared)
// =============================================================================

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TesouroApiResponse<T> {
    Success(TesouroApiSuccess<T>),
    Error(TesouroApiErrorResponse),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TesouroApiSuccess<T> {
    data: T,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TesouroApiErrorResponse {
    errors: Vec<TesouroApiErrorData>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TesouroApiErrorData {
    message: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TesouroTokenDetails {
    token: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroTransactionErrorData {
    pub message: String,
    pub processor_response_code: Option<String>,
    pub transaction_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroTransactionMetadata {
    pub payment_id: String,
}

// ---- error response for HTTP/build_error_response path ----

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TesouroGraphQlErrorResponse {
    pub errors: Vec<TesouroGraphQlError>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TesouroGraphQlError {
    pub message: String,
    pub extensions: Option<TesouroGraphQlErrorExtensions>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TesouroGraphQlErrorExtensions {
    pub code: Option<String>,
    pub reason: Option<String>,
}

// ---- helpers to build responses ----

fn build_mandate_reference(
    token_details: Option<TesouroTokenDetails>,
    payment_id: Option<String>,
    activity_date: Option<String>,
) -> Option<MandateReference> {
    token_details.map(|token_details| {
        let mandate_metadata = activity_date.map(|activity_date| {
            common_utils::pii::SecretSerdeValue::new(serde_json::json!(TesouroMandateMetadata {
                activity_date
            }))
        });
        MandateReference {
            connector_mandate_id: token_details.token.map(|token| token.expose()),
            payment_method_id: None,
            connector_mandate_request_reference_id: payment_id,
            mandate_metadata,
        }
    })
}

fn in_band_error(
    errors: &[TesouroTransactionErrorData],
    http_code: u16,
    attempt_status: AttemptStatus,
) -> Result<PaymentsResponseData, ErrorResponse> {
    let error = errors.first();
    let message = error
        .map(|e| e.message.clone())
        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
    Err(ErrorResponse {
        code: error
            .and_then(|e| e.processor_response_code.clone())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: message.clone(),
        reason: Some(message),
        status_code: http_code,
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(attempt_status)),
        connector_transaction_id: error.and_then(|e| e.transaction_id.clone()),
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
    })
}

fn top_level_error(
    error_response: &TesouroApiErrorResponse,
    http_code: u16,
    attempt_status: AttemptStatus,
) -> Result<PaymentsResponseData, ErrorResponse> {
    let message = error_response
        .errors
        .iter()
        .map(|error| error.message.clone())
        .collect::<Vec<String>>()
        .join(" ");
    let message = if message.is_empty() {
        NO_ERROR_MESSAGE.to_string()
    } else {
        message
    };
    Err(ErrorResponse {
        code: NO_ERROR_CODE.to_string(),
        message: message.clone(),
        reason: Some(message),
        status_code: http_code,
        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(attempt_status)),
        connector_transaction_id: None,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
    })
}

fn success_transaction_response(
    transaction_id: String,
    payment_id: Option<String>,
    mandate_reference: Option<MandateReference>,
    http_code: u16,
) -> PaymentsResponseData {
    let connector_metadata = payment_id
        .map(|payment_id| serde_json::json!(TesouroTransactionMetadata { payment_id }));
    PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
        redirection_data: None,
        mandate_reference: mandate_reference.map(Box::new),
        connector_metadata,
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code: http_code,
        splits: None,
    }
}

// =============================================================================
// SetupMandate response
// =============================================================================

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroVerifyAccountData {
    verify_account: TesouroVerifyAccountResponse,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroVerifyAccountResponse {
    verify_account_response: Option<VerifyAccountResponseType>,
    errors: Option<Vec<TesouroTransactionErrorData>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyAccountResponseType {
    pub payment_id: String,
    pub transaction_id: String,
    pub token_details: TesouroTokenDetails,
    pub activity_date: String,
}

pub type TesouroSetupMandateResponse = TesouroApiResponse<TesouroVerifyAccountData>;

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TesouroSetupMandateResponse, Self>>
    for RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<TesouroSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = match item.response {
            TesouroApiResponse::Success(success) => {
                let verify_account = success.data.verify_account;
                if let Some(verify_response) = verify_account.verify_account_response {
                    let mandate_reference = build_mandate_reference(
                        Some(verify_response.token_details),
                        Some(verify_response.payment_id.clone()),
                        Some(verify_response.activity_date),
                    );
                    (
                        AttemptStatus::Charged,
                        Ok(success_transaction_response(
                            verify_response.transaction_id,
                            Some(verify_response.payment_id),
                            mandate_reference,
                            item.http_code,
                        )),
                    )
                } else if let Some(errors) = verify_account.errors {
                    (
                        AttemptStatus::Failure,
                        in_band_error(&errors, item.http_code, AttemptStatus::Failure),
                    )
                } else {
                    return Err(ConnectorError::unexpected_response_error(item.http_code)
                    .into());
                }
            }
            TesouroApiResponse::Error(error_response) => (
                AttemptStatus::Failure,
                top_level_error(&error_response, item.http_code, AttemptStatus::Failure),
            ),
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// Authorize / RepeatPayment shared response (authorizationResponse)
// =============================================================================

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroAuthorizeData {
    #[serde(
        alias = "authorizeRecurring",
        alias = "authorizeCustomerInitiatedTransaction"
    )]
    authorize_customer_initiated_transaction: AuthorizationInnerResponse,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationInnerResponse {
    authorization_response: Option<AuthorizationResponseData>,
    errors: Option<Vec<TesouroTransactionErrorData>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationResponseData {
    #[serde(rename = "__typename")]
    pub type_name: Option<AuthorizeTransactionResponseType>,
    pub payment_id: Option<String>,
    pub transaction_id: String,
    pub message: Option<String>,
    pub token_details: Option<TesouroTokenDetails>,
    pub activity_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AuthorizeTransactionResponseType {
    AuthorizationApproval,
    AuthorizationDecline,
}

pub type TesouroAuthorizeResponse = TesouroApiResponse<TesouroAuthorizeData>;
/// Distinct alias for the RepeatPayment flow (same wire shape as Authorize) so the
/// per-flow response templating types generated by the macro do not collide.
pub type TesouroRepeatPaymentResponse = TesouroAuthorizeResponse;

#[allow(clippy::type_complexity)]
fn map_authorization_response(
    inner: AuthorizationInnerResponse,
    auto_capture: bool,
    http_code: u16,
) -> Result<(AttemptStatus, Result<PaymentsResponseData, ErrorResponse>), ResponseError> {
    if let Some(auth_response) = inner.authorization_response {
        let transaction_id = auth_response.transaction_id.clone();
        match auth_response.type_name {
            Some(AuthorizeTransactionResponseType::AuthorizationApproval) => {
                let status = if auto_capture {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                };
                let mandate_reference = build_mandate_reference(
                    auth_response.token_details,
                    auth_response.payment_id.clone(),
                    auth_response.activity_date,
                );
                Ok((
                    status,
                    Ok(success_transaction_response(
                        transaction_id,
                        auth_response.payment_id,
                        mandate_reference,
                        http_code,
                    )),
                ))
            }
            Some(AuthorizeTransactionResponseType::AuthorizationDecline) => {
                let status = if auto_capture {
                    AttemptStatus::Failure
                } else {
                    AttemptStatus::AuthorizationFailed
                };
                let message = auth_response
                    .message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
                Ok((
                    status,
                    Err(ErrorResponse {
                        code: NO_ERROR_CODE.to_string(),
                        message: message.clone(),
                        reason: Some(message),
                        status_code: http_code,
                        attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                            status,
                        )),
                        connector_transaction_id: Some(transaction_id),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                    }),
                ))
            }
            None => Ok((
                AttemptStatus::Pending,
                Ok(success_transaction_response(
                    transaction_id,
                    auth_response.payment_id,
                    None,
                    http_code,
                )),
            )),
        }
    } else if let Some(errors) = inner.errors {
        let status = if auto_capture {
            AttemptStatus::Failure
        } else {
            AttemptStatus::AuthorizationFailed
        };
        Ok((status, in_band_error(&errors, http_code, status)))
    } else {
        Err(ConnectorError::unexpected_response_error(http_code)
        .into())
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TesouroAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<TesouroAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let auto_capture = is_auto_capture(item.router_data.request.capture_method);
        let (status, response) = match item.response {
            TesouroApiResponse::Success(success) => map_authorization_response(
                success.data.authorize_customer_initiated_transaction,
                auto_capture,
                item.http_code,
            )?,
            TesouroApiResponse::Error(error_response) => {
                let status = if auto_capture {
                    AttemptStatus::Failure
                } else {
                    AttemptStatus::AuthorizationFailed
                };
                (
                    status,
                    top_level_error(&error_response, item.http_code, status),
                )
            }
        };
        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TesouroAuthorizeResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<TesouroAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let auto_capture = is_auto_capture(item.router_data.request.capture_method);
        let (status, response) = match item.response {
            TesouroApiResponse::Success(success) => map_authorization_response(
                success.data.authorize_customer_initiated_transaction,
                auto_capture,
                item.http_code,
            )?,
            TesouroApiResponse::Error(error_response) => {
                let status = if auto_capture {
                    AttemptStatus::Failure
                } else {
                    AttemptStatus::AuthorizationFailed
                };
                (
                    status,
                    top_level_error(&error_response, item.http_code, status),
                )
            }
        };
        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// PSync response
// =============================================================================

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroSyncData {
    payment_transaction: TesouroPaymentTransactionResponse,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TesouroPaymentTransactionResponse {
    pub id: String,
    #[serde(rename = "__typename")]
    pub typename: TesouroSyncStatus,
    pub processor_response_code: Option<String>,
    pub processor_response_message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum TesouroSyncStatus {
    AcceptedSale,
    ApprovedAuthorization,
    ApprovedCapture,
    ApprovedReversal,
    DeclinedAuthorization,
    DeclinedCapture,
    DeclinedReversal,
    GenericPaymentTransaction,
    Authorization,
    Capture,
    Reversal,
    Sale,
}

pub type TesouroSyncResponse = TesouroApiResponse<TesouroSyncData>;

fn map_sync_status(
    status: &TesouroSyncStatus,
    auto_capture: bool,
    previous_status: AttemptStatus,
) -> AttemptStatus {
    match status {
        TesouroSyncStatus::AcceptedSale
        | TesouroSyncStatus::ApprovedCapture
        | TesouroSyncStatus::Sale => AttemptStatus::Charged,
        TesouroSyncStatus::ApprovedAuthorization => {
            if auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        }
        TesouroSyncStatus::DeclinedAuthorization => {
            if auto_capture {
                AttemptStatus::AuthorizationFailed
            } else {
                AttemptStatus::Failure
            }
        }
        TesouroSyncStatus::ApprovedReversal => AttemptStatus::Voided,
        TesouroSyncStatus::DeclinedCapture => AttemptStatus::Failure,
        TesouroSyncStatus::DeclinedReversal => AttemptStatus::VoidFailed,
        TesouroSyncStatus::GenericPaymentTransaction => previous_status,
        TesouroSyncStatus::Authorization => AttemptStatus::Authorizing,
        TesouroSyncStatus::Capture => AttemptStatus::CaptureInitiated,
        TesouroSyncStatus::Reversal => AttemptStatus::VoidInitiated,
    }
}

impl TryFrom<ResponseRouterData<TesouroSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(item: ResponseRouterData<TesouroSyncResponse, Self>) -> Result<Self, Self::Error> {
        let previous_status = item.router_data.resource_common_data.status;
        let auto_capture = is_auto_capture(item.router_data.request.capture_method);
        let (status, response) = match item.response {
            TesouroApiResponse::Success(success) => {
                let payment_transaction = success.data.payment_transaction;
                let status =
                    map_sync_status(&payment_transaction.typename, auto_capture, previous_status);
                if domain_types::utils::is_payment_failure(status) {
                    let message = payment_transaction
                        .processor_response_message
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
                    (
                        status,
                        Err(ErrorResponse {
                            code: payment_transaction
                                .processor_response_code
                                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                            message: message.clone(),
                            reason: Some(message),
                            status_code: item.http_code,
                            attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                                status,
                            )),
                            connector_transaction_id: Some(payment_transaction.id),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                        }),
                    )
                } else {
                    (
                        status,
                        Ok(success_transaction_response(
                            payment_transaction.id,
                            None,
                            None,
                            item.http_code,
                        )),
                    )
                }
            }
            TesouroApiResponse::Error(error_response) => (
                previous_status,
                top_level_error(&error_response, item.http_code, previous_status),
            ),
        };
        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
