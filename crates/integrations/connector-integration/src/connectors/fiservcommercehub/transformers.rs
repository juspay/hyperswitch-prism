use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::ResponseRouterData;
use base64::{engine::general_purpose, Engine};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    crypto::{self, RsaOaepSha256, SignMessage},
    FloatMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, RSync, Refund, RepeatPayment, ServerAuthenticationToken,
        SetupMandate, Void,
    },
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        SetupMandateRequestData,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// Constants for encryption and token formatting
pub(crate) const ENCRYPTION_TYPE_RSA: &str = "RSA";
pub(crate) const ACCESS_TOKEN_SEPARATOR: &str = "|||";

/// Helper struct holding encrypted card data for Fiserv Commerce Hub
#[derive(Debug)]
pub struct EncryptedCardData {
    pub key_id: String,
    pub encryption_block: Secret<String>,
    pub encryption_block_fields: String,
}

/// Encrypts card data using RSA-OAEP-SHA256 for Fiserv Commerce Hub
///
/// # Arguments
/// * `card` - The card data to encrypt
/// * `key_id` - The encryption key ID from access token
/// * `public_key_der` - The DER-encoded RSA public key
///
/// # Returns
/// * `Ok(EncryptedCardData)` - The encrypted card data structure
/// * `Err` - If encryption fails or required fields are missing
fn encrypt_card_data<T: PaymentMethodDataTypes>(
    card: &domain_types::payment_method_data::Card<T>,
    key_id: String,
    public_key_der: &[u8],
) -> Result<EncryptedCardData, error_stack::Report<errors::IntegrationError>> {
    let card_data = card.card_number.peek().to_string();
    let name_on_card = card
        .card_holder_name
        .as_ref()
        .map(|n| n.peek().clone())
        .ok_or(errors::IntegrationError::MissingRequiredField {
            field_name: "card_holder_name",
            context: Default::default(),
        })?;
    let expiration_month = card.card_exp_month.peek().to_string();
    let expiration_year = card.get_expiry_year_4_digit().peek().to_string();

    let plain_block = format!("{card_data}{name_on_card}{expiration_month}{expiration_year}");

    let card_data_len = card_data.len();
    let name_on_card_len = name_on_card.len();
    let expiration_month_len = expiration_month.len();
    let expiration_year_len = expiration_year.len();
    let encryption_block_fields = format!(
        "card.cardData:{card_data_len},card.nameOnCard:{name_on_card_len},card.expirationMonth:{expiration_month_len},card.expirationYear:{expiration_year_len}"
    );

    let encrypted_bytes = RsaOaepSha256::encrypt(public_key_der, plain_block.as_bytes())
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("RSA OAEP-SHA256 encryption of card data failed")?;

    let encryption_block = Secret::new(general_purpose::STANDARD.encode(&encrypted_bytes));

    Ok(EncryptedCardData {
        key_id,
        encryption_block,
        encryption_block_fields,
    })
}

#[derive(Debug, Clone)]
pub struct FiservcommercehubAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
    pub merchant_id: Secret<String>,
    pub terminal_id: Secret<String>,
}

impl FiservcommercehubAuthType {
    pub fn generate_hmac_signature(
        &self,
        api_key: &str,
        client_request_id: &str,
        timestamp: &str,
        request_body: &str,
    ) -> Result<String, error_stack::Report<errors::IntegrationError>> {
        let raw_signature = format!("{api_key}{client_request_id}{timestamp}{request_body}");
        let signature = crypto::HmacSha256
            .sign_message(self.api_secret.peek().as_bytes(), raw_signature.as_bytes())
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        Ok(general_purpose::STANDARD.encode(signature))
    }

