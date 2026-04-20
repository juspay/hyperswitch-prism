use common_enums::{AttemptStatus, CaptureMethod};
use common_utils::pii::SecretSerdeValue;
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, Refund, ServerAuthenticationToken, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        JpmorganClientAuthenticationResponse as JpmorganClientAuthenticationResponseDomain,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    },
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{requests, responses, JpmorganAmountConvertor};
use crate::{connectors::jpmorgan::JpmorganRouterData, types::ResponseRouterData, utils};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};

type Error = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

const JPMORGAN_GETTING_STARTED_DOC: &str =
    "https://developer.payments.jpmorgan.com/docs/commerce-solutions/online-payments/guides/getting-started";

/// Build an `IntegrationErrorContext` for a missing JPMorgan connector config field.
fn jpmorgan_missing_field_context(field_name: &str) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(format!(
            "Set the '{}' field in the JPMorgan connector configuration. This is required \
             by JPMorgan's Online Payments API for every payment request.",
            field_name
        )),
        doc_url: Some(JPMORGAN_GETTING_STARTED_DOC.to_owned()),
        additional_context: Some(format!(
            "JPMorgan requires '{}' as a mandatory field in the merchant software or \
             connector configuration.",
            field_name
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub company_name: Option<Secret<String>>,
    pub product_name: Option<Secret<String>>,
    pub merchant_purchase_description: Option<Secret<String>>,
    pub statement_descriptor: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for JpmorganAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Jpmorgan {
                client_id,
                client_secret,
                company_name,
                product_name,
                merchant_purchase_description,
                statement_descriptor,
                ..
            } => Ok(Self {
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                company_name: company_name.clone(),
                product_name: product_name.clone(),
                merchant_purchase_description: merchant_purchase_description.clone(),
                statement_descriptor: statement_descriptor.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

/// JPMorgan connector metadata containing merchant software information
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct JpmorganConnectorMetadataObject {
    pub company_name: Secret<String>,
    pub product_name: Secret<String>,
    pub merchant_purchase_description: Secret<String>,
    pub statement_descriptor: Secret<String>,
}

impl TryFrom<&Option<SecretSerdeValue>> for JpmorganConnectorMetadataObject {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(meta_data: &Option<SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata: Self = utils::to_connector_meta_from_secret::<Self>(meta_data.clone())
            .change_context(IntegrationError::InvalidConnectorConfig {
                config: "merchant_connector_account.metadata",
                context: Default::default(),
            })?;
        Ok(metadata)
    }
}

// OAuth 2.0 transformers
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for requests::JpmorganTokenRequest
{
    type Error = Error;
    fn try_from(
        _item: JpmorganRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: String::from("client_credentials"),
            scope: String::from("jpm:payments:sandbox"),
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganAuthUpdateResponse, Self>>
    for RouterDataV2<
        F,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                token_type: Some(item.response.token_type.clone()),
                expires_in: Some(item.response.expires_in),
            }),
            ..item.router_data
        })
    }
}

fn map_capture_method(
    capture_method: Option<CaptureMethod>,
) -> Result<requests::CapMethod, error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(CaptureMethod::Automatic) | None => Ok(requests::CapMethod::Now),
        Some(CaptureMethod::Manual) => Ok(requests::CapMethod::Manual),
        Some(CaptureMethod::Scheduled)
        | Some(CaptureMethod::ManualMultiple)
        | Some(CaptureMethod::SequentialAutomatic) => {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Capture Method".to_string(),
                connector: "Jpmorgan",
                context: Default::default(),
            }))
        }
    }
}

