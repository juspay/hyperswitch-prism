use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::ResponseRouterData;
use base64::{engine::general_purpose, Engine};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
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
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{CardWithNoCvc, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// Constants for encryption and token formatting
pub(crate) const ENCRYPTION_TYPE_RSA: &str = "RSA";
pub(crate) const ACCESS_TOKEN_SEPARATOR: &str = "|||";
pub(crate) const TOKEN_SOURCE_TRANSARMOR: &str = "TRANSARMOR";
const FISERV_PAYMENT_METHOD_ENCRYPTION_URL: &str =
    "https://developer.fiserv.com/product/CommerceHub/docs/Payment-Methods/Payment-Methods.mdx";
const FISERV_PAYMENT_AUTHENTICATION_URL: &str =
    "https://developer.fiserv.com/product/CommerceHub/docs/Developer-Resources/Authentication/Authentication.mdx";
const FISERV_CHARGES_API_VERSION_URL: &str = "https://developer.fiserv.com/product/CommerceHub/api/post/payments/v1/charges?branch=active&version=1.26.0602";
const FISERV_TOKEN_API_VERSION_URL: &str = "https://developer.fiserv.com/product/CommerceHub/api/post/payments-vas/v1/tokens?branch=active&version=1.26.0602";
#[derive(Debug)]
pub struct EncryptedCardData {
    pub key_id: String,
    pub encryption_block: Secret<String>,
    pub encryption_block_fields: String,
}

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
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "card_holder_name is required for card encryption".to_string(),
                ),
                ..Default::default()
            },
        })?;
    let expiration_month = card.card_exp_month.peek().to_string();
    let expiration_year = card.get_expiry_year_4_digit().peek().to_string();
    let security_code = card.card_cvc.peek().to_string();

    let plain_block =
        format!("{card_data}{name_on_card}{expiration_month}{expiration_year}{security_code}");

    let card_data_len = card_data.len();
    let name_on_card_len = name_on_card.len();
    let expiration_month_len = expiration_month.len();
    let expiration_year_len = expiration_year.len();
    let security_code_len = security_code.len();
    let encryption_block_fields = format!(
        "card.cardData:{card_data_len},card.nameOnCard:{name_on_card_len},card.expirationMonth:{expiration_month_len},card.expirationYear:{expiration_year_len},card.securityCode:{security_code_len}"
    );

    let encrypted_bytes = RsaOaepSha256::encrypt(public_key_der, plain_block.as_bytes())
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext {
                doc_url: Some(FISERV_PAYMENT_METHOD_ENCRYPTION_URL.to_string()),
                suggested_action: Some(
                    "Ensure the RSA public key is correctly configured and valid".to_string(),
                ),
                additional_context: Some(
                    "RSA OAEP-SHA256 encryption for card data failed".to_string(),
                ),
            },
        })
        .attach_printable("RSA OAEP-SHA256 encryption of card data failed")?;

    let encryption_block = Secret::new(general_purpose::STANDARD.encode(&encrypted_bytes));

    Ok(EncryptedCardData {
        key_id,
        encryption_block,
        encryption_block_fields,
    })
}

