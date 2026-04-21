use std::fmt::Debug;

use common_enums::{AttemptStatus, CaptureMethod, Currency};
use common_utils::types::StringMinorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// ===== CONSTANTS =====
const TRUSTPAYMENTS_API_VERSION: &str = "1.00";
const TRUSTPAYMENTS_ACCOUNT_TYPE_ECOM: &str = "ECOM";
const TRUSTPAYMENTS_CREDENTIALS_ON_FILE: &str = "1";

// ===== ENUMS =====
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrustpaymentsRequestType {
    Auth,
    Transactionquery,
    Transactionupdate,
    Refund,
}

impl TrustpaymentsRequestType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Auth => "AUTH",
            Self::Transactionquery => "TRANSACTIONQUERY",
            Self::Transactionupdate => "TRANSACTIONUPDATE",
            Self::Refund => "REFUND",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrustpaymentsSettleStatus {
    /// Automatic capture - will be settled automatically
    #[serde(rename = "0")]
    AutomaticCapture,
    /// Settled and being processed
    #[serde(rename = "1")]
    SettledPending,
    /// Fully settled and completed
    #[serde(rename = "100")]
    SettledComplete,
    /// Manual capture - suspended, requires manual capture
    #[serde(rename = "2")]
    ManualCapture,
    /// Cancelled/Reversed
    #[serde(rename = "3")]
    Cancelled,
}

// ===== AUTHENTICATION =====
// Trust Payments requires 3 credentials:
// 1. username - for Basic Auth and as "alias" in request
// 2. password - for Basic Auth
// 3. site_reference - for "sitereference" in request body
#[derive(Debug, Clone)]
pub struct TrustpaymentsAuthType {
    pub username: Secret<String>,       // api_key → username/alias
    pub password: Secret<String>,       // key1 → password
    pub site_reference: Secret<String>, // api_secret → sitereference
}

impl TrustpaymentsAuthType {
    pub fn generate_basic_auth(&self) -> Secret<String> {
        let credentials = format!("{}:{}", self.username.peek(), self.password.peek());
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            credentials.as_bytes(),
        );
        Secret::new(format!("Basic {encoded}"))
    }
}

impl TryFrom<&ConnectorSpecificConfig> for TrustpaymentsAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Trustpayments {
                username,
                password,
                site_reference,
                ..
            } => Ok(Self {
                username: username.to_owned(),
                password: password.to_owned(),
                site_reference: site_reference.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?,
        }
    }
}

// ===== ERROR RESPONSE =====
// Trust Payments returns errors in the same structure as success responses
// but uses "response" instead of "responses" in some cases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustpaymentsErrorResponse {
    pub requestreference: Option<String>,
    pub version: Option<String>,
    pub responses: Option<Vec<TrustpaymentsErrorResponseItem>>,
    // Trust Payments API inconsistently uses "response" vs "responses"
    pub response: Option<Vec<TrustpaymentsErrorResponseItem>>,
    // For simple error messages
    pub errorcode: Option<String>,
    pub errormessage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustpaymentsErrorResponseItem {
    pub errorcode: String,
    pub errormessage: String,
    pub transactionreference: Option<String>,
    pub requesttypedescription: Option<String>,
}