/// Extract first name and last name from account holder name or billing info
fn extract_account_holder_names<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    _bank_account_holder_name: &Option<Secret<String>>,
) -> Result<(Secret<String>, Secret<String>), error_stack::Report<IntegrationError>> {
    // Use billing address first_name and last_name directly (like Forte connector)
    let first_name = router_data
        .resource_common_data
        .get_billing_first_name()
        .ok()
        .unwrap_or_else(|| Secret::new("".to_string()));

    let last_name = router_data
        .resource_common_data
        .get_optional_billing_last_name()
        .unwrap_or_else(|| first_name.clone());

    Ok((first_name, last_name))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
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

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // JPMorgan doesn't support 3DS for card payments
                if router_data.resource_common_data.auth_type
                    == common_enums::AuthenticationType::ThreeDs
                {
                    return Err(error_stack::report!(IntegrationError::NotSupported {
                        message: "3DS payments".to_string(),
                        connector: "Jpmorgan",
                        context: Default::default(),
                    }));
                }
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant =
                    requests::JpmorganMerchant {
                        merchant_software: requests::JpmorganMerchantSoftware {
                            company_name: auth.company_name.clone().ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "company_name",
                                    context: Default::default(),
                                },
                            )?,
                            product_name: auth.product_name.clone().ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "product_name",
                                    context: Default::default(),
                                },
                            )?,
                        },
                        soft_merchant: requests::JpmorganSoftMerchant {
                            merchant_purchase_description: auth
                                .merchant_purchase_description
                                .clone()
                                .ok_or(IntegrationError::MissingRequiredField {
                                    field_name: "merchant_purchase_description",
                                    context: Default::default(),
                                })?,
                        },
                    };

                let expiry = requests::Expiry {
                    month: Secret::new(
                        card_data
                            .card_exp_month
                            .peek()
                            .parse::<i32>()
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: Default::default(),
                            })?,
                    ),
                    year: Secret::new(
                        card_data
                            .get_expiry_year_4_digit()
                            .peek()
                            .parse::<i32>()
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: Default::default(),
                            })?,
                    ),
                };

                let card = requests::JpmorganCard {
                    account_number: card_data.card_number.clone(),
                    expiry,
                };

                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: Some(card),
                    ach: None,
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                Ok(Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: None,
                    statement_descriptor: None,
                })
            }
            PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                account_number,
                routing_number,
                bank_account_holder_name,
                bank_type,
                ..
            }) => {
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant =
                    requests::JpmorganMerchant {
                        merchant_software: requests::JpmorganMerchantSoftware {
                            company_name: auth.company_name.clone().ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "company_name",
                                    context: Default::default(),
                                },
                            )?,
                            product_name: auth.product_name.clone().ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "product_name",
                                    context: Default::default(),
                                },
                            )?,
                        },
                        soft_merchant: requests::JpmorganSoftMerchant {
                            merchant_purchase_description: auth
                                .merchant_purchase_description
                                .clone()
                                .ok_or(IntegrationError::MissingRequiredField {
                                    field_name: "merchant_purchase_description",
                                    context: Default::default(),
                                })?,
                        },
                    };

                // Extract first name and last name from account holder name or billing info
                let (first_name, last_name) =
                    extract_account_holder_names(router_data, bank_account_holder_name)?;

                let account_holder = requests::JpmorganAccountHolder {
                    first_name,
                    last_name,
                };

                // Determine account type based on bank_type field, default to Checking
                let account_type = if let Some(common_enums::BankType::Savings) = bank_type {
                    requests::JpmorganAchAccountType::Savings
                } else {
                    requests::JpmorganAchAccountType::Checking
                };

                let ach = requests::JpmorganAch {
                    account_number: account_number.clone(),
                    financial_institution_routing_number: routing_number.clone(),
                    account_type,
                };

                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: None,
                    ach: Some(ach),
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                // Get statement_descriptor from connector config
                let statement_descriptor = auth.statement_descriptor.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "statement_descriptor",
                        context: Default::default(),
                    },
                )?;

                Ok(Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: Some(account_holder),
                    statement_descriptor: Some(statement_descriptor),
                })
            }
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let token = token_data.token.clone();

                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant =
                    requests::JpmorganMerchant {
                        merchant_software: requests::JpmorganMerchantSoftware {
                            company_name: auth.company_name.clone().ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "company_name",
                                    context: jpmorgan_missing_field_context("company_name"),
                                },
                            )?,
                            product_name: auth.product_name.clone().ok_or(
                                IntegrationError::MissingRequiredField {
                                    field_name: "product_name",
                                    context: jpmorgan_missing_field_context("product_name"),
                                },
                            )?,
                        },
                        soft_merchant: requests::JpmorganSoftMerchant {
                            merchant_purchase_description: auth
                                .merchant_purchase_description
                                .clone()
                                .ok_or(IntegrationError::MissingRequiredField {
                                    field_name: "merchant_purchase_description",
                                    context: jpmorgan_missing_field_context(
                                        "merchant_purchase_description",
                                    ),
                                })?,
                        },
                    };

                // For CardToken, the token is passed in the payment_method_type
                // instead of raw card details
                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: None,
                    ach: None,
                    googlepay: None,
                    token: Some(token),
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.minor_amount,
                    router_data.request.currency,
                )?;

                let account_holder = requests::JpmorganAccountHolder {
                    first_name: Secret::new("NA".to_string()),
                    last_name: Secret::new("NA".to_string()),
                };
                let statement_descriptor = Secret::new("Statement Descriptor".to_string());

                Ok(Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: Some(account_holder),
                    statement_descriptor: Some(statement_descriptor),
                })
            }
            PaymentMethodData::BankDebit(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Only ACH Bank Debit is supported".to_string(),
                    connector: "Jpmorgan",
                    context: Default::default(),
                }))
            }
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                domain_types::payment_method_data::WalletData::GooglePay(google_pay_data) => {
                    match &google_pay_data.tokenization_data {
                        domain_types::payment_method_data::GpayTokenizationData::Encrypted(
                            encrypted_data,
                        ) => {
                            let capture_method =
                                map_capture_method(router_data.request.capture_method)?;

                            let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                            let merchant = requests::JpmorganMerchant {
                                merchant_software: requests::JpmorganMerchantSoftware {
                                    company_name: auth.company_name.clone().ok_or(
                                        IntegrationError::MissingRequiredField {
                                            field_name: "company_name",
                                            context: Default::default(),
                                        },
                                    )?,
                                    product_name: auth.product_name.clone().ok_or(
                                        IntegrationError::MissingRequiredField {
                                            field_name: "product_name",
                                            context: Default::default(),
                                        },
                                    )?,
                                },
                                soft_merchant: requests::JpmorganSoftMerchant {
                                    merchant_purchase_description: auth
                                        .merchant_purchase_description
                                        .clone()
                                        .ok_or(IntegrationError::MissingRequiredField {
                                            field_name: "merchant_purchase_description",
                                            context: Default::default(),
                                        })?,
                                },
                            };

                            let amount = JpmorganAmountConvertor::convert(
                                router_data.request.minor_amount,
                                router_data.request.currency,
                            )?;

                            // Parse the Google Pay token string into its component fields.
                            // The token is a JSON string containing protocolVersion, signature,
                            // optionally intermediateSigningKey, and signedMessage.
                            let gpay_token: requests::GooglePayToken =
                                serde_json::from_str(&encrypted_data.token).change_context(
                                    IntegrationError::RequestEncodingFailed {
                                        context: Default::default(),
                                    },
                                )?;

                            // Parse signedMessage to extract ephemeralPublicKey.
                            // signedMessage is itself a JSON string.
                            let signed_message: requests::GooglePaySignedMessage =
                                serde_json::from_str(gpay_token.signed_message.peek())
                                    .change_context(
                                        IntegrationError::RequestEncodingFailed {
                                            context: Default::default(),
                                        },
                                    )?;

                            // For ECv2, signature comes from intermediateSigningKey.signatures[0].
                            // For ECv1, signature comes from the top-level signature field.
                            let signature =
                                if let Some(isk) = &gpay_token.intermediate_signing_key {
                                    isk.signatures
                                        .first()
                                        .cloned()
                                        .ok_or(IntegrationError::MissingRequiredField {
                                            field_name: "intermediateSigningKey.signatures[0]",
                                            context: Default::default(),
                                        })?
                                } else {
                                    gpay_token.signature.clone()
                                };

                            let googlepay = requests::JpmorganGooglePay {
                                // latLong is required by JPMorgan; use "0,0" when not available
                                lat_long: "0,0".to_string(),
                                encrypted_payment_bundle: requests::JpmorganEncryptedPaymentBundle {
                                    // encryptedPayload is the raw signedMessage JSON string
                                    encrypted_payload: gpay_token.signed_message.clone(),
                                    encrypted_payment_header:
                                        requests::JpmorganEncryptedPaymentHeader {
                                            ephemeral_public_key: signed_message
                                                .ephemeral_public_key,
                                        },
                                    signature,
                                    protocol_version: gpay_token.protocol_version,
                                },
                            };

                            let payment_method_type = requests::JpmorganPaymentMethodType {
                                card: None,
                                ach: None,
                                googlepay: Some(googlepay),
                                token: None,
                            };

                            Ok(Self {
                                capture_method,
                                currency: router_data.request.currency,
                                amount,
                                merchant,
                                payment_method_type,
                                // account_holder and statement_descriptor are not required
                                // for Google Pay encrypted flow
                                account_holder: None,
                                statement_descriptor: None,
                            })
                        }
                        domain_types::payment_method_data::GpayTokenizationData::Decrypted(_) => {
                            Err(IntegrationError::NotSupported {
                                message: "Decrypted Google Pay token is not supported for JPMorgan; use encrypted flow".to_string(),
                                connector: "jpmorgan",
                                context: Default::default(),
                            }
                            .into())
                        }
                    }
                }
                _ => Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Wallet not supported".to_string(),
                    connector: "Jpmorgan",
                    context: Default::default(),
                })),
            },
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Payment method not supported".to_string(),
                connector: "Jpmorgan",
                context: Default::default(),
            })),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for requests::JpmorganCaptureRequest
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let capture_method = requests::CapMethod::Now;
        let amount_to_capture = item.router_data.request.minor_amount_to_capture;

        let amount =
            JpmorganAmountConvertor::convert(amount_to_capture, item.router_data.request.currency)?;

        // When AuthenticationType is `Manual`, Documentation suggests us to pass `isAmountFinal` field being `true`
        // isAmountFinal is by default `true`. Since Manual Multiple support is not added here, the field is not used.
        Ok(Self {
            capture_method,
            amount,
            currency: item.router_data.request.currency,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for requests::JpmorganVoidRequest
{
    type Error = Error;
    fn try_from(
        _item: JpmorganRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self { is_void: true })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for requests::JpmorganRefundRequest
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = JpmorganAuthType::try_from(&item.router_data.connector_config)?;

        let merchant = requests::JpmorganMerchantRefund {
            merchant_software: requests::JpmorganMerchantSoftware {
                company_name: auth
                    .company_name
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "company_name",
                        context: Default::default(),
                    })?,
                product_name: auth
                    .product_name
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "product_name",
                        context: Default::default(),
                    })?,
            },
        };

        let amount = JpmorganAmountConvertor::convert(
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            merchant,
            amount,
            currency: item.router_data.request.currency,
        })
    }
}