fn encrypt_card_data_no_cvc(
    card: &CardWithNoCvc,
    key_id: String,
    public_key_der: &[u8],
) -> Result<EncryptedCardData, error_stack::Report<errors::IntegrationError>> {
    let card_data = card.card_number.get_card_no();
    let name_on_card = card
        .get_cardholder_name()
        .change_context(errors::IntegrationError::MissingRequiredField {
            field_name: "card_holder_name",
            context: errors::IntegrationErrorContext {
                doc_url: Some(FISERV_PAYMENT_METHOD_ENCRYPTION_URL.to_string()),
                suggested_action: Some(
                    "Provide card_holder_name as it is required for card encryption".to_string(),
                ),
                additional_context: Some(
                    "card_holder_name is required for card encryption".to_string(),
                ),
            },
        })?
        .peek()
        .clone();
    let expiration_month = card
        .get_card_expiry_month_2_digit()
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?
        .peek()
        .to_string();
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
            context: errors::IntegrationErrorContext {
                doc_url: Some(FISERV_PAYMENT_METHOD_ENCRYPTION_URL.to_string()),
                suggested_action: Some(
                    "Ensure the RSA public key is correctly configured and valid".to_string(),
                ),
                additional_context: Some(
                    "RSA OAEP-SHA256 encryption for card data failed".to_string(),
                ),
            },
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
                context: errors::IntegrationErrorContext {
                    additional_context: Some("HMAC-SHA256 signature generation failed".to_string()),
                    doc_url: Some(FISERV_PAYMENT_AUTHENTICATION_URL.to_string()),
                    suggested_action: Some(
                        "Verify the API secret is correct and properly configured".to_string(),
                    ),
                },
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
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentCardSource {
    pub encryption_data: FiservcommercehubEncryptionData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentTokenSource {
    pub token_data: Secret<String>,
    pub token_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decline_duplicates: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<FiservcommercehubTokenCardInfo>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "sourceType")]
pub enum FiservcommercehubSourceData {
    PaymentCard(FiservcommercehubPaymentCardSource),
    PaymentToken(FiservcommercehubPaymentTokenSource),
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

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubStoredCredentialInitiator {
    Merchant,
    CardHolder,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservcommercehubStoredCredentialSequence {
    First,
    Subsequent,
}

/// Reference: https://developer.fiserv.com/product/CommerceHub/docs/Payment-Methods/Tokenization/Stored-Credentials.mdx
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubStoredCredentials {
    pub initiator: FiservcommercehubStoredCredentialInitiator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme_referenced_transaction_id: Option<String>,
    pub sequence: FiservcommercehubStoredCredentialSequence,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentToken {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_data: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_response_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_response_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_requestor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_assurance_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_reference_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FiservcommercehubPaymentTokens(pub Vec<FiservcommercehubPaymentToken>);

impl FiservcommercehubPaymentTokens {
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
                        (code == "000" || code.eq_ignore_ascii_case("SUCCESS"))
                            && token.token_source == Some(TOKEN_SOURCE_TRANSARMOR.to_string())
                    })
                    .unwrap_or(false)
            })
            .map(|token| {
                Box::new(MandateReference {
                    connector_mandate_id: token.token_data.as_ref().map(|t| t.peek().clone()),
                    payment_method_id: token.token_source.clone(),
                    connector_mandate_request_reference_id: original_txn_id,
                    mandate_metadata: None,
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
                    FiservcommercehubSourceData::PaymentCard(FiservcommercehubPaymentCardSource {
                        encryption_data: FiservcommercehubEncryptionData {
                            key_id: encrypted_card.key_id,
                            encryption_type: ENCRYPTION_TYPE_RSA.to_string(),
                            encryption_block: encrypted_card.encryption_block,
                            encryption_block_fields: encrypted_card.encryption_block_fields,
                        },
                    }),
                    stored_credentials,
                )
            }
            PaymentMethodData::CardWithNoCvc(card) => {
                let encrypted_card = encrypt_card_data_no_cvc(card, key_id, &public_key_der)?;

                let stored_credentials =
                    if router_data.request.is_customer_initiated_mandate_payment() {
                        Some(FiservcommercehubStoredCredentials::new_cit())
                    } else {
                        None
                    };

                (
                    FiservcommercehubSourceData::PaymentCard(FiservcommercehubPaymentCardSource {
                        encryption_data: FiservcommercehubEncryptionData {
                            key_id: encrypted_card.key_id,
                            encryption_type: ENCRYPTION_TYPE_RSA.to_string(),
                            encryption_block: encrypted_card.encryption_block,
                            encryption_block_fields: encrypted_card.encryption_block_fields,
                        },
                    }),
                    stored_credentials,
                )
            }
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "This payment method is not implemented".to_string(),
                        Default::default()
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
    pub payment_receipt: Option<FiservcommercehubPaymentReceipt>,
    pub payment_tokens: Option<FiservcommercehubPaymentTokens>,
    /// Additional 3DS data returned in the response as a generic JSON Value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<serde_json::Value>,
}

impl FiservcommercehubAuthorizeResponse {
    fn approval_code(&self) -> Option<String> {
        self.payment_receipt
            .as_ref()
            .and_then(FiservcommercehubPaymentReceipt::approval_code)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPaymentReceipt {
    pub processor_response_details: Option<FiservcommercehubProcessorResponseDetails>,
}

impl FiservcommercehubPaymentReceipt {
    fn approval_code(&self) -> Option<String> {
        self.processor_response_details
            .as_ref()
            .and_then(|details| details.approval_code.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubProcessorResponseDetails {
    pub approval_code: Option<String>,
    pub response_code: Option<String>,
    pub response_message: Option<String>,
    pub host_response_code: Option<String>,
    pub host_response_message: Option<String>,
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

fn build_connector_response(
    additional_data_3ds: Option<&serde_json::Value>,
    auth_code: Option<String>,
) -> Option<domain_types::router_data::ConnectorResponseData> {
    if additional_data_3ds.is_some() || auth_code.is_some() {
        let additional_payment_method_data =
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: additional_data_3ds.cloned(),
                payment_checks: None,
                card_network: None,
                domestic_network: None,
                auth_code,
            };
        Some(
            domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
                additional_payment_method_data,
            ),
        )
    } else {
        None
    }
}

fn build_payment_response(
    status: AttemptStatus,
    status_code: u16,
    resource_id: ResponseId,
    connector_transaction_id: Option<String>,
    connector_response_reference_id: Option<String>,
    payment_tokens: Option<&FiservcommercehubPaymentTokens>,
    payment_receipt: Option<&FiservcommercehubPaymentReceipt>,
) -> Result<PaymentsResponseData, ErrorResponse> {
    match status {
        AttemptStatus::Failure => {
            let processor_response_details =
                payment_receipt.and_then(|receipt| receipt.processor_response_details.as_ref());

            let response_code =
                processor_response_details.and_then(|details| details.response_code.clone());
            let response_message =
                processor_response_details.and_then(|details| details.response_message.clone());
            let host_response_code =
                processor_response_details.and_then(|details| details.host_response_code.clone());
            let host_response_message = processor_response_details
                .and_then(|details| details.host_response_message.clone());

            Err(ErrorResponse {
                code: response_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: response_message
                    .clone()
                    .or_else(|| host_response_message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: host_response_message
                    .clone()
                    .or_else(|| response_message.clone()),
                status_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id,
                network_decline_code: host_response_code,
                network_advice_code: None,
                network_error_message: host_response_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        }
        _ => Ok(PaymentsResponseData::TransactionResponse {
            resource_id,
            redirection_data: None,
            mandate_reference: payment_tokens
                .and_then(|token| token.get_mandate_reference(connector_transaction_id)),
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id,
            incremental_authorization_allowed: None,
            status_code,
            splits: None,
            payment_account_reference: None,
        }),
    }
}

fn build_transaction_payment_response(
    status: AttemptStatus,
    status_code: u16,
    txn: &FiservcommercehubTxnDetails,
    payment_tokens: Option<&FiservcommercehubPaymentTokens>,
    payment_receipt: Option<&FiservcommercehubPaymentReceipt>,
) -> Result<PaymentsResponseData, ErrorResponse> {
    let connector_transaction_id = txn.transaction_id.clone();

    build_payment_response(
        status,
        status_code,
        ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
        Some(connector_transaction_id),
        txn.order_id.clone(),
        payment_tokens,
        payment_receipt,
    )
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

        let connector_response = build_connector_response(
            item.response.additional_data_3ds.as_ref(),
            item.response.approval_code(),
        );
        let response = build_transaction_payment_response(
            status,
            item.http_code,
            txn,
            item.response.payment_tokens.as_ref(),
            item.response.payment_receipt.as_ref(),
        );

        Ok(Self {
            response,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_merchant_transaction_id: Option<String>,
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
            .inspect_err(|_| {
                tracing::warn!(
                    "fiservcommercehub PSync: connector_transaction_id not present,
                     falling back to connector_request_reference_id"
                );
            })
            .ok()
            .filter(|id| !id.is_empty());

        let connector_request_reference_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let reference_transaction_details = match connector_transaction_id {
            Some(txn_id) => FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: Some(txn_id),
                reference_merchant_transaction_id: None,
            },
            None => FiservcommercehubReferenceTransactionDetails {
                reference_transaction_id: None,
                reference_merchant_transaction_id: Some(connector_request_reference_id),
            },
        };

        Ok(Self {
            merchant_details: FiservcommercehubPSyncMerchantDetails {
                merchant_id: auth.merchant_id.clone(),
            },
            reference_transaction_details,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncGatewayResponse {
    pub transaction_state: FiservcommercehubTransactionState,
    pub transaction_processing_details: Option<FiservcommercehubTxnDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubPSyncItem {
    pub gateway_response: FiservcommercehubPSyncGatewayResponse,
    pub payment_receipt: Option<FiservcommercehubPaymentReceipt>,
}

impl FiservcommercehubPSyncItem {
    fn approval_code(&self) -> Option<String> {
        self.payment_receipt
            .as_ref()
            .and_then(FiservcommercehubPaymentReceipt::approval_code)
    }
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
        let connector_response = build_connector_response(None, psync_item.approval_code());
        let connector_transaction_id = psync_item
            .gateway_response
            .transaction_processing_details
            .as_ref()
            .map(|txn| txn.transaction_id.clone());
        let resource_id = connector_transaction_id
            .clone()
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or_else(|| {
                tracing::warn!(
                    "fiservcommercehub PSync: connector_transaction_id absent in response, 
                     resource_id set to NoResponseId"
                );
                ResponseId::NoResponseId
            });
        let response = build_payment_response(
            status,
            item.http_code,
            resource_id,
            connector_transaction_id,
            None,
            None,
            psync_item.payment_receipt.as_ref(),
        );

        Ok(Self {
            response,
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
                reference_transaction_id: Some(
                    router_data.request.connector_transaction_id.clone(),
                ),
                reference_merchant_transaction_id: None,
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
                acquirer_reference_number: None,
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
                reference_transaction_id: Some(router_data.request.connector_refund_id.clone()),
                reference_merchant_transaction_id: None,
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
                acquirer_reference_number: None,
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
                reference_transaction_id: Some(
                    router_data.request.connector_transaction_id.clone(),
                ),
                reference_merchant_transaction_id: None,
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
                network_txn_link_id: None,
                connector_response_reference_id: txn.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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
                MerchantAuthenticationFlowData,
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
                MerchantAuthenticationFlowData,
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
    for RouterDataV2<F, MerchantAuthenticationFlowData, T, ServerAuthenticationTokenResponseData>
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
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "connector_transaction_id is required for Capture".to_string(),
                    ),
                    doc_url: Some(FISERV_CHARGES_API_VERSION_URL.to_string()),
                    ..Default::default()
                },
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
                reference_transaction_id: Some(connector_transaction_id),
                reference_merchant_transaction_id: None,
            },
            // Note: Capture flow doesn't currently receive authentication_data
            // in PaymentsCaptureData. Set to None unless Fiserv requires it.
            additional_data_3ds: None,
        })
    }
}

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
        let txn = &item
            .response
            .0
            .gateway_response
            .transaction_processing_details;
        let status = AttemptStatus::from(&item.response.0.gateway_response.transaction_state);

        let connector_response = build_connector_response(
            item.response.0.additional_data_3ds.as_ref(),
            item.response.0.approval_code(),
        );
        let response = build_transaction_payment_response(
            status,
            item.http_code,
            txn,
            item.response.0.payment_tokens.as_ref(),
            item.response.0.payment_receipt.as_ref(),
        );

        Ok(Self {
            response,
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
                            context: errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "connector_mandate_id is required for repeat payments"
                                        .to_string(),
                                ),
                                doc_url: Some(FISERV_CHARGES_API_VERSION_URL.to_string()),
                                ..Default::default()
                            },
                        },
                    )?;
                    let token_source = TOKEN_SOURCE_TRANSARMOR.to_string();
                    let scheme_ref_id = id.get_connector_mandate_request_reference_id();
                    (connector_mandate_id, token_source, scheme_ref_id)
                }
                _ => {
                    return Err(error_stack::report!(
                        errors::IntegrationError::MissingRequiredField {
                            field_name: "mandate_reference_id.connector_mandate_id",
                            context: errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "expected MandateReferenceId::ConnectorMandateId for repeat payments"
                                        .to_string(),
                                ),
                                doc_url: Some(FISERV_CHARGES_API_VERSION_URL.to_string()),
                                ..Default::default()
                            },
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
                            expiration_year: utils::expand_expiry_year_to_four_digits(year),
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
            source: FiservcommercehubSourceData::PaymentToken(
                FiservcommercehubPaymentTokenSource {
                    token_data: Secret::new(connector_mandate_id),
                    token_source,
                    decline_duplicates: Some(false),
                    card: card_info,
                },
            ),
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

        // Build connector_response with 3DS authentication data or approval code if available
        let connector_response = build_connector_response(
            item.response.0.additional_data_3ds.as_ref(),
            item.response.0.approval_code(),
        );
        let response = build_transaction_payment_response(
            status,
            item.http_code,
            txn,
            item.response.0.payment_tokens.as_ref(),
            item.response.0.payment_receipt.as_ref(),
        );

        Ok(Self {
            response,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSetupMandateRequest {
    pub source: FiservcommercehubSourceData,
    pub merchant_details: FiservcommercehubMerchantDetails,
    pub transaction_details: FiservcommercehubSetupMandateTransactionDetails,
    pub transaction_interaction: FiservcommercehubTransactionInteractionReq,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<FiservcommercehubAdditionalData3DS>,
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
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "SetupMandate is for tokenization only; amount must be 0".to_string(),
                        ),
                        doc_url: Some(FISERV_TOKEN_API_VERSION_URL.to_string()),
                        ..Default::default()
                    },
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

                FiservcommercehubSourceData::PaymentCard(FiservcommercehubPaymentCardSource {
                    encryption_data: FiservcommercehubEncryptionData {
                        key_id: encrypted_card.key_id,
                        encryption_type: ENCRYPTION_TYPE_RSA.to_string(),
                        encryption_block: encrypted_card.encryption_block,
                        encryption_block_fields: encrypted_card.encryption_block_fields,
                    },
                })
            }
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "This payment method is not implemented".to_string(),
                        Default::default()
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservcommercehubSetupMandateResponse {
    pub gateway_response: FiservcommercehubGatewayResponseBody,
    pub payment_receipt: Option<FiservcommercehubPaymentReceipt>,
    pub payment_tokens: Option<FiservcommercehubPaymentTokens>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data_3ds: Option<serde_json::Value>,
}

impl FiservcommercehubSetupMandateResponse {
    fn approval_code(&self) -> Option<String> {
        self.payment_receipt
            .as_ref()
            .and_then(FiservcommercehubPaymentReceipt::approval_code)
    }
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
        let txn_state = &item.response.gateway_response.transaction_state;
        let status = match txn_state {
            FiservcommercehubTransactionState::Authorized => AttemptStatus::Charged,
            _ => AttemptStatus::from(txn_state),
        };

        let connector_response = build_connector_response(
            item.response.additional_data_3ds.as_ref(),
            item.response.approval_code(),
        );
        let response = build_transaction_payment_response(
            status,
            item.http_code,
            txn,
            item.response.payment_tokens.as_ref(),
            item.response.payment_receipt.as_ref(),
        );

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