    pub fn generate_client_request_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    pub fn generate_timestamp() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string()
    }

    /// Builds the HMAC-authenticated headers for Fiserv Commerce Hub API requests.
    /// This is a common function used by all flows to generate the standard headers
    /// including Content-Type, Api-Key, Timestamp, Client-Request-Id, Authorization,
    /// Auth-Token-Type, and Accept-Language.
    pub fn build_hmac_headers(
        &self,
        content_type: &str,
        request_body_str: &str,
    ) -> Result<Vec<(String, Maskable<String>)>, error_stack::Report<errors::IntegrationError>>
    {
        let api_key = self.api_key.peek().to_string();
        let client_request_id = Self::generate_client_request_id();
        let timestamp = Self::generate_timestamp();

        let authorization = self.generate_hmac_signature(
            &api_key,
            &client_request_id,
            &timestamp,
            request_body_str,
        )?;

        Ok(vec![
            (
                super::headers::CONTENT_TYPE.to_string(),
                Secret::new(content_type.to_string()).into_masked(),
            ),
            (
                super::headers::API_KEY.to_string(),
                Secret::new(api_key).into_masked(),
            ),
            (
                super::headers::TIMESTAMP.to_string(),
                Secret::new(timestamp).into_masked(),
            ),
            (
                super::headers::CLIENT_REQUEST_ID.to_string(),
                Secret::new(client_request_id).into_masked(),
            ),
            (
                super::headers::AUTHORIZATION.to_string(),
                Secret::new(authorization).into_masked(),
            ),
            (
                super::headers::AUTH_TOKEN_TYPE.to_string(),
                Secret::new(super::headers::AUTH_TOKEN_TYPE_HMAC.to_string()).into_masked(),
            ),
            (
                super::headers::ACCEPT_LANGUAGE.to_string(),
                Secret::new(super::headers::ACCEPT_LANGUAGE_EN.to_string()).into_masked(),
            ),
        ])
    }
}