fn map_transaction_state_to_attempt_status(
    transaction_state: &responses::JpmorganTransactionState,
    capture_method: &Option<requests::CapMethod>,
) -> AttemptStatus {
    match transaction_state {
        responses::JpmorganTransactionState::Closed => match capture_method {
            Some(requests::CapMethod::Now) => AttemptStatus::Charged,
            _ => AttemptStatus::Authorized,
        },
        responses::JpmorganTransactionState::Authorized => AttemptStatus::Authorized,
        responses::JpmorganTransactionState::Declined
        | responses::JpmorganTransactionState::Error => AttemptStatus::Failure,
        responses::JpmorganTransactionState::Pending => AttemptStatus::Pending,
        responses::JpmorganTransactionState::Voided => AttemptStatus::Voided,
    }
}

impl TryFrom<&responses::JpmorganPaymentsResponse> for PaymentsResponseData {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganPaymentsResponse) -> Result<Self, Self::Error> {
        Ok(Self::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(item.request_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.response_code.parse::<u16>().unwrap_or(0),
        })
    }
}

impl TryFrom<&responses::JpmorganPaymentsResponse> for AttemptStatus {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganPaymentsResponse) -> Result<Self, Self::Error> {
        Ok(map_transaction_state_to_attempt_status(
            &item.transaction_state,
            &item.capture_method,
        ))
    }
}