// ===== AUTHORIZE REQUEST =====
#[derive(Debug, Serialize)]
pub struct TrustpaymentsAuthorizeRequest {
    pub alias: String,
    pub version: String,
    pub request: Vec<TrustpaymentsAuthRequest>,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsAuthRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accounttypedescription: Option<String>,
    pub baseamount: StringMinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billingfirstname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billinglastname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentialsonfile: Option<String>,
    pub currencyiso3a: Currency,
    pub orderreference: String,
    pub requesttypedescriptions: Vec<TrustpaymentsRequestType>,
    pub sitereference: Secret<String>,
    pub settlestatus: String,
    #[serde(flatten)]
    pub payment_method: TrustpaymentsPaymentMethod,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum TrustpaymentsPaymentMethod {
    Card(TrustpaymentsCardData),
    GooglePay(Box<TrustpaymentsGooglePayData>),
    ApplePay(Box<TrustpaymentsApplePayData>),
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsCardData {
    pub pan: Secret<String>,
    pub expirydate: Secret<String>,
    pub securitycode: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsGooglePayData {
    pub pan: Secret<String>,
    pub expirydate: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenisedpayment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokentype: Option<String>,
    pub walletsource: String,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsApplePayData {
    pub pan: Secret<String>,
    pub expirydate: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenisedpayment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokentype: Option<String>,
    pub walletdisplayname: String,
    pub walletsource: String,
}

// ===== AUTHORIZE RESPONSE =====
#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsAuthorizeResponse {
    pub requestreference: String,
    pub version: String,
    // Trust Payments API inconsistently uses "response" and "responses"
    #[serde(alias = "response")]
    pub responses: Vec<TrustpaymentsAuthResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsAuthResponse {
    pub errorcode: String,
    pub errormessage: String,
    pub transactionreference: Option<String>,
    pub authcode: Option<String>,
    pub baseamount: Option<StringMinorUnit>,
    pub currencyiso3a: Option<Currency>,
    pub settlestatus: Option<TrustpaymentsSettleStatus>,
    pub requesttypedescription: String,
    pub paymenttypedescription: Option<String>,
    pub maskedpan: Option<Secret<String>>,
}

// ===== REQUEST TRANSFORMER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::TrustpaymentsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TrustpaymentsAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::TrustpaymentsRouterData<
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

        // Extract auth credentials for alias and sitereference
        let auth = TrustpaymentsAuthType::try_from(&router_data.connector_config)?;

        // Extract payment method data
        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Serialize to get the string representation (needed due to generic type constraints)
                let card_number_json =
                    serde_json::to_value(&card_data.card_number.0).map_err(|_| {
                        IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        }
                    })?;
                let card_number_string = card_number_json
                    .as_str()
                    .ok_or(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    })?
                    .to_string();

                // Format expiry date as MM/YY (Trust Payments requires 2-digit year)
                let expiry_date =
                    card_data.get_card_expiry_month_year_2_digit_with_delimiter("/".to_string())?;
                TrustpaymentsPaymentMethod::Card(TrustpaymentsCardData {
                    pan: Secret::new(card_number_string),
                    expirydate: expiry_date,
                    securitycode: card_data.card_cvc.clone(),
                })
            }
            PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => {
                let decrypted_data = match &google_pay_data.tokenization_data {
                    GpayTokenizationData::Decrypted(data) => data,
                    GpayTokenizationData::Encrypted(_) => {
                        return Err(error_stack::report!(IntegrationError::InvalidWalletToken {
                            wallet_name: "Google Pay".to_string(),
                            context: Default::default(),
                        }))
                    }
                };

                let four_digit_year = decrypted_data.get_four_digit_expiry_year().change_context(
                    IntegrationError::InvalidWalletToken {
                        wallet_name: "Google Pay".to_string(),
                        context: Default::default(),
                    },
                )?;
                let month = decrypted_data.get_expiry_month().change_context(
                    IntegrationError::InvalidWalletToken {
                        wallet_name: "Google Pay".to_string(),
                        context: Default::default(),
                    },
                )?;
                // TrustPayments expects MM/YYYY format
                let expirydate =
                    Secret::new(format!("{}/{}", month.peek(), four_digit_year.peek()));

                let is_cryptogram_3ds = decrypted_data.cryptogram.is_some();

                TrustpaymentsPaymentMethod::GooglePay(Box::new(TrustpaymentsGooglePayData {
                    pan: Secret::new(
                        decrypted_data
                            .application_primary_account_number
                            .get_card_no(),
                    ),
                    expirydate,
                    // CRYPTOGRAM_3DS: send tavv (cryptogram) + tokenisedpayment + tokentype
                    tavv: if is_cryptogram_3ds {
                        decrypted_data.cryptogram.clone()
                    } else {
                        None
                    },
                    // For PAN_ONLY, eci is required by TrustPayments; default to "06" if absent.
                    // Google Pay PAN_ONLY decrypted payloads never include eciIndicator (by spec).
                    // Ref: https://github.com/juspay/hyperswitch-prism/issues/894
                    eci: Some(
                        decrypted_data
                            .eci_indicator
                            .clone()
                            .unwrap_or_else(|| "06".to_string()),
                    ),
                    tokenisedpayment: if is_cryptogram_3ds {
                        Some("1".to_string())
                    } else {
                        None
                    },
                    tokentype: if is_cryptogram_3ds {
                        Some("GOOGLEPAY".to_string())
                    } else {
                        None
                    },
                    walletsource: "GOOGLEPAY".to_string(),
                }))
            }
            PaymentMethodData::Wallet(WalletData::ApplePay(apple_pay_data)) => {
                // Trust Payments has no native encrypted Apple Pay endpoint; follow
                // the decrypted-passthrough pattern and submit the decrypted DPAN
                // as a wallet-sourced card transaction (walletsource=APPLEPAY).
                const WALLET_SOURCE: &str = "APPLEPAY";
                // ECI "07" = Apple Pay non-3DS (no 3DS challenge, liability shift via TAVV).
                const DEFAULT_ECI: &str = "07";
                // tokenisedpayment "1" = Trust Payments flag indicating a tokenised/wallet payment.
                const TOKENISED_PAYMENT_ENABLED: &str = "1";

                let apple_pay_decrypted_data = apple_pay_data
                    .payment_data
                    .get_decrypted_apple_pay_payment_data_optional()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "apple_pay_decrypted_data",
                            context: Default::default(),
                        })
                        .attach_printable(
                            "Trust Payments requires pre-decrypted Apple Pay data; \
                             encrypted Apple Pay tokens are not supported.",
                        )
                    })?;

                // Trust Payments expects MM/YYYY format
                let expirydate = apple_pay_decrypted_data.get_expiry_date_as_mmyyyy("/");

                let pan = Secret::new(
                    apple_pay_decrypted_data
                        .application_primary_account_number
                        .get_card_no(),
                );

                let cryptogram = apple_pay_decrypted_data
                    .payment_data
                    .online_payment_cryptogram
                    .clone();
                let is_cryptogram_3ds = !cryptogram.peek().is_empty();
                let eci = apple_pay_decrypted_data.payment_data.eci_indicator.clone();

                TrustpaymentsPaymentMethod::ApplePay(Box::new(TrustpaymentsApplePayData {
                    pan,
                    expirydate,
                    tavv: if is_cryptogram_3ds {
                        Some(cryptogram)
                    } else {
                        None
                    },
                    // For non-cryptogram flows, Trust Payments requires an ECI value;
                    // default to DEFAULT_ECI (Apple Pay non-3DS) if absent.
                    eci: Some(eci.unwrap_or_else(|| DEFAULT_ECI.to_string())),
                    tokenisedpayment: if is_cryptogram_3ds {
                        Some(TOKENISED_PAYMENT_ENABLED.to_string())
                    } else {
                        None
                    },
                    tokentype: if is_cryptogram_3ds {
                        Some(WALLET_SOURCE.to_string())
                    } else {
                        None
                    },
                    walletdisplayname: apple_pay_data.payment_method.display_name.clone(),
                    walletsource: WALLET_SOURCE.to_string(),
                }))
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::not_implemented(
                    "Payment method not supported".to_string()
                )))
            }
        };

        // Extract billing name using router data utility functions
        let first_name = router_data
            .resource_common_data
            .get_optional_billing_first_name();

        let last_name = router_data
            .resource_common_data
            .get_optional_billing_last_name();

        // Get amount from connector's amount_converter
        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .map_err(|_| IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // Determine settlestatus based on capture method
        let settlestatus = match router_data.request.capture_method {
            Some(CaptureMethod::Manual) => TrustpaymentsSettleStatus::ManualCapture,
            // All other variants default to automatic capture
            Some(_) | None => TrustpaymentsSettleStatus::AutomaticCapture,
        };

        let auth_request = TrustpaymentsAuthRequest {
            accounttypedescription: Some(TRUSTPAYMENTS_ACCOUNT_TYPE_ECOM.to_string()),
            baseamount: amount,
            billingfirstname: first_name,
            billinglastname: last_name,
            credentialsonfile: Some(TRUSTPAYMENTS_CREDENTIALS_ON_FILE.to_string()),
            currencyiso3a: router_data.request.currency,
            orderreference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            requesttypedescriptions: vec![TrustpaymentsRequestType::Auth],
            sitereference: auth.site_reference.clone(),
            settlestatus: serde_json::to_value(&settlestatus)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "0".to_string()),
            payment_method,
        };

        Ok(Self {
            alias: auth.username.expose(),
            version: TRUSTPAYMENTS_API_VERSION.to_string(),
            request: vec![auth_request],
        })
    }
}