impl TryFrom<&ConnectorSpecificConfig> for FiservcommercehubAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Fiservcommercehub {
                api_key,
                secret: api_secret,
                merchant_id,
                terminal_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
                merchant_id: merchant_id.to_owned(),
                terminal_id: terminal_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorResponse {
    pub gateway_response: Option<FiservcommercehubErrorGatewayResponse>,
    pub error: Vec<FiservcommercehubErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorGatewayResponse {
    pub transaction_state: Option<String>,
    pub transaction_processing_details: Option<FiservcommercehubErrorTxnDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubErrorTxnDetails {
    pub api_trace_id: Option<String>,
    pub transaction_id: Option<String>,
    pub order_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiservcommercehubErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
}

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAuthorizeRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub source: FiservcommercehubSourceData,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub transaction_details: FiservcommercehubTransactionDetailsReq,
    pub transaction_interaction: FiservcommercehubTransactionInteractionReq,
    /// Additional 3DS data for external 3DS authentication (when authentication_data is present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<FiservcommercehubAdditionalData3DS>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credentials: Option<FiservcommercehubStoredCredentials>,
}

#[derive(Debug, Serialize)]
pub struct FiservcommercehubAuthorizeAmount {
    pub currency: common_enums::Currency,
    pub total: FloatMajorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "sourceType")]
pub enum FiservcommercehubSourceData {
    /// Payment source using encrypted card data
    #[serde(rename = "PaymentCard")]
    PaymentCard {
        #[serde(rename = "encryptionData")]
        encryption_data: FiservcommercehubEncryptionData,
    },
    /// Payment source using tokenized card data
    #[serde(rename = "PaymentToken")]
    PaymentToken {
        #[serde(rename = "tokenData")]
        token_data: String,
        #[serde(rename = "tokenSource")]
        token_source: String,
        #[serde(rename = "declineDuplicates")]
        #[serde(skip_serializing_if = "Option::is_none")]
        decline_duplicates: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        card: Option<FiservcommercehubTokenCardInfo>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubEncryptionData {
    pub key_id: String,
    pub encryption_type: String,
    pub encryption_block: Secret<String>,
    pub encryption_block_fields: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTokenCardInfo {
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionDetailsReq {
    pub capture_flag: bool,
    pub merchant_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubOrigin {
    Ecom,
    Moto,
    Pos,
}

impl From<Option<&common_enums::PaymentChannel>> for FiservcommercehubOrigin {
    fn from(channel: Option<&common_enums::PaymentChannel>) -> Self {
        match channel {
            Some(common_enums::PaymentChannel::MailOrder)
            | Some(common_enums::PaymentChannel::TelephoneOrder) => Self::Moto,
            Some(common_enums::PaymentChannel::Ecommerce) | None => Self::Ecom,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionInteractionReq {
    pub origin: FiservcommercehubOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
}

// =============================================================================
// STORED CREDENTIALS STRUCTURES
// =============================================================================

/// Indicates whether it is a merchant-initiated transaction or explicitly
/// consented to by the customer.
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubStoredCredentialInitiator {
    /// Transaction initiated by the merchant (MIT)
    Merchant,
    /// Transaction explicitly consented to by the card holder (CIT)
    CardHolder,
}

/// Indicates if the transaction is FIRST or SUBSEQUENT.
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubStoredCredentialSequence {
    /// First transaction in a stored credential series
    First,
    /// Subsequent transaction using previously stored credentials
    Subsequent,
}

/// Reference: https://developer.fiserv.com/product/CommerceHub/docs/Payment-Methods/Tokenization/Stored-Credentials.mdx
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubStoredCredentials {
    /// Indicates whether it is a merchant-initiated transaction or
    /// explicitly consented to by the customer.
    /// Valid Values: MERCHANT, CARD_HOLDER
    pub initiator: FiservcommercehubStoredCredentialInitiator,
    /// Indicates if this is a scheduled transaction.
    // pub scheduled: bool,
    /// The transaction ID received from the initial transaction.
    /// Required when the sequence is SUBSEQUENT if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme_referenced_transaction_id: Option<String>,
    /// Indicates if the transaction is FIRST or SUBSEQUENT.
    pub sequence: FiservcommercehubStoredCredentialSequence,
    /// Original transaction amount. Required for Discover transactions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme_original_amount: Option<FloatMajorUnit>,
}

impl FiservcommercehubStoredCredentials {
    /// * `scheme_original_amount` - Optional original amount (required for Discover)
    pub fn new_cit() -> Self {
        Self {
            initiator: FiservcommercehubStoredCredentialInitiator::CardHolder,
            // scheduled: false,
            scheme_referenced_transaction_id: None,
            sequence: FiservcommercehubStoredCredentialSequence::First,
            scheme_original_amount: None,
        }
    }
    /// * `scheme_original_amount` - Optional original amount (required for Discover)
    pub fn new_mit(
        scheme_referenced_transaction_id: Option<String>,
        scheme_original_amount: Option<FloatMajorUnit>,
    ) -> Self {
        Self {
            initiator: FiservcommercehubStoredCredentialInitiator::Merchant,
            // scheduled,
            scheme_referenced_transaction_id,
            sequence: FiservcommercehubStoredCredentialSequence::Subsequent,
            scheme_original_amount,
        }
    }
}

// =============================================================================
// PAYMENT TOKEN STRUCTURES
// =============================================================================

/// Payment token information received from tokenization providers.
/// Contains token data and metadata for network tokenization.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentToken {
    /// Token created from the payment source (e.g., "1234123412340019")
    /// Max Length: 2048
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_data: Option<Secret<String>>,
    /// Source for the Token Service Provider (TSP) (e.g., "TRANSARMOR")
    /// Max Length: 256
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_source: Option<String>,
    /// Response code for token generation request (e.g., "000")
    /// Max Length: 256
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_response_code: Option<String>,
    /// Response description for token generation request (e.g., "SUCCESS")
    /// Max Length: 256
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_response_description: Option<String>,
    /// Cryptographic value sent by the merchant during payment authentication
    /// Max Length: 256
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    /// Token Requestor ID - identifier used by merchants to request network tokens
    /// Max Length: 256
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_requestor_id: Option<String>,
    /// Token Assurance Method returned to merchants in auth response
    /// Max Length: 256
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_assurance_method: Option<String>,
    /// Reference id of MPAN used for MPAN data management
    /// Max Length: 100
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_reference_id: Option<String>,
}

/// Wrapper for a list of payment tokens.
/// Response contains a list of tokens and their status for each tokenization provider.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FiservcommercehubPaymentTokens(pub Vec<FiservcommercehubPaymentToken>);

impl FiservcommercehubPaymentTokens {
    /// Returns the first successful payment token from the list.
    /// A token is considered successful if the response code is "000" or "SUCCESS".
    pub fn get_mandate_reference(
        &self,
        original_txn_id: Option<String>,
    ) -> Option<Box<MandateReference>> {
        self.0
            .iter()
            .find(|token| {
                token
                    .token_response_code
                    .as_ref()
                    .map(|code| {
                        code == "000"
                            || code.eq_ignore_ascii_case("SUCCESS")
                                && token.token_source == Some("TRANSARMOR".to_string())
                    })
                    .unwrap_or(false)
            })
            .map(|token| {
                Box::new(MandateReference {
                    connector_mandate_id: token.token_data.as_ref().map(|t| t.peek().clone()),
                    payment_method_id: token.token_source.clone(),
                    connector_mandate_request_reference_id: original_txn_id,
                })
            })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubMpiData {
    pub cavv: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAdditionalData3DS {
    pub ds_transaction_id: String,
    pub mpi_data: FiservcommercehubMpiData,
}

/// Builds the additional_data_3ds structure from authentication data.
/// This is reusable across Authorize flow.
pub fn build_additional_data_3ds(
    authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
) -> Option<FiservcommercehubAdditionalData3DS> {
    authentication_data.and_then(
        |auth_data| match (&auth_data.ds_trans_id, &auth_data.cavv) {
            (Some(ds_trans_id), Some(cavv)) => {
                let xid = auth_data
                    .threeds_server_transaction_id
                    .clone()
                    .or_else(|| auth_data.ds_trans_id.clone());

                Some(FiservcommercehubAdditionalData3DS {
                    ds_transaction_id: ds_trans_id.clone(),
                    mpi_data: FiservcommercehubMpiData {
                        cavv: cavv.clone(),
                        xid,
                    },
                })
            }
            _ => None,
        },
    )
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let total = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        let access_token = router_data.resource_common_data.get_access_token()?;
        let parts: Vec<&str> = access_token.split(ACCESS_TOKEN_SEPARATOR).collect();

        let key_id = parts
            .first()
            .ok_or_else(|| {
                error_stack::report!(errors::IntegrationError::MissingRequiredField {
                    field_name: "key_id",
                    context: Default::default()
                })
            })?
            .to_string();

        let encoded_public_key = parts.get(1).ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "encoded_public_key",
                context: Default::default()
            })
        })?;

        let public_key_der = general_purpose::STANDARD
            .decode(encoded_public_key)
            .map_err(|_| {
                error_stack::report!(errors::IntegrationError::RequestEncodingFailed {
                    context: Default::default()
                })
            })
            .attach_printable("Failed to decode Base64 RSA public key")?;

        let auth_type = &router_data.connector_config;
        let auth = FiservcommercehubAuthType::try_from(auth_type)?;

        let (source, stored_credentials) = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                let encrypted_card = encrypt_card_data(card, key_id, &public_key_der)?;

                let stored_credentials =
                    if router_data.request.is_customer_initiated_mandate_payment() {
                        Some(FiservcommercehubStoredCredentials::new_cit())
                    } else {
                        None
                    };

                (
                    FiservcommercehubSourceData::PaymentCard {
                        encryption_data: FiservcommercehubEncryptionData {
                            key_id: encrypted_card.key_id,
                            encryption_type: ENCRYPTION_TYPE_RSA.to_string(),
                            encryption_block: encrypted_card.encryption_block,
                            encryption_block_fields: encrypted_card.encryption_block_fields,
                        },
                    },
                    stored_credentials,
                )
            }
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::not_implemented(
                        "This payment method is not implemented".to_string(),
                    )
                ))
            }
        };

        let origin = FiservcommercehubOrigin::from(router_data.request.payment_channel.as_ref());

        let eci_indicator = router_data
            .request
            .authentication_data
            .as_ref()
            .and_then(|auth_data| auth_data.eci.clone());

        let additional_data_3ds =
            build_additional_data_3ds(router_data.request.authentication_data.as_ref());

        let request = Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            source,
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            transaction_details: FiservcommercehubTransactionDetailsReq {
                capture_flag: router_data.request.is_auto_capture(),
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            stored_credentials,
            transaction_interaction: FiservcommercehubTransactionInteractionReq {
                origin,
                eci_indicator,
            },
            additional_data_3ds,
        };
        Ok(request)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubTransactionState {
    Approved,
    Captured,
    Authorized,
    Pending,
    Declined,
    Rejected,
    Failed,
    Cancelled,
}

impl From<&FiservcommercehubTransactionState> for AttemptStatus {
    fn from(state: &FiservcommercehubTransactionState) -> Self {
        match state {
            FiservcommercehubTransactionState::Approved
            | FiservcommercehubTransactionState::Captured => Self::Charged,
            FiservcommercehubTransactionState::Authorized => Self::Authorized,
            FiservcommercehubTransactionState::Pending => Self::Pending,
            FiservcommercehubTransactionState::Declined
            | FiservcommercehubTransactionState::Rejected
            | FiservcommercehubTransactionState::Failed => Self::Failure,
            FiservcommercehubTransactionState::Cancelled => Self::Voided,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubRefundState {
    Approved,
    Captured,
    Authorized,
    Pending,
    Declined,
    Rejected,
    Failed,
    Cancelled,
}

impl From<&FiservcommercehubRefundState> for RefundStatus {
    fn from(state: &FiservcommercehubRefundState) -> Self {
        match state {
            FiservcommercehubRefundState::Approved | FiservcommercehubRefundState::Captured => {
                Self::Success
            }
            FiservcommercehubRefundState::Authorized | FiservcommercehubRefundState::Pending => {
                Self::Pending
            }
            FiservcommercehubRefundState::Declined
            | FiservcommercehubRefundState::Rejected
            | FiservcommercehubRefundState::Failed
            | FiservcommercehubRefundState::Cancelled => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAuthorizeResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
    pub payment_tokens: Option<FiservcommercehubPaymentTokens>,
    /// Additional 3DS data returned in the response as a generic JSON Value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubGatewayResponseBody {
    pub transaction_state: FiservcommercehubTransactionState,
    pub transaction_processing_details: FiservcommercehubTxnDetails,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTxnDetails {
    pub order_id: Option<String>,
    pub transaction_id: String,
}

/// Builds the ConnectorResponseData with additional_payment_method_data containing
/// 3DS authentication data if available
fn build_connector_response_with_3ds(
    additional_data_3ds: Option<&serde_json::Value>,
) -> Option<domain_types::router_data::ConnectorResponseData> {
    additional_data_3ds.map(|auth_data| {
        let additional_payment_method_data =
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: Some(auth_data.clone()),
                payment_checks: None,
                card_network: None,
                domestic_network: None,
                auth_code: None,
            };
        domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
            additional_payment_method_data,
        )
    })
}

impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<FiservcommercehubAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        let status = AttemptStatus::from(&item.response.gateway_response.transaction_state);

        // Build connector_response with 3DS authentication data if available
        let connector_response =
            build_connector_response_with_3ds(item.response.additional_data_3ds.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: item.response.payment_tokens.and_then(|token| {
                    token.get_mandate_reference(Some(txn.transaction_id.clone()))
                }),
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: txn.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// PSYNC FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncMerchantDetails {
    pub merchant_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubReferenceTransactionDetails {
    pub reference_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncRequest {
    pub merchant_details: FiservcommercehubPSyncMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for FiservcommercehubPSyncRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;
        Ok(Self {
            merchant_details: FiservcommercehubPSyncMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: connector_transaction_id,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncGatewayResponse {
    pub transaction_state: FiservcommercehubTransactionState,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncItem {
    pub gateway_response: FiservcommercehubPSyncGatewayResponse,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FiservcommercehubPSyncResponse(pub Vec<FiservcommercehubPSyncItem>);

impl TryFrom<ResponseRouterData<FiservcommercehubPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let psync_item = item.response.0.into_iter().next().ok_or_else(|| {
            error_stack::report!(
                crate::utils::response_deserialization_fail(
                    item.http_code
                , "fiservcommercehub: response body did not match the expected format; confirm API version and connector documentation.")
            )
        })?;
        let status = AttemptStatus::from(&psync_item.gateway_response.transaction_state);
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REFUND FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundTransactionDetails {
    pub merchant_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub transaction_details: FiservcommercehubRefundTransactionDetails,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for FiservcommercehubRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let total = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            transaction_details: FiservcommercehubRefundTransactionDetails {
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_transaction_id.clone(),
            },
        })
    }
}

/// Response body from `POST /payments/v1/refunds`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundResponse {
    pub gateway_response: FiservcommercehubRefundGatewayResponseBody,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRefundGatewayResponseBody {
    pub transaction_state: FiservcommercehubRefundState,
    pub transaction_processing_details: FiservcommercehubTxnDetails,
}

impl TryFrom<ResponseRouterData<FiservcommercehubRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.gateway_response.transaction_state);
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: txn.transaction_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// RSYNC FLOW (Refund Sync)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncRequest {
    pub merchant_details: FiservcommercehubPSyncMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for FiservcommercehubRSyncRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            merchant_details: FiservcommercehubPSyncMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_refund_id.clone(),
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncGatewayResponse {
    pub transaction_state: FiservcommercehubRefundState,
    pub transaction_processing_details: Option<FiservcommercehubRSyncTxnDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncTxnDetails {
    pub transaction_id: String,
    pub order_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRSyncItem {
    pub gateway_response: FiservcommercehubRSyncGatewayResponse,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FiservcommercehubRSyncResponse(pub Vec<FiservcommercehubRSyncItem>);

impl TryFrom<ResponseRouterData<FiservcommercehubRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let rsync_item = item.response.0.into_iter().next().ok_or_else(|| {
            error_stack::report!(
                crate::utils::response_deserialization_fail(
                    item.http_code
                , "fiservcommercehub: response body did not match the expected format; confirm API version and connector documentation.")
            )
        })?;
        let refund_status = RefundStatus::from(&rsync_item.gateway_response.transaction_state);
        let connector_refund_id = rsync_item
            .gateway_response
            .transaction_processing_details
            .map(|d| d.transaction_id)
            .unwrap_or(item.router_data.request.connector_refund_id.clone());
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// VOID FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubVoidRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<FiservcommercehubAuthorizeAmount>,
    pub transaction_details: FiservcommercehubRefundTransactionDetails,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for FiservcommercehubVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;

        let amount = match (router_data.request.amount, router_data.request.currency) {
            (Some(minor_amount), Some(currency)) => {
                let total =
                    utils::convert_amount(item.connector.amount_converter, minor_amount, currency)?;
                Some(FiservcommercehubAuthorizeAmount { currency, total })
            }
            _ => None,
        };

        Ok(Self {
            amount,
            transaction_details: FiservcommercehubRefundTransactionDetails {
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_transaction_id.clone(),
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubVoidResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
}

impl TryFrom<ResponseRouterData<FiservcommercehubVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.gateway_response.transaction_state);
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: txn.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// ACCESS TOKEN FLOW
// =============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubMerchantDetails {
    pub merchant_id: Secret<String>,
    pub terminal_id: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAccessTokenRequest {
    pub merchant_details: FiservcommercehubMerchantDetails,
}

impl TryFrom<&ConnectorSpecificConfig> for FiservcommercehubAccessTokenRequest {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        let auth = FiservcommercehubAuthType::try_from(auth_type)?;
        Ok(Self {
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for FiservcommercehubAccessTokenRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data.connector_config)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubTransactionProcessingDetails {
    pub api_key: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubGatewayResponse {
    pub transaction_processing_details: FiservcommercehubTransactionProcessingDetails,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAsymmetricKeyDetails {
    pub key_id: String,
    pub encoded_public_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubAccessTokenResponse {
    pub gateway_response: FiservcommercehubGatewayResponse,
    pub asymmetric_key_details: FiservcommercehubAsymmetricKeyDetails,
}

impl<F, T> TryFrom<ResponseRouterData<FiservcommercehubAccessTokenResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let key_id = &item.response.asymmetric_key_details.key_id;
        let encoded_public_key = &item.response.asymmetric_key_details.encoded_public_key;
        let combined_token = Secret::new(format!(
            "{key_id}{ACCESS_TOKEN_SEPARATOR}{encoded_public_key}"
        ));
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: combined_token,
                expires_in: Some(604_800), // 1 week in seconds
                token_type: None,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// CAPTURE FLOW
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubCaptureRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub transaction_details: FiservcommercehubTransactionDetailsReq,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub reference_transaction_details: FiservcommercehubReferenceTransactionDetails,
    /// Additional 3DS data for capture requests (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<FiservcommercehubAdditionalData3DS>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for FiservcommercehubCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let total = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;
        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;
        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;
        Ok(Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            transaction_details: FiservcommercehubTransactionDetailsReq {
                capture_flag: true,
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: connector_transaction_id,
            },
            // Note: Capture flow doesn't currently receive authentication_data
            // in PaymentsCaptureData. Set to None unless Fiserv requires it.
            additional_data_3ds: None,
        })
    }
}

/// Capture response - wrapper around AuthorizeResponse using transparent serde
/// This allows deserializing the same response format as Authorize, since Fiserv
/// Commerce Hub uses the same response structure for both /charges and capture operations.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct FiservcommercehubCaptureResponse(pub FiservcommercehubAuthorizeResponse);

impl TryFrom<ResponseRouterData<FiservcommercehubCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Delegate to the Authorize response handling since the inner structure is identical
        let txn = &item
            .response
            .0
            .gateway_response
            .transaction_processing_details;
        let status = AttemptStatus::from(&item.response.0.gateway_response.transaction_state);

        // Build connector_response with 3DS authentication data if available
        let connector_response =
            build_connector_response_with_3ds(item.response.0.additional_data_3ds.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: item.response.0.payment_tokens.and_then(|token| {
                    token.get_mandate_reference(Some(txn.transaction_id.clone()))
                }),
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: txn.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// REPEAT PAYMENT FLOW
// =============================================================================

/// RepeatPayment request - reused from AuthorizeRequest since Fiserv Commerce Hub
/// uses the same /charges endpoint for both initial authorization and repeat payments.
/// The difference is in the source (PaymentToken vs PaymentCard) and stored credentials.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubRepeatPaymentRequest {
    pub amount: FiservcommercehubAuthorizeAmount,
    pub source: FiservcommercehubSourceData,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub transaction_details: FiservcommercehubTransactionDetailsReq,
    pub transaction_interaction: FiservcommercehubTransactionInteractionReq,
    pub stored_credentials: FiservcommercehubStoredCredentials,
}

/// RepeatPayment response - wrapper around AuthorizeResponse using transparent serde
/// This allows deserializing the same response format as Authorize.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct FiservcommercehubRepeatResponse(pub FiservcommercehubAuthorizeResponse);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubRepeatPaymentRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let total = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        let auth = FiservcommercehubAuthType::try_from(&router_data.connector_config)?;

        // Extract mandate reference for repeat payment
        let (connector_mandate_id, token_source, scheme_referenced_transaction_id) =
            match &router_data.request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(id) => {
                    let connector_mandate_id = id.get_connector_mandate_id().ok_or(
                        errors::IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                            context: Default::default(),
                        },
                    )?;
                    let token_source = id
                        .get_payment_method_id()
                        .ok_or(errors::IntegrationError::MissingRequiredField {
                            field_name: "payment_method_id",
                            context: Default::default(),
                        })?
                        .to_string();
                    let scheme_ref_id = id.get_connector_mandate_request_reference_id();
                    (connector_mandate_id, token_source, scheme_ref_id)
                }
                _ => {
                    return Err(error_stack::report!(
                        errors::IntegrationError::MissingRequiredField {
                            field_name: "mandate_reference_id.connector_mandate_id",
                            context: Default::default(),
                        }
                    ))
                }
            };

        // Build stored credentials for MIT (Merchant Initiated Transaction)
        let stored_credentials =
            FiservcommercehubStoredCredentials::new_mit(scheme_referenced_transaction_id, None);

        // For repeat payments, use Ecom origin as default
        let origin = FiservcommercehubOrigin::Ecom;

        // Extract card expiration details from additional_payment_data if available
        let card_info = router_data
            .request
            .additional_payment_data
            .as_ref()
            .and_then(|data| match data {
                domain_types::types::AdditionalPaymentData::Card(card_info) => {
                    match (&card_info.card_exp_month, &card_info.card_exp_year) {
                        (Some(month), Some(year)) => Some(FiservcommercehubTokenCardInfo {
                            expiration_month: month.clone(),
                            expiration_year: year.clone(),
                        }),
                        _ => None,
                    }
                }
            });

        let request = Self {
            amount: FiservcommercehubAuthorizeAmount {
                currency: router_data.request.currency,
                total,
            },
            source: FiservcommercehubSourceData::PaymentToken {
                token_data: connector_mandate_id,
                token_source,
                decline_duplicates: Some(false),
                card: card_info,
            },
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            transaction_details: FiservcommercehubTransactionDetailsReq {
                capture_flag: router_data.request.is_auto_capture(),
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            transaction_interaction: FiservcommercehubTransactionInteractionReq {
                origin,
                eci_indicator: None,
            },
            stored_credentials,
        };
        Ok(request)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiservcommercehubRepeatResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubRepeatResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Delegate to the Authorize response handling since the inner structure is identical
        let txn = &item
            .response
            .0
            .gateway_response
            .transaction_processing_details;
        let status = AttemptStatus::from(&item.response.0.gateway_response.transaction_state);

        // Build connector_response with 3DS authentication data if available
        let connector_response =
            build_connector_response_with_3ds(item.response.0.additional_data_3ds.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: item.response.0.payment_tokens.and_then(|token| {
                    token.get_mandate_reference(Some(txn.transaction_id.clone()))
                }),
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: txn.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// SETUP MANDATE FLOW (Tokenize Card)
// =============================================================================

/// SetupMandate request for tokenizing card data without charging
/// Maps to POST /payments-vas/v1/tokens
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSetupMandateRequest {
    pub source: FiservcommercehubSourceData,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub transaction_details: FiservcommercehubSetupMandateTransactionDetails,
    pub transaction_interaction: FiservcommercehubTransactionInteractionReq,
    /// Additional 3DS data for external 3DS authentication (when authentication_data is present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<FiservcommercehubAdditionalData3DS>,
    /// Stored credentials for CIT (Card Holder Initiated Transaction)
    /// Indicates this is a first-time stored credential setup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credentials: Option<FiservcommercehubStoredCredentials>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSetupMandateTransactionDetails {
    pub merchant_transaction_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FiservcommercehubRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservcommercehubSetupMandateRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::FiservcommercehubRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        // SetupMandate (tokenization) should not have an amount - it's for storing cards without charging
        let amount = router_data.request.amount.unwrap_or(0);
        if amount > 0 {
            return Err(error_stack::report!(
                errors::IntegrationError::NotSupported {
                    message: "SetupMandate flow does not support amounts greater than 0"
                        .to_string(),
                    connector: "fiservcommercehub",
                    context: Default::default(),
                }
            ));
        }

        let access_token = router_data.resource_common_data.get_access_token()?;
        let parts: Vec<&str> = access_token.split(ACCESS_TOKEN_SEPARATOR).collect();

        let key_id = parts
            .first()
            .ok_or_else(|| {
                error_stack::report!(errors::IntegrationError::MissingRequiredField {
                    field_name: "key_id",
                    context: Default::default()
                })
            })?
            .to_string();

        let encoded_public_key = parts.get(1).ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "encoded_public_key",
                context: Default::default()
            })
        })?;

        let public_key_der = general_purpose::STANDARD
            .decode(encoded_public_key)
            .map_err(|_| {
                error_stack::report!(errors::IntegrationError::RequestEncodingFailed {
                    context: Default::default()
                })
            })
            .attach_printable("Failed to decode Base64 RSA public key")?;

        let auth_type = &router_data.connector_config;
        let auth = FiservcommercehubAuthType::try_from(auth_type)?;

        let source = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                let encrypted_card = encrypt_card_data(card, key_id, &public_key_der)?;

                FiservcommercehubSourceData::PaymentCard {
                    encryption_data: FiservcommercehubEncryptionData {
                        key_id: encrypted_card.key_id,
                        encryption_type: ENCRYPTION_TYPE_RSA.to_string(),
                        encryption_block: encrypted_card.encryption_block,
                        encryption_block_fields: encrypted_card.encryption_block_fields,
                    },
                }
            }
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::not_implemented(
                        "This payment method is not implemented".to_string(),
                    )
                ))
            }
        };

        let origin = FiservcommercehubOrigin::from(router_data.request.payment_channel.as_ref());

        // SetupMandate is always a CIT (Card Holder Initiated Transaction)
        // as it's the first transaction where cardholder consents to store credentials
        let stored_credentials = Some(FiservcommercehubStoredCredentials::new_cit());

        let request = Self {
            source,
            merchant_details: FiservcommercehubMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            transaction_details: FiservcommercehubSetupMandateTransactionDetails {
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            transaction_interaction: FiservcommercehubTransactionInteractionReq {
                origin,
                eci_indicator: None,
            },
            additional_data_3ds: None,
            stored_credentials,
        };
        Ok(request)
    }
}

/// SetupMandate response - same structure as Authorize response
/// The tokens endpoint returns a similar structure to charges
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSetupMandateResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
    pub payment_tokens: Option<FiservcommercehubPaymentTokens>,
    /// Additional 3DS data returned in the response as a generic JSON Value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<serde_json::Value>,
}

impl<F, T> TryFrom<ResponseRouterData<FiservcommercehubSetupMandateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
where
    F: Clone,
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservcommercehubSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let txn = &item
            .response
            .gateway_response
            .transaction_processing_details;
        // For setup mandate, Authorized status means the mandate was successfully set up
        // and should be treated as charged/completed
        let txn_state = &item.response.gateway_response.transaction_state;
        let status = match txn_state {
            FiservcommercehubTransactionState::Authorized => AttemptStatus::Charged,
            _ => AttemptStatus::from(txn_state),
        };

        // Build connector_response with 3DS authentication data if available
        let connector_response =
            build_connector_response_with_3ds(item.response.additional_data_3ds.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(txn.transaction_id.clone()),
                redirection_data: None,
                mandate_reference: item.response.payment_tokens.and_then(|token| {
                    token.get_mandate_reference(Some(txn.transaction_id.clone()))
                }),
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: txn.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