impl TryFrom<&responses::JpmorganRefundResponse> for RefundsResponseData {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganRefundResponse) -> Result<Self, Self::Error> {
        let refund_status = responses::RefundStatus::from((
            item.response_status.clone(),
            item.transaction_state.clone(),
        ))
        .into();

        Ok(Self {
            connector_refund_id: item.transaction_id.clone(),
            refund_status,
            status_code: item.response_code.parse::<u16>().unwrap_or(0),
        })
    }
}

// Bridge pattern implementations for RouterDataV2

impl<T: PaymentMethodDataTypes, F>
    TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response_data = PaymentsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response_data = PaymentsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response_data = PaymentsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response_data = PaymentsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = responses::RefundStatus::from((
            item.response.response_status.clone(),
            item.response.transaction_state.clone(),
        ))
        .into();
        let response_data = RefundsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = responses::RefundStatus::from((
            item.response.response_status.clone(),
            item.response.transaction_state.clone(),
        ))
        .into();
        let response_data = RefundsResponseData::try_from(&item.response)?;

        Ok(Self {
            response: Ok(response_data),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Obtains an OAuth2 access token from JPMorgan for client-side SDK initialization.
/// The access_token serves as the client authentication token.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: JpmorganRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let scope = if item
            .router_data
            .resource_common_data
            .test_mode
            .unwrap_or(true)
        {
            String::from("jpm:payments:sandbox")
        } else {
            String::from("jpm:payments")
        };
        Ok(Self {
            grant_type: String::from("client_credentials"),
            scope,
        })
    }
}

impl TryFrom<ResponseRouterData<responses::JpmorganClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Jpmorgan(
                JpmorganClientAuthenticationResponseDomain {
                    transaction_id: response.access_token.peek().to_string(),
                    request_id: response.token_type,
                },
            ),
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