// ===== RESPONSE TRANSFORMER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TrustpaymentsAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpaymentsAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get the first response from the array
        let response = item.response.responses.first().ok_or(
            crate::utils::response_handling_fail_for_connector(item.http_code, "trustpayments"),
        )?;

        // Check for errors
        if response.errorcode != "0" {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: response.errorcode.clone(),
                    message: response.errormessage.clone(),
                    reason: Some(response.errormessage.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.transactionreference.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Map status based on settlestatus using helper function
        let status = get_status_from_settlestatus(
            response.settlestatus.as_ref(),
            response.authcode.as_deref(),
        );

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                response
                    .transactionreference
                    .clone()
                    .unwrap_or(item.response.requestreference.clone()),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(item.response.requestreference.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ===== PSYNC REQUEST =====
#[derive(Debug, Serialize)]
pub struct TrustpaymentsPSyncRequest {
    pub alias: String,
    pub version: String,
    pub request: Vec<TrustpaymentsPSyncRequestItem>,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsPSyncRequestItem {
    pub requesttypedescriptions: Vec<TrustpaymentsRequestType>,
    pub filter: TrustpaymentsFilter,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsFilter {
    pub sitereference: Vec<TrustpaymentsFilterValue>,
    pub transactionreference: Vec<TrustpaymentsFilterValue>,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsFilterValue {
    pub value: String,
}

// ===== PSYNC RESPONSE =====
#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsPSyncResponse {
    pub requestreference: String,
    pub version: String,
    // Trust Payments API inconsistently uses "response" and "responses"
    #[serde(alias = "responses")]
    pub response: Vec<TrustpaymentsPSyncResponseItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsPSyncResponseItem {
    pub errorcode: String,
    pub errormessage: String,
    pub found: Option<String>,
    pub records: Option<Vec<TrustpaymentsTransactionRecord>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsTransactionRecord {
    pub errorcode: String,
    pub errormessage: String,
    pub transactionreference: String,
    pub authcode: Option<String>,
    pub baseamount: Option<StringMinorUnit>,
    pub currencyiso3a: Option<Currency>,
    pub settlestatus: Option<TrustpaymentsSettleStatus>,
    pub requesttypedescription: String,
    pub paymenttypedescription: Option<String>,
    pub maskedpan: Option<Secret<String>>,
}

// ===== PSYNC REQUEST TRANSFORMER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::TrustpaymentsRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for TrustpaymentsPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::TrustpaymentsRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth credentials for alias and sitereference
        let auth = TrustpaymentsAuthType::try_from(&router_data.connector_config)?;

        // Extract transaction reference from connector_transaction_id
        let transaction_reference =
            router_data
                .request
                .get_connector_transaction_id()
                .map_err(|_| IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;

        let filter = TrustpaymentsFilter {
            sitereference: vec![TrustpaymentsFilterValue {
                value: auth.site_reference.peek().to_string(),
            }],
            transactionreference: vec![TrustpaymentsFilterValue {
                value: transaction_reference,
            }],
        };

        let request_item = TrustpaymentsPSyncRequestItem {
            requesttypedescriptions: vec![TrustpaymentsRequestType::Transactionquery],
            filter,
        };

        Ok(Self {
            alias: auth.username.expose(),
            version: TRUSTPAYMENTS_API_VERSION.to_string(),
            request: vec![request_item],
        })
    }
}

// ===== STATUS MAPPING HELPER =====
fn get_status_from_settlestatus(
    settlestatus: Option<&TrustpaymentsSettleStatus>,
    authcode: Option<&str>,
) -> AttemptStatus {
    match settlestatus {
        Some(TrustpaymentsSettleStatus::AutomaticCapture) => {
            // Automatic capture - pending settlement, will be auto-settled
            if authcode.is_some() {
                AttemptStatus::Charged // Authorized and will be captured automatically
            } else {
                AttemptStatus::Pending
            }
        }
        Some(TrustpaymentsSettleStatus::SettledPending) => AttemptStatus::Charged, // Settled but being processed
        Some(TrustpaymentsSettleStatus::SettledComplete) => AttemptStatus::Charged, // Fully settled
        Some(TrustpaymentsSettleStatus::ManualCapture) => {
            // Manual capture - suspended, requires manual capture
            if authcode.is_some() {
                AttemptStatus::Authorized // Authorized but needs manual capture
            } else {
                AttemptStatus::Pending
            }
        }
        Some(TrustpaymentsSettleStatus::Cancelled) => AttemptStatus::Voided, // Cancelled/Reversed
        None => AttemptStatus::Pending,
    }
}

// ===== PSYNC RESPONSE TRANSFORMER =====
impl TryFrom<ResponseRouterData<TrustpaymentsPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpaymentsPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get the first response from the array
        let response_item = item.response.response.first().ok_or(
            crate::utils::response_handling_fail_for_connector(item.http_code, "trustpayments"),
        )?;

        // Check for errors at the response level
        if response_item.errorcode != "0" {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: response_item.errorcode.clone(),
                    message: response_item.errormessage.clone(),
                    reason: Some(response_item.errormessage.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Get the first record from the records array
        let record = response_item
            .records
            .as_ref()
            .and_then(|records| records.first())
            .ok_or(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "trustpayments",
            ))?;

        // Check for errors at the record level
        if record.errorcode != "0" {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: record.errorcode.clone(),
                    message: record.errormessage.clone(),
                    reason: Some(record.errormessage.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(record.transactionreference.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Map status based on settlestatus
        let status =
            get_status_from_settlestatus(record.settlestatus.as_ref(), record.authcode.as_deref());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(record.transactionreference.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(item.response.requestreference.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ===== CAPTURE REQUEST =====
#[derive(Debug, Serialize)]
pub struct TrustpaymentsCaptureRequest {
    pub alias: String,
    pub version: String,
    pub request: Vec<TrustpaymentsCaptureRequestItem>,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsCaptureRequestItem {
    pub requesttypedescriptions: Vec<TrustpaymentsRequestType>,
    pub filter: TrustpaymentsFilter,
    pub updates: TrustpaymentsCaptureUpdates,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsCaptureUpdates {
    pub settlestatus: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseamount: Option<StringMinorUnit>,
}

// ===== CAPTURE RESPONSE =====
#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsCaptureResponse {
    pub requestreference: String,
    pub version: String,
    // Trust Payments API inconsistently uses "response" and "responses"
    #[serde(alias = "responses")]
    pub response: Vec<TrustpaymentsCaptureResponseItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsCaptureResponseItem {
    pub errorcode: String,
    pub errormessage: String,
    pub requesttypedescription: String,
    pub transactionstartedtimestamp: Option<String>,
    pub operatorname: Option<String>,
}

// ===== CAPTURE REQUEST TRANSFORMER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::TrustpaymentsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TrustpaymentsCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::TrustpaymentsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth credentials for alias and sitereference
        let auth = TrustpaymentsAuthType::try_from(&router_data.connector_config)?;

        // Extract transaction reference from connector_transaction_id
        let transaction_reference =
            router_data
                .request
                .get_connector_transaction_id()
                .map_err(|_| IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;

        let filter = TrustpaymentsFilter {
            sitereference: vec![TrustpaymentsFilterValue {
                value: auth.site_reference.peek().to_string(),
            }],
            transactionreference: vec![TrustpaymentsFilterValue {
                value: transaction_reference,
            }],
        };

        // Trust Payments TRANSACTIONUPDATE for capture only needs settlestatus change
        // Do NOT send baseamount - it causes "Invalid updates specified" error
        // The full authorized amount will be captured automatically
        let settlestatus = TrustpaymentsSettleStatus::AutomaticCapture;
        let updates = TrustpaymentsCaptureUpdates {
            settlestatus: serde_json::to_value(&settlestatus)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "0".to_string()),
            baseamount: None, // Never send amount for Trust Payments captures
        };

        let request_item = TrustpaymentsCaptureRequestItem {
            requesttypedescriptions: vec![TrustpaymentsRequestType::Transactionupdate],
            filter,
            updates,
        };

        Ok(Self {
            alias: auth.username.expose(),
            version: TRUSTPAYMENTS_API_VERSION.to_string(),
            request: vec![request_item],
        })
    }
}

// ===== CAPTURE RESPONSE TRANSFORMER =====
impl TryFrom<ResponseRouterData<TrustpaymentsCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpaymentsCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get the first response from the array
        let response_item = item.response.response.first().ok_or(
            crate::utils::response_handling_fail_for_connector(item.http_code, "trustpayments"),
        )?;

        // Check for errors
        if response_item.errorcode != "0" {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: response_item.errorcode.clone(),
                    message: response_item.errormessage.clone(),
                    reason: Some(response_item.errormessage.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Successful capture - TRANSACTIONUPDATE returns success
        // The actual settlement status should be verified via TRANSACTIONQUERY (PSync)
        // For now, we mark as Charged since the capture was accepted
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                router_data
                    .request
                    .get_connector_transaction_id()
                    .unwrap_or_else(|_| item.response.requestreference.clone()),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(item.response.requestreference.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ===== VOID REQUEST =====
#[derive(Debug, Serialize)]
pub struct TrustpaymentsVoidRequest {
    pub alias: String,
    pub version: String,
    pub request: Vec<TrustpaymentsVoidRequestItem>,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsVoidRequestItem {
    pub requesttypedescriptions: Vec<TrustpaymentsRequestType>,
    pub filter: TrustpaymentsFilter,
    pub updates: TrustpaymentsVoidUpdates,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsVoidUpdates {
    pub settlestatus: String,
}

// ===== VOID RESPONSE =====
// Void response has the same structure as Capture response
pub type TrustpaymentsVoidResponse = TrustpaymentsCaptureResponse;

// ===== VOID REQUEST TRANSFORMER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::TrustpaymentsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TrustpaymentsVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::TrustpaymentsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth credentials for alias and sitereference
        let auth = TrustpaymentsAuthType::try_from(&router_data.connector_config)?;

        // Extract transaction reference from connector_transaction_id
        let transaction_reference = router_data.request.connector_transaction_id.clone();

        let filter = TrustpaymentsFilter {
            sitereference: vec![TrustpaymentsFilterValue {
                value: auth.site_reference.peek().to_string(),
            }],
            transactionreference: vec![TrustpaymentsFilterValue {
                value: transaction_reference,
            }],
        };

        // settlestatus="3" means Cancelled/Reversed (Void)
        let settlestatus = TrustpaymentsSettleStatus::Cancelled;
        let updates = TrustpaymentsVoidUpdates {
            settlestatus: serde_json::to_value(&settlestatus)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "3".to_string()),
        };

        let request_item = TrustpaymentsVoidRequestItem {
            requesttypedescriptions: vec![TrustpaymentsRequestType::Transactionupdate],
            filter,
            updates,
        };

        Ok(Self {
            alias: auth.username.expose(),
            version: TRUSTPAYMENTS_API_VERSION.to_string(),
            request: vec![request_item],
        })
    }
}

// ===== VOID RESPONSE TRANSFORMER =====
impl TryFrom<ResponseRouterData<TrustpaymentsVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpaymentsVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get the first response from the array
        let response_item = item.response.response.first().ok_or(
            crate::utils::response_handling_fail_for_connector(item.http_code, "trustpayments"),
        )?;

        // Check for errors
        if response_item.errorcode != "0" {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: response_item.errorcode.clone(),
                    message: response_item.errormessage.clone(),
                    reason: Some(response_item.errormessage.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::VoidFailed),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Successful void - TRANSACTIONUPDATE returns success
        // Mark as Voided since the void was accepted
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                router_data.request.connector_transaction_id.clone(),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(item.response.requestreference.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ===== REFUND REQUEST =====
#[derive(Debug, Serialize)]
pub struct TrustpaymentsRefundRequest {
    pub alias: String,
    pub version: String,
    pub request: Vec<TrustpaymentsRefundRequestItem>,
}

#[derive(Debug, Serialize)]
pub struct TrustpaymentsRefundRequestItem {
    pub requesttypedescriptions: Vec<TrustpaymentsRequestType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseamount: Option<StringMinorUnit>,
    pub currencyiso3a: Currency,
    pub parenttransactionreference: String,
    pub sitereference: Secret<String>,
}

// ===== REFUND RESPONSE =====
#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsRefundResponse {
    pub requestreference: String,
    // Trust Payments API inconsistently uses "response" and "responses"
    #[serde(alias = "response")]
    pub responses: Vec<TrustpaymentsRefundResponseItem>,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TrustpaymentsRefundResponseItem {
    pub errorcode: String,
    pub errormessage: String,
    pub transactionreference: Option<String>,
    pub authcode: Option<String>,
    pub baseamount: Option<StringMinorUnit>,
    pub currencyiso3a: Option<Currency>,
    pub settlestatus: Option<TrustpaymentsSettleStatus>,
    pub requesttypedescription: String,
    pub paymenttypedescription: Option<String>,
    pub parenttransactionreference: Option<String>,
}

// ===== REFUND REQUEST TRANSFORMER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::TrustpaymentsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TrustpaymentsRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::TrustpaymentsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth credentials for alias and sitereference
        let auth = TrustpaymentsAuthType::try_from(&router_data.connector_config)?;

        // Extract parent transaction reference from connector_transaction_id
        let parent_transaction_reference = router_data.request.connector_transaction_id.clone();

        // Check if this is a partial refund
        // For partial refunds, include baseamount; for full refunds, omit it
        let base_amount = if router_data.request.minor_refund_amount.get_amount_as_i64() > 0 {
            // Partial refund - include the amount
            let amount = item
                .connector
                .amount_converter
                .convert(
                    router_data.request.minor_refund_amount,
                    router_data.request.currency,
                )
                .map_err(|_| IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;
            Some(amount)
        } else {
            // Full refund - no amount needed
            None
        };

        let refund_request_item = TrustpaymentsRefundRequestItem {
            requesttypedescriptions: vec![TrustpaymentsRequestType::Refund],
            baseamount: base_amount,
            currencyiso3a: router_data.request.currency,
            parenttransactionreference: parent_transaction_reference,
            sitereference: auth.site_reference.clone(),
        };

        Ok(Self {
            alias: auth.username.expose(),
            version: TRUSTPAYMENTS_API_VERSION.to_string(),
            request: vec![refund_request_item],
        })
    }
}

// ===== REFUND STATUS MAPPING HELPER =====
fn get_refund_status_from_settlestatus(
    settlestatus: Option<&TrustpaymentsSettleStatus>,
    errorcode: &str,
) -> common_enums::RefundStatus {
    use common_enums::RefundStatus;

    // If errorcode is not "0", it's a failure
    if errorcode != "0" {
        return RefundStatus::Failure;
    }

    // Map settlestatus to refund status
    match settlestatus {
        Some(TrustpaymentsSettleStatus::SettledComplete) => RefundStatus::Success, // Fully settled
        Some(TrustpaymentsSettleStatus::AutomaticCapture) => RefundStatus::Pending, // Pending settlement
        Some(TrustpaymentsSettleStatus::SettledPending) => RefundStatus::Success, // Being processed (settled)
        Some(TrustpaymentsSettleStatus::ManualCapture) => RefundStatus::ManualReview, // Suspended
        Some(TrustpaymentsSettleStatus::Cancelled) => RefundStatus::Failure, // Cancelled/Reversed
        None => RefundStatus::Pending,
    }
}

// ===== RSYNC REQUEST/RESPONSE (Reuse TRANSACTIONQUERY structures from PSync) =====
// RSync uses the same TRANSACTIONQUERY endpoint as PSync, so we reuse the structures
pub type TrustpaymentsRSyncRequest = TrustpaymentsPSyncRequest;
pub type TrustpaymentsRSyncResponse = TrustpaymentsPSyncResponse;

// ===== RSYNC REQUEST TRANSFORMER =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::TrustpaymentsRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for TrustpaymentsRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::TrustpaymentsRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth credentials for alias and sitereference
        let auth = TrustpaymentsAuthType::try_from(&router_data.connector_config)?;

        // Extract refund transaction reference from connector_refund_id
        let refund_transaction_reference = router_data.request.connector_refund_id.clone();

        let filter = TrustpaymentsFilter {
            sitereference: vec![TrustpaymentsFilterValue {
                value: auth.site_reference.peek().to_string(),
            }],
            transactionreference: vec![TrustpaymentsFilterValue {
                value: refund_transaction_reference,
            }],
        };

        let request_item = TrustpaymentsPSyncRequestItem {
            requesttypedescriptions: vec![TrustpaymentsRequestType::Transactionquery],
            filter,
        };

        Ok(Self {
            alias: auth.username.expose(),
            version: TRUSTPAYMENTS_API_VERSION.to_string(),
            request: vec![request_item],
        })
    }
}

// ===== RSYNC RESPONSE TRANSFORMER =====
impl TryFrom<ResponseRouterData<TrustpaymentsRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpaymentsRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get the first response from the array
        let response_item = item.response.response.first().ok_or(
            crate::utils::response_handling_fail_for_connector(item.http_code, "trustpayments"),
        )?;

        // Check for errors at the response level
        if response_item.errorcode != "0" {
            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: response_item.errorcode.clone(),
                    message: response_item.errormessage.clone(),
                    reason: Some(response_item.errormessage.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Get the first record from the records array
        let record = response_item
            .records
            .as_ref()
            .and_then(|records| records.first())
            .ok_or(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "trustpayments",
            ))?;

        // Check for errors at the record level
        if record.errorcode != "0" {
            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: record.errorcode.clone(),
                    message: record.errormessage.clone(),
                    reason: Some(record.errormessage.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(record.transactionreference.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Map refund status using the shared helper function
        let refund_status =
            get_refund_status_from_settlestatus(record.settlestatus.as_ref(), &record.errorcode);

        let refunds_response_data = RefundsResponseData {
            connector_refund_id: record.transactionreference.clone(),
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// ===== REFUND RESPONSE TRANSFORMER =====
impl TryFrom<ResponseRouterData<TrustpaymentsRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TrustpaymentsRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get the first response from the array
        let response = item.response.responses.first().ok_or(
            crate::utils::response_handling_fail_for_connector(item.http_code, "trustpayments"),
        )?;

        // Map refund status
        let refund_status = get_refund_status_from_settlestatus(
            response.settlestatus.as_ref(),
            &response.errorcode,
        );

        // Extract connector refund ID
        let connector_refund_id = response
            .transactionreference
            .clone()
            .unwrap_or_else(|| item.response.requestreference.clone());

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}
