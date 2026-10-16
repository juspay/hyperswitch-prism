use common_enums::{AttemptStatus, CaptureMethod};
use common_utils::consts::NO_ERROR_MESSAGE;
use common_utils::{fp_utils::when, pii::SecretSerdeValue};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, Refund, RepeatPayment,
        ServerAuthenticationToken, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        JpmorganClientAuthenticationResponse as JpmorganClientAuthenticationResponseDomain,
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, SetupMandateRequestData,
    },
    mandates::{MandateData, MandateDataType},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        ApplePayDecryptedData, ApplePayPaymentData, BankDebitData, GooglePayDecryptedData,
        PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{requests, responses, JpmorganAmountConvertor};
use crate::{connectors::jpmorgan::JpmorganRouterData, types::ResponseRouterData, utils};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::utils::is_payment_failure;

type Error = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

const JPMORGAN_GETTING_STARTED_DOC: &str =
    "https://developer.payments.jpmorgan.com/docs/commerce-solutions/online-payments/guides/getting-started";
const JPMORGAN_THREE_DS_NOT_IMPLEMENTED: &str = "3DS payments";
// JPMorgan Online Payments 2.21.0 defines these limits in transactionId and agreementId.
const JPMORGAN_TRANSACTION_ID_MIN_LENGTH: usize = 4;
const JPMORGAN_TRANSACTION_ID_MAX_LENGTH: usize = 40;
const JPMORGAN_AGREEMENT_ID_MAX_LENGTH: usize = 100;

fn response_card_network(card: Option<&responses::Card>) -> Option<common_enums::CardNetwork> {
    match card?.card_type.as_ref()?.peek().as_str() {
        "VI" => Some(common_enums::CardNetwork::Visa),
        "MC" => Some(common_enums::CardNetwork::Mastercard),
        "AX" => Some(common_enums::CardNetwork::AmericanExpress),
        _ => None,
    }
}

fn pan_card_network(number: &str) -> Option<common_enums::CardNetwork> {
    use domain_types::utils::CardIssuer;
    Some(match domain_types::utils::get_card_issuer(number).ok()? {
        CardIssuer::Visa => common_enums::CardNetwork::Visa,
        CardIssuer::Master => common_enums::CardNetwork::Mastercard,
        CardIssuer::Maestro => common_enums::CardNetwork::Maestro,
        CardIssuer::AmericanExpress => common_enums::CardNetwork::AmericanExpress,
        CardIssuer::Discover => common_enums::CardNetwork::Discover,
        CardIssuer::DinersClub | CardIssuer::CarteBlanche => common_enums::CardNetwork::DinersClub,
        CardIssuer::JCB => common_enums::CardNetwork::JCB,
        CardIssuer::CartesBancaires => common_enums::CardNetwork::CartesBancaires,
        CardIssuer::UnionPay => common_enums::CardNetwork::UnionPay,
    })
}

fn initial_wallet_context<T: PaymentMethodDataTypes>(
    payment_method: &PaymentMethodData<T>,
    card: Option<&responses::Card>,
) -> Option<requests::JpmorganStoredWallet> {
    let (provider, account_number_type) = match payment_method {
        PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) if matches!(&wallet.payment_data, ApplePayPaymentData::Decrypted(data) if data.merchant_token_identifier.is_none()) => {
            (
                requests::JpmorganWalletProvider::ApplePay,
                requests::JpmorganAccountNumberType::DeviceToken,
            )
        }
        PaymentMethodData::Wallet(WalletData::GooglePay(wallet)) => {
            let domain_types::payment_method_data::GpayTokenizationData::Decrypted(data) =
                &wallet.tokenization_data
            else {
                return None;
            };
            (
                requests::JpmorganWalletProvider::GooglePay,
                if data.cryptogram.is_some() {
                    requests::JpmorganAccountNumberType::DeviceToken
                } else {
                    requests::JpmorganAccountNumberType::Pan
                },
            )
        }
        _ => return None,
    };
    Some(requests::JpmorganStoredWallet {
        provider,
        account_number_type,
        network: response_card_network(card),
    })
}

fn safetech_mandate<T: PaymentMethodDataTypes>(
    payment_method: &PaymentMethodData<T>,
    handle: &str,
    response_card: Option<&responses::Card>,
    recurring: Option<requests::JpmorganRecurring>,
    http_code: u16,
) -> Result<Option<Box<MandateReference>>, ResponseError> {
    let PaymentMethodData::Card(card) = payment_method else {
        return Ok(None);
    };
    if handle.trim().is_empty() {
        return Ok(None);
    }
    let Some(response_card) = response_card else {
        return Ok(None);
    };
    let tokens = response_card.payment_tokens.as_deref().unwrap_or_default();
    let mut successful = tokens.iter().filter(|token| {
        token.token_provider.as_deref() == Some("SAFETECH")
            && token.response_status.as_deref() == Some("SUCCESS")
    });
    let Some(token) = successful.next() else {
        return Ok(None);
    };
    let Some(token_number) = token
        .token_number
        .as_ref()
        .filter(|number| !number.peek().trim().is_empty())
    else {
        return Ok(None);
    };
    if successful.next().is_some() {
        return Ok(None);
    }
    let Some(network) = response_card_network(Some(response_card)) else {
        return Ok(None);
    };
    let Some(references) = &response_card.network_response else {
        return Ok(None);
    };
    let Some(ntid) = references
        .network_transaction_id
        .as_ref()
        .filter(|id| !id.trim().is_empty())
    else {
        return Ok(None);
    };
    if references
        .transaction_link_id
        .as_ref()
        .is_some_and(|id| id.trim().is_empty())
        || (network == common_enums::CardNetwork::Mastercard
            && references.transaction_link_id.is_none())
    {
        return Ok(None);
    }
    let credential = requests::JpmorganSafetechCredential {
        handle: handle.to_owned(),
        token: token_number.clone(),
        expiry: requests::Expiry {
            month: Secret::new(i32::from(
                card.get_expiry_month_as_u8()
                    .map_err(|_| requests::JpmorganSyncResource::response_error(http_code))?,
            )),
            year: Secret::new(i32::from(
                card.get_expiry_year_4_digit_as_u16()
                    .map_err(|_| requests::JpmorganSyncResource::response_error(http_code))?,
            )),
        },
        network,
        original_network_transaction_id: ntid.clone(),
        original_transaction_link_id: references.transaction_link_id.clone(),
        recurring,
    };
    Ok(Some(Box::new(MandateReference {
        connector_mandate_id: Some(handle.to_owned()),
        payment_method_id: None,
        connector_mandate_request_reference_id: None,
        mandate_metadata: Some(Secret::new(
            serde_json::to_value(credential)
                .map_err(|_| requests::JpmorganSyncResource::response_error(http_code))?,
        )),
    })))
}

impl TryFrom<Option<common_enums::BankType>> for requests::JpmorganAchAccountType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(bank_type: Option<common_enums::BankType>) -> Result<Self, Self::Error> {
        match bank_type {
            Some(common_enums::BankType::Savings) => Ok(Self::Savings),
            Some(common_enums::BankType::Checking) | None => Ok(Self::Checking),
            Some(bank) => Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!("Bank type {bank:?} is not supported by jpmorgan"),
                connector: "jpmorgan",
                context: IntegrationErrorContext {
                    suggested_action: Some("Provide a valid bank account type".to_owned()),
                    additional_context: None,
                    doc_url: None,
                },
            })),
        }
    }
}

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
                MerchantAuthenticationFlowData,
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
                MerchantAuthenticationFlowData,
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
        MerchantAuthenticationFlowData,
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

fn merchant_order_number(merchant_order_id: Option<&str>, reference: &str) -> String {
    merchant_order_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or(reference)
        .to_owned()
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

impl<T: PaymentMethodDataTypes> requests::JpmorganCard<T> {
    fn wallet_field_context(field_name: &'static str) -> IntegrationErrorContext {
        IntegrationErrorContext {
            suggested_action: Some(format!(
                "Supply a valid {field_name} from the decrypted wallet payload."
            )),
            ..Default::default()
        }
    }

    fn invalid_wallet_field(field_name: &'static str) -> IntegrationError {
        IntegrationError::InvalidDataFormat {
            field_name,
            context: Self::wallet_field_context(field_name),
        }
    }

    fn wallet_authentication(
        cryptogram: &Secret<String>,
        eci: &Option<String>,
    ) -> Result<requests::JpmorganWalletAuthentication, Error> {
        if cryptogram.peek().trim().is_empty() {
            return Err(Self::invalid_wallet_field("token_cryptogram").into());
        }
        Ok(requests::JpmorganWalletAuthentication {
            token_authentication_value: cryptogram.clone(),
            electronic_commerce_indicator: eci.clone(),
        })
    }

    fn network_token_authentication(
        token: &domain_types::payment_method_data::NetworkTokenData,
        cryptogram: &Secret<String>,
    ) -> Result<requests::JpmorganWalletAuthentication, Error> {
        let eci = match &token.eci {
            Some(eci) if eci.trim().is_empty() => {
                return Err(Self::invalid_wallet_field("network_token.eci").into())
            }
            Some(eci) => eci.clone(),
            None => match token.card_network.as_ref() {
                Some(common_enums::CardNetwork::Visa) => "7".to_owned(),
                Some(common_enums::CardNetwork::Mastercard) => "5".to_owned(),
                _ => {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "network_token.eci",
                        context: Self::wallet_field_context("network_token.eci"),
                    }
                    .into())
                }
            },
        };
        Self::wallet_authentication(cryptogram, &Some(eci))
    }

    fn validate_apple_pay_storage(payment_method: &PaymentMethodData<T>) -> Result<(), Error> {
        if matches!(payment_method, PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) if matches!(&wallet.payment_data, ApplePayPaymentData::Decrypted(data) if data.merchant_token_identifier.is_some()))
        {
            return Err(IntegrationError::NotSupported {
                message:
                    "Apple Pay merchant-token storage without a documented payment account-number type"
                        .to_owned(),
                connector: "jpmorgan",
                context: Self::wallet_field_context("apple_pay.merchant_token_identifier"),
            }
            .into());
        }
        Ok(())
    }

    fn from_apple_pay(data: &ApplePayDecryptedData) -> Result<Self, Error> {
        Ok(Self {
            account_number: RawCardNumber(T::inner_from_card_number(
                data.application_primary_account_number.clone(),
            )),
            expiry: requests::Expiry {
                month: Secret::new(
                    data.get_expiry_month()
                        .peek()
                        .parse::<i32>()
                        .change_context(Self::invalid_wallet_field("wallet.expiry.month"))?,
                ),
                year: Secret::new(
                    data.get_four_digit_expiry_year()
                        .peek()
                        .parse::<i32>()
                        .change_context(Self::invalid_wallet_field("wallet.expiry.year"))?,
                ),
            },
            account_number_type: Some(requests::JpmorganAccountNumberType::DeviceToken),
            wallet_provider: Some(requests::JpmorganWalletProvider::ApplePay),
            authentication: Some(Self::wallet_authentication(
                &data.payment_data.online_payment_cryptogram,
                &data.payment_data.eci_indicator,
            )?),
        })
    }

    fn from_google_pay(data: &GooglePayDecryptedData) -> Result<Self, Error> {
        // A present cryptogram classifies the payload as a 3DS cryptogram
        // (DEVICE_TOKEN); otherwise the payload is PAN-only.
        let (account_number_type, authentication) = match data.cryptogram.as_ref() {
            Some(cryptogram) => (
                requests::JpmorganAccountNumberType::DeviceToken,
                Some(Self::wallet_authentication(
                    cryptogram,
                    &data.eci_indicator,
                )?),
            ),
            None => {
                if data.eci_indicator.is_some() {
                    return Err(Self::invalid_wallet_field("google_pay.cryptogram").into());
                }
                (requests::JpmorganAccountNumberType::Pan, None)
            }
        };
        let exp_month = data
            .get_expiry_month()
            .change_context(Self::invalid_wallet_field("wallet.expiry.month"))?;
        let exp_year = data
            .get_four_digit_expiry_year()
            .change_context(Self::invalid_wallet_field("wallet.expiry.year"))?;

        Ok(Self {
            account_number: RawCardNumber(T::inner_from_card_number(
                data.application_primary_account_number.clone(),
            )),
            expiry: requests::Expiry {
                month: Secret::new(
                    exp_month
                        .peek()
                        .parse::<i32>()
                        .change_context(Self::invalid_wallet_field("wallet.expiry.month"))?,
                ),
                year: Secret::new(
                    exp_year
                        .peek()
                        .parse::<i32>()
                        .change_context(Self::invalid_wallet_field("wallet.expiry.year"))?,
                ),
            },
            account_number_type: Some(account_number_type),
            wallet_provider: Some(requests::JpmorganWalletProvider::GooglePay),
            authentication,
        })
    }
}

impl requests::JpmorganRecurring {
    fn from_initial_context(
        category: Option<&common_enums::MitCategory>,
        mandate: Option<&MandateData>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<Option<Self>, Error> {
        if metadata.is_some_and(|metadata| !metadata.is_object()) {
            return Err(requests::JpmorganStoredContext::invalid());
        }
        let Some(value) = metadata.and_then(|metadata| metadata.get("jpmorgan")) else {
            return Self::from_optional_mandate_terms(category, mandate);
        };
        let context: requests::JpmorganPreservedContext = serde_json::from_value(value.clone())
            .map_err(|_| requests::JpmorganStoredContext::invalid())?;
        let recurring = context
            .recurring
            .ok_or_else(requests::JpmorganStoredContext::invalid)?;
        if context.stored_credential.is_some()
            || context.wallet.is_some()
            || context.wallet_account_number_type.is_some()
            || !matches!(
                category,
                Some(
                    common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription
                )
            )
            || recurring.recurring_sequence != requests::JpmorganRecurringSequence::First
            || recurring.agreement_id.trim().is_empty()
            || recurring.agreement_id.chars().count() > JPMORGAN_AGREEMENT_ID_MAX_LENGTH
            || recurring.is_variable_amount.is_none()
        {
            return Err(requests::JpmorganStoredContext::invalid());
        }
        Self::validate_supplied_terms(Some(&recurring), mandate)?;
        if mandate.is_some_and(|mandate| mandate.update_mandate_id.is_some()) {
            return Err(requests::JpmorganStoredContext::invalid());
        }
        Ok(Some(recurring))
    }

    fn input_context() -> IntegrationErrorContext {
        IntegrationErrorContext {
            suggested_action: Some(
                "Supply the original agreement identifier and an explicit exact or variable amount classification.".to_owned(),
            ),
            ..Default::default()
        }
    }

    fn from_optional_mandate_terms(
        category: Option<&common_enums::MitCategory>,
        mandate: Option<&MandateData>,
    ) -> Result<Option<Self>, Error> {
        match category {
            None | Some(common_enums::MitCategory::Unscheduled) => return Ok(None),
            Some(
                common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription,
            ) => (),
            Some(
                common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission,
            ) => {
                return Err(IntegrationError::NotSupported {
                    message: "Installment or resubmission agreements".to_owned(),
                    connector: "jpmorgan",
                    context: Self::input_context(),
                }
                .into());
            }
        }
        let (agreement_id, amount_type) = Self::supplied_recurring_fields(mandate)?;
        let agreement_id = agreement_id.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "metadata.jpmorgan.recurring.agreementId",
            context: Self::input_context(),
        })?;
        let amount_type = amount_type.ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "metadata.jpmorgan.recurring.isVariableAmount",
            context: Self::input_context(),
        })?;
        Ok(Some(Self {
            recurring_sequence: requests::JpmorganRecurringSequence::First,
            agreement_id: agreement_id.to_owned(),
            is_variable_amount: Some(amount_type == requests::JpmorganAmountType::Variable),
        }))
    }

    // Only these two optional fields supply JPMorgan recurring information.
    // Mandate amounts, currencies, limits, dates, and acceptance do not construct this object.
    fn supplied_recurring_fields(
        mandate: Option<&MandateData>,
    ) -> Result<(Option<&str>, Option<requests::JpmorganAmountType>), Error> {
        let terms = match mandate.and_then(|data| data.mandate_type.as_ref()) {
            Some(MandateDataType::MultiUse(Some(terms))) => Some(terms),
            Some(MandateDataType::SingleUse(_)) => {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "setup_mandate_details.mandate_type",
                    context: Self::input_context(),
                }
                .into());
            }
            Some(MandateDataType::MultiUse(None)) | None => None,
        };
        let amount_type = terms
            .and_then(|terms| terms.amount_type.as_deref())
            .map(|amount_type| {
                amount_type
                    .parse::<requests::JpmorganAmountType>()
                    .map_err(|_| IntegrationError::InvalidDataFormat {
                        field_name: "setup_mandate_details.mandate_type.multi_use.amount_type",
                        context: Self::input_context(),
                    })
            })
            .transpose()?;
        let agreement_id = terms.and_then(|terms| terms.external_subscription_id.as_deref());
        if agreement_id.is_some_and(|agreement_id| {
            agreement_id.trim().is_empty()
                || agreement_id.chars().count() > JPMORGAN_AGREEMENT_ID_MAX_LENGTH
        }) {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "setup_mandate_details.mandate_type.multi_use.external_subscription_id",
                context: Self::input_context(),
            }
            .into());
        }
        Ok((agreement_id, amount_type))
    }

    fn validate_supplied_terms(
        recurring: Option<&Self>,
        mandate: Option<&MandateData>,
    ) -> Result<(), Error> {
        let (agreement_id, amount_type) = Self::supplied_recurring_fields(mandate)?;
        if agreement_id.is_some_and(|agreement_id| {
            recurring.is_none_or(|recurring| recurring.agreement_id != agreement_id)
        }) || amount_type.is_some_and(|amount_type| {
            recurring.is_none_or(|recurring| {
                recurring.is_variable_amount
                    != Some(amount_type == requests::JpmorganAmountType::Variable)
            })
        }) {
            return Err(requests::JpmorganStoredContext::invalid());
        }
        Ok(())
    }
}

impl<T: PaymentMethodDataTypes> requests::JpmorganPaymentsRequest<T> {
    fn has_storage_intent(request: &PaymentsAuthorizeData<T>) -> bool {
        matches!(
            request.setup_future_usage,
            Some(common_enums::FutureUsage::OffSession)
        ) || request.setup_mandate_details.is_some()
            || request.tokenization == Some(common_enums::Tokenization::TokenizeAtPsp)
            || request.mit_category.is_some()
            || request
                .metadata
                .as_ref()
                .is_some_and(|data| data.peek().get("jpmorgan").is_some())
    }

    fn supports_initial_storage(payment_method: &PaymentMethodData<T>) -> bool {
        match payment_method {
            PaymentMethodData::Card(_) | PaymentMethodData::NetworkToken(_) => true,
            PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) => {
                matches!(wallet.payment_data, ApplePayPaymentData::Decrypted(_))
            }
            PaymentMethodData::Wallet(WalletData::GooglePay(wallet)) => matches!(
                wallet.tokenization_data,
                domain_types::payment_method_data::GpayTokenizationData::Decrypted(_)
            ),
            _ => false,
        }
    }

    fn with_initial_storage(
        mut self,
        request: &PaymentsAuthorizeData<T>,
        reference: &str,
    ) -> Result<Self, Error> {
        if let Some(context) = requests::JpmorganStoredContext::from_request(request, reference)? {
            requests::JpmorganCard::validate_apple_pay_storage(&request.payment_method_data)?;
            Self::validate_cit_context(request)?;
            if let Some(wallet) = context.wallet {
                let card = self
                    .payment_method_type
                    .card
                    .as_ref()
                    .ok_or_else(requests::JpmorganStoredContext::invalid)?;
                if card.wallet_provider.as_ref() != Some(&wallet.provider)
                    || card.account_number_type.as_ref() != Some(&wallet.account_number_type)
                {
                    return Err(requests::JpmorganStoredContext::invalid());
                }
            }
            self.initiator_type = Some(requests::JpmorganInitiatorType::Cardholder);
            self.account_on_file = Some(requests::JpmorganAccountOnFile::Stored);
            self.is_amount_final = Some(true);
            self.merchant_order_number = Some(merchant_order_number(
                request.merchant_order_id.as_deref(),
                reference,
            ));
            self.recurring = None;
            if let Some(card) = self.payment_method_type.card.as_mut() {
                card.account_number_type
                    .get_or_insert(requests::JpmorganAccountNumberType::Pan);
            }
            return Ok(self);
        }
        let storage_requested = matches!(
            request.setup_future_usage,
            Some(common_enums::FutureUsage::OffSession)
        ) || request.setup_mandate_details.is_some()
            || request.tokenization == Some(common_enums::Tokenization::TokenizeAtPsp);
        if !storage_requested && request.mit_category.is_none() {
            return Ok(self);
        }
        if !storage_requested {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "setup_future_usage",
                context: requests::JpmorganRecurring::input_context(),
            }
            .into());
        }
        requests::JpmorganCard::validate_apple_pay_storage(&request.payment_method_data)?;
        Self::validate_cit_context(request)?;
        if let Some(mandate) = request.setup_mandate_details.as_ref() {
            let has_acceptance =
                mandate.customer_acceptance.is_some() || request.customer_acceptance.is_some();
            if mandate.update_mandate_id.is_some()
                || (mandate.mandate_type.is_none() && !has_acceptance)
            {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "setup_mandate_details",
                    context: requests::JpmorganRecurring::input_context(),
                }
                .into());
            }
        }
        self.recurring = requests::JpmorganRecurring::from_initial_context(
            request.mit_category.as_ref(),
            request.setup_mandate_details.as_ref(),
            request.metadata.as_ref().map(|data| data.peek()),
        )?;
        self.initiator_type = Some(requests::JpmorganInitiatorType::Cardholder);
        self.account_on_file = Some(requests::JpmorganAccountOnFile::ToBeStored);
        self.is_amount_final = Some(true);
        self.merchant_order_number = Some(merchant_order_number(
            request.merchant_order_id.as_deref(),
            reference,
        ));
        if let Some(card) = self.payment_method_type.card.as_mut() {
            card.account_number_type
                .get_or_insert(requests::JpmorganAccountNumberType::Pan);
        }
        Ok(self)
    }

    fn validate_cit_context(request: &PaymentsAuthorizeData<T>) -> Result<(), Error> {
        if request.off_session == Some(true)
            || request.mandate_id.is_some()
            || request.authentication_data.is_some()
            || (request.tokenization == Some(common_enums::Tokenization::TokenizeAtPsp)
                && !matches!(request.payment_method_data, PaymentMethodData::Card(_)))
            || request.request_incremental_authorization == Some(true)
            || request.enable_overcapture == Some(true)
            || request.request_extended_authorization == Some(true)
        {
            return Err(IntegrationError::NotSupported {
                message: "Cardholder-initiated payments with merchant initiation, separate authentication, non-card PSP tokenization, incremental authorization, overcapture, or extended authorization".to_owned(),
                connector: "jpmorgan",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Submit a cardholder-initiated payment without these authorization options.".to_owned(),
                    ),
                    ..Default::default()
                },
            }
            .into());
        }
        Ok(())
    }

    fn from_decrypted_wallet(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        wallet_card: requests::JpmorganCard<T>,
    ) -> Result<Self, Error> {
        if router_data.request.authentication_data.is_some() {
            return Err(IntegrationError::NotSupported {
                message: "Separate 3DS authentication for decrypted wallet payments".to_owned(),
                connector: "jpmorgan",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Supply wallet authentication proof without a separate 3DS flow."
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            }
            .into());
        }
        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;
        let request = Self {
            capture_method: map_capture_method(router_data.request.capture_method)?,
            amount: JpmorganAmountConvertor::convert(
                router_data.request.amount.amount,
                router_data.request.currency,
            )?,
            currency: router_data.request.currency,
            merchant: requests::JpmorganMerchant::try_from(&auth)?,
            payment_method_type: requests::JpmorganPaymentMethodType {
                card: Some(wallet_card),
                ach: None,
                googlepay: None,
                token: None,
            },
            account_holder: None,
            statement_descriptor: None,
            merchant_order_number: Some(merchant_order_number(
                router_data.request.merchant_order_id.as_deref(),
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )),
            initiator_type: Some(requests::JpmorganInitiatorType::Cardholder),
            account_on_file: Some(requests::JpmorganAccountOnFile::NotStored),
            is_amount_final: Some(true),
            recurring: None,
        };
        request.with_initial_storage(
            &router_data.request,
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )
    }
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

        if Self::has_storage_intent(&router_data.request)
            && !Self::supports_initial_storage(&router_data.request.payment_method_data)
        {
            return Err(IntegrationError::NotSupported {
                message: "Initial credential storage for this payment method".to_owned(),
                connector: "jpmorgan",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Supply a card or a merchant-decrypted Apple Pay or Google Pay credential."
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            }
            .into());
        }
        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // JPMorgan doesn't support 3DS for card payments
                if router_data.resource_common_data.auth_type
                    == common_enums::AuthenticationType::ThreeDs
                {
                    return Err(IntegrationError::NotImplemented(
                        JPMORGAN_THREE_DS_NOT_IMPLEMENTED.to_string(),
                        Default::default(),
                    )
                    .into());
                }
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                let exp_month_str = card_data.card_exp_month.peek().to_string();
                let exp_year_str = card_data.get_expiry_year_4_digit().peek().to_string();

                // Vault token placeholders (e.g. "{{$card_exp_month}}") cannot be parsed as i32.
                // JPMorgan requires numeric expiry values, so proxy flows are not supported.
                when(
                    exp_month_str.contains("{{") || exp_year_str.contains("{{"),
                    || {
                        Err(error_stack::report!(IntegrationError::NotSupported {
                            message: "JPMorgan requires numeric expiry values; vault token placeholders are not supported for proxy flows".to_string(),
                            connector: "Jpmorgan",
                            context: Default::default(),
                        }))
                    },
                )?;

                let expiry = requests::Expiry {
                    month: Secret::new(exp_month_str.parse::<i32>().change_context(
                        IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        },
                    )?),
                    year: Secret::new(exp_year_str.parse::<i32>().change_context(
                        IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        },
                    )?),
                };

                let card = requests::JpmorganCard {
                    account_number: card_data.card_number.clone(),
                    expiry,
                    account_number_type: None,
                    wallet_provider: None,
                    authentication: None,
                };

                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: Some(card),
                    ach: None,
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.amount.amount,
                    router_data.request.currency,
                )?;

                let request = Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: None,
                    statement_descriptor: None,
                    merchant_order_number: None,
                    initiator_type: None,
                    account_on_file: None,
                    is_amount_final: None,
                    recurring: None,
                };
                Ok(request.with_initial_storage(
                    &router_data.request,
                    &router_data
                        .resource_common_data
                        .connector_request_reference_id,
                )?)
            }
            PaymentMethodData::NetworkToken(token_data) => {
                Self::validate_cit_context(&router_data.request)?;
                if router_data.resource_common_data.auth_type
                    == common_enums::AuthenticationType::ThreeDs
                {
                    return Err(IntegrationError::NotImplemented(
                        JPMORGAN_THREE_DS_NOT_IMPLEMENTED.to_string(),
                        Default::default(),
                    )
                    .into());
                }
                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                let cryptogram = token_data.token_cryptogram.as_ref().ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                        field_name: "network_token.token_cryptogram",
                        context: requests::JpmorganCard::<T>::wallet_field_context(
                            "network_token.token_cryptogram",
                        ),
                    }
                })?;
                let authentication =
                    Some(requests::JpmorganCard::<T>::network_token_authentication(
                        token_data, cryptogram,
                    )?);

                let card = requests::JpmorganCard {
                    account_number: RawCardNumber(T::inner_from_card_number(
                        token_data
                            .token_number
                            .get_card_no()
                            .parse::<cards::CardNumber>()
                            .map_err(|_| {
                                requests::JpmorganCard::<T>::invalid_wallet_field(
                                    "network_token.token_number",
                                )
                            })?,
                    )),
                    expiry: requests::Expiry {
                        month: Secret::new(get_network_token_expiry_month_i32(
                            &token_data.get_network_token_expiry_month(),
                            requests::JpmorganCard::<T>::invalid_wallet_field(
                                "wallet.expiry.month",
                            ),
                        )?),
                        year: Secret::new(
                            token_data
                                .get_expiry_year_4_digit()
                                .peek()
                                .parse::<i32>()
                                .change_context(
                                    requests::JpmorganCard::<T>::invalid_wallet_field(
                                        "wallet.expiry.year",
                                    ),
                                )?,
                        ),
                    },
                    account_number_type: Some(requests::JpmorganAccountNumberType::NetworkToken),
                    wallet_provider: None,
                    authentication,
                };

                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: Some(card),
                    ach: None,
                    googlepay: None,
                    token: None,
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.amount.amount,
                    router_data.request.currency,
                )?;

                let request = Self {
                    capture_method,
                    currency: router_data.request.currency,
                    amount,
                    merchant,
                    payment_method_type,
                    account_holder: None,
                    statement_descriptor: None,
                    merchant_order_number: None,
                    initiator_type: None,
                    account_on_file: None,
                    is_amount_final: None,
                    recurring: None,
                };
                request.with_initial_storage(
                    &router_data.request,
                    &router_data
                        .resource_common_data
                        .connector_request_reference_id,
                )
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

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                // Extract first name and last name from account holder name or billing info
                let (first_name, last_name) =
                    extract_account_holder_names(router_data, bank_account_holder_name)?;

                let account_holder = requests::JpmorganAccountHolder {
                    first_name,
                    last_name,
                };

                // Determine account type based on bank_type field, default to Checking
                let account_type = requests::JpmorganAchAccountType::try_from(*bank_type)?;

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
                    router_data.request.amount.amount,
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
                    merchant_order_number: None,
                    initiator_type: None,
                    account_on_file: None,
                    is_amount_final: None,
                    recurring: None,
                })
            }
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let token = token_data.token.clone();

                let capture_method = map_capture_method(router_data.request.capture_method)?;

                let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                // For CardToken, the token is passed in the payment_method_type
                // instead of raw card details
                let payment_method_type = requests::JpmorganPaymentMethodType {
                    card: None,
                    ach: None,
                    googlepay: None,
                    token: Some(token),
                };

                let amount = JpmorganAmountConvertor::convert(
                    router_data.request.amount.amount,
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
                    merchant_order_number: None,
                    initiator_type: None,
                    account_on_file: None,
                    is_amount_final: None,
                    recurring: None,
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
                WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
                    ApplePayPaymentData::Decrypted(data) => Self::from_decrypted_wallet(
                        router_data,
                        requests::JpmorganCard::from_apple_pay(data)?,
                    ),
                    ApplePayPaymentData::Encrypted(_) => Err(IntegrationError::NotImplemented(
                        "Encrypted Apple Pay payments".to_owned(),
                        IntegrationErrorContext {
                            suggested_action: Some(
                                "Supply merchant-decrypted Apple Pay data.".to_owned(),
                            ),
                            ..Default::default()
                        },
                    )
                    .into()),
                },
                WalletData::GooglePay(google_pay_data) => {
                    match &google_pay_data.tokenization_data {
                        domain_types::payment_method_data::GpayTokenizationData::Encrypted(
                            encrypted_data,
                        ) => {
                            let capture_method =
                                map_capture_method(router_data.request.capture_method)?;

                            let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;

                            let merchant = requests::JpmorganMerchant::try_from(&auth)?;

                            let amount = JpmorganAmountConvertor::convert(
                                router_data.request.amount.amount,
                                router_data.request.currency,
                            )?;

                            // Parse the Google Pay token string into its component fields.
                            // The token is a JSON string containing protocolVersion, signature,
                            // optionally intermediateSigningKey, and signedMessage.
                            let gpay_token: requests::GooglePayToken = serde_json::from_str(
                                &encrypted_data.token,
                            )
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: IntegrationErrorContext {
                                    suggested_action: Some(
                                        "Supply a valid Google Pay payment token from the wallet."
                                            .to_owned(),
                                    ),
                                    additional_context: Some(
                                        "The encrypted Google Pay token must be a JSON object with protocolVersion, signature, and signedMessage."
                                            .to_owned(),
                                    ),
                                    ..Default::default()
                                },
                            })?;

                            // Parse signedMessage to extract ephemeralPublicKey.
                            // signedMessage is itself a JSON string.
                            let signed_message: requests::GooglePaySignedMessage =
                                serde_json::from_str(gpay_token.signed_message.peek())
                                    .change_context(IntegrationError::RequestEncodingFailed {
                                        context: IntegrationErrorContext {
                                            suggested_action: Some(
                                                "Supply a signedMessage JSON payload inside the Google Pay token."
                                                    .to_owned(),
                                            ),
                                            additional_context: Some(
                                                "The signedMessage value must be a JSON object containing ephemeralPublicKey."
                                                    .to_owned(),
                                            ),
                                            ..Default::default()
                                        },
                                    })?;

                            // For ECv2, signature comes from intermediateSigningKey.signatures[0].
                            // For ECv1, signature comes from the top-level signature field.
                            let signature = if let Some(isk) = &gpay_token.intermediate_signing_key
                            {
                                isk.signatures.first().cloned().ok_or(
                                    IntegrationError::MissingRequiredField {
                                        field_name: "intermediateSigningKey.signatures[0]",
                                        context: Default::default(),
                                    },
                                )?
                            } else {
                                gpay_token.signature.clone()
                            };

                            let googlepay = requests::JpmorganGooglePay {
                                // latLong is required by JPMorgan; use "0,0" when not available
                                lat_long: "0,0".to_string(),
                                encrypted_payment_bundle:
                                    requests::JpmorganEncryptedPaymentBundle {
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
                                merchant_order_number: None,
                                initiator_type: None,
                                account_on_file: None,
                                is_amount_final: None,
                                recurring: None,
                            })
                        }
                        domain_types::payment_method_data::GpayTokenizationData::Decrypted(
                            data,
                        ) => Self::from_decrypted_wallet(
                            router_data,
                            requests::JpmorganCard::from_google_pay(data)?,
                        ),
                    }
                }
                _ => Err(IntegrationError::NotImplemented(
                    "Wallet not supported".to_string(),
                    Default::default(),
                )
                .into()),
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
        let amount_to_capture = item.router_data.request.amount_to_capture.amount;

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

/// VoidPC (post-capture void/reversal) request transformer.
///
/// JPMorgan uses the same `PATCH /payments/{id}` endpoint with `{"isVoid": true}`
/// for both pre-capture void and post-capture reversal. The transaction ID is used
/// to build the URL in the connector implementation.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganVoidPcRequest
{
    type Error = Error;
    fn try_from(
        _item: JpmorganRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self { is_void: true })
    }
}

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map JPMorgan's transaction state directly to `PostCaptureVoidStatus` —
        // the Reverse flow has its own status enum, so we don't go through
        // `AttemptStatus`. `Closed` / `Authorized` mean the PATCH was accepted
        // but the transaction did not move to `Voided`, so the reversal did
        // not apply. `Declined` / `Error` are matched explicitly to keep the
        // match exhaustive.
        let post_capture_void_status = match item.response.transaction_state {
            responses::JpmorganTransactionState::Voided => {
                common_enums::PostCaptureVoidStatus::Succeeded
            }
            responses::JpmorganTransactionState::Pending => {
                common_enums::PostCaptureVoidStatus::Pending
            }
            responses::JpmorganTransactionState::Closed
            | responses::JpmorganTransactionState::Authorized
            | responses::JpmorganTransactionState::Declined
            | responses::JpmorganTransactionState::Error => {
                common_enums::PostCaptureVoidStatus::Failed
            }
        };

        let response = if post_capture_void_status.is_post_capture_void_failure() {
            Err(ErrorResponse {
                attempt_status: None,
                code: item.response.response_code.clone(),
                message: item
                    .response
                    .response_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.response_message.clone(),
                status_code: item.http_code,
                connector_transaction_id: Some(item.response.transaction_id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status,
                connector_reference_id: Some(item.response.transaction_id.clone()),
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
            item.router_data.request.refund_amount.amount,
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
        // Extract networkTransactionId from card.networkResponse for MIT flows
        let network_response = item
            .payment_method_type
            .as_ref()
            .and_then(|pmt| pmt.card.as_ref())
            .and_then(|card| card.network_response.as_ref());

        Ok(Self::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: network_response
                .and_then(|network| network.network_transaction_id.clone()),
            network_txn_link_id: network_response
                .and_then(|network| network.transaction_link_id.clone()),
            connector_response_reference_id: Some(item.request_id.clone()),
            incremental_authorization_allowed: None,
            status_code: item.response_code.parse::<u16>().unwrap_or(0),
            splits: None,
            payment_account_reference: None,
        })
    }
}

impl TryFrom<&responses::JpmorganPaymentsResponse> for AttemptStatus {
    type Error = ResponseError;
    fn try_from(item: &responses::JpmorganPaymentsResponse) -> Result<Self, Self::Error> {
        Ok(match item.response_status {
            responses::JpmorganTransactionStatus::Denied
            | responses::JpmorganTransactionStatus::Error => AttemptStatus::Failure,
            responses::JpmorganTransactionStatus::Success => {
                map_transaction_state_to_attempt_status(
                    &item.transaction_state,
                    &item.capture_method,
                )
            }
        })
    }
}

/// Build the `response` field for a JPMorgan payments flow: `Err(ErrorResponse)`
/// when the transaction was declined/errored, otherwise `Ok(PaymentsResponseData)`.
fn build_payments_response_result(
    response: &responses::JpmorganPaymentsResponse,
    http_code: u16,
    status: AttemptStatus,
) -> Result<Result<PaymentsResponseData, ErrorResponse>, ResponseError> {
    if is_payment_failure(status) {
        Ok(Err(ErrorResponse {
            attempt_status: Some(FlowStatus::Payment(status)),
            code: response.response_code.clone(),
            message: response
                .response_message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.response_message.clone(),
            status_code: http_code,
            connector_transaction_id: Some(response.transaction_id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }))
    } else {
        Ok(Ok(PaymentsResponseData::try_from(response)?))
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
            acquirer_reference_number: None,
        })
    }
}

impl responses::JpmorganVerificationResponse {
    fn status(&self, http_code: u16) -> Result<AttemptStatus, ResponseError> {
        match self.response_status {
            // UCS uses Charged for successful setup; verification does not move funds.
            responses::JpmorganVerificationStatus::Success => {
                if self.payment_method_type.card.is_none() || self.payment_method_type.ach.is_some()
                {
                    return Err(requests::JpmorganSyncResource::response_error(http_code));
                }
                self.verification_authentication_result
                    .as_ref()
                    .map_or(Ok(AttemptStatus::Charged), |result| {
                        result.peek().status(http_code)
                    })
            }
            responses::JpmorganVerificationStatus::Denied
            | responses::JpmorganVerificationStatus::Error => Ok(AttemptStatus::Failure),
            responses::JpmorganVerificationStatus::Unknown => {
                Err(requests::JpmorganSyncResource::response_error(http_code))
            }
        }
    }

    fn into_payment_response(
        self,
        http_code: u16,
        stored_context: Option<requests::JpmorganStoredContext>,
        mut metadata: requests::JpmorganRequestMetadata,
    ) -> Result<Result<PaymentsResponseData, ErrorResponse>, ResponseError> {
        let status = self.status(http_code)?;
        if is_payment_failure(status) {
            return Ok(Err(ErrorResponse {
                code: self.response_code,
                message: self.response_message.clone(),
                reason: Some(self.response_message),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(self.transaction_id.clone()),
                ..Default::default()
            }));
        }
        if let Some(context) = stored_context {
            metadata.jpmorgan.stored_credential = Some(context.stored_credential);
            metadata.jpmorgan.recurring = context.recurring;
            metadata.jpmorgan.wallet = context.wallet;
        }
        metadata.jpmorgan.resource = Some(requests::JpmorganResourceContext {
            resource_type: requests::JpmorganResourceType::Verification,
            id: None,
        });
        let metadata = serde_json::to_value(metadata)
            .change_context(requests::JpmorganSyncResource::response_context(http_code))?;
        let network_response = self
            .payment_method_type
            .card
            .and_then(|card| card.network_response);
        Ok(Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(self.transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: Some(metadata),
            network_txn_id: network_response
                .as_ref()
                .and_then(|network| network.network_transaction_id.clone()),
            network_txn_link_id: network_response.and_then(|network| network.transaction_link_id),
            connector_response_reference_id: Some(self.request_id),
            incremental_authorization_allowed: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
        }))
    }
}

impl responses::JpmorganVerificationAuthenticationResult {
    fn status(&self, http_code: u16) -> Result<AttemptStatus, ResponseError> {
        let invalid = || requests::JpmorganSyncResource::response_error(http_code);
        if self
            .three_domain_secure_server_error_message_text
            .as_ref()
            .is_some_and(|message| !message.peek().trim().is_empty())
            || !self.other.is_empty()
        {
            return Err(invalid());
        }
        if let Some(completion) = &self.three_domain_secure_completion {
            return match completion.peek().transaction_status {
                Some(responses::JpmorganThreeDsTransactionStatus::AuthenticationSuccessful)
                    if self.three_domain_secure_challenge.is_none() =>
                {
                    Ok(AttemptStatus::Charged)
                }
                Some(responses::JpmorganThreeDsTransactionStatus::AuthenticationDenied)
                | Some(responses::JpmorganThreeDsTransactionStatus::AuthenticationRejected) => {
                    Ok(AttemptStatus::AuthenticationFailed)
                }
                Some(responses::JpmorganThreeDsTransactionStatus::ChallengeRequired)
                | Some(responses::JpmorganThreeDsTransactionStatus::DecoupledChallengeRequired) => {
                    Ok(AttemptStatus::AuthenticationPending)
                }
                _ => Err(invalid()),
            };
        }
        if self.authentication_id.is_none()
            && self.authentication_return_url.is_none()
            && self.authentication_orchestration_url.is_none()
            && self.three_domain_secure_challenge.is_none()
            && self.three_domain_secure_exemption.is_none()
            && self.three_domain_secure_server_error_message_text.is_none()
        {
            Ok(AttemptStatus::Charged)
        } else {
            Err(invalid())
        }
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
        let initial_storage =
            requests::JpmorganPaymentsRequest::<T>::has_storage_intent(&item.router_data.request)
                && requests::JpmorganPaymentsRequest::<T>::supports_initial_storage(
                    &item.router_data.request.payment_method_data,
                );
        let status = AttemptStatus::try_from(&item.response)?;
        let mut response = build_payments_response_result(&item.response, item.http_code, status)?;
        if initial_storage {
            if let Ok(PaymentsResponseData::TransactionResponse {
                mandate_reference,
                connector_metadata,
                ..
            }) = &mut response
            {
                let reference = &item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id;
                let previous = requests::JpmorganStoredContext::from_request(
                    &item.router_data.request,
                    reference,
                )
                .change_context(
                    requests::JpmorganSyncResource::response_context(item.http_code),
                )?;
                let recurring = if let Some(context) = &previous {
                    context.recurring.clone()
                } else {
                    requests::JpmorganRecurring::from_initial_context(
                        item.router_data.request.mit_category.as_ref(),
                        item.router_data.request.setup_mandate_details.as_ref(),
                        item.router_data
                            .request
                            .metadata
                            .as_ref()
                            .map(|data| data.peek()),
                    )
                    .change_context(ConnectorError::ResponseHandlingFailed {
                        context: domain_types::errors::ResponseTransformationErrorContext {
                            http_status_code: Some(item.http_code),
                            additional_context: Some(
                                "Cannot preserve the initial recurring agreement from the request."
                                    .to_owned(),
                            ),
                        },
                    })?
                };
                // Return the initialization context for payment-method storage.
                let stored_context = if let Some(previous) = previous {
                    Some(previous)
                } else if matches!(status, AttemptStatus::Authorized | AttemptStatus::Charged) {
                    Some(requests::JpmorganStoredContext {
                        stored_credential: requests::JpmorganStoredCredential {
                            initialization_reference: reference.clone(),
                            source: requests::JpmorganStorageSource::Payment,
                        },
                        recurring,
                        wallet: initial_wallet_context(
                            &item.router_data.request.payment_method_data,
                            item.response
                                .payment_method_type
                                .as_ref()
                                .and_then(|method| method.card.as_ref()),
                        ),
                    })
                } else {
                    None
                };
                *mandate_reference =
                    if matches!(status, AttemptStatus::Authorized | AttemptStatus::Charged) {
                        safetech_mandate(
                            &item.router_data.request.payment_method_data,
                            &item.response.transaction_id,
                            item.response
                                .payment_method_type
                                .as_ref()
                                .and_then(|method| method.card.as_ref()),
                            stored_context
                                .as_ref()
                                .and_then(|context| context.recurring.clone()),
                            item.http_code,
                        )?
                    } else {
                        None
                    };
                *connector_metadata = stored_context
                    .map(|context| {
                        serde_json::to_value(requests::JpmorganStoredFeatureData {
                            jpmorgan: context,
                        })
                    })
                    .transpose()
                    .map_err(|_| requests::JpmorganSyncResource::response_error(item.http_code))?;
            }
        }

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

impl<F> TryFrom<ResponseRouterData<responses::JpmorganTransactionResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganTransactionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (resource, mut metadata) =
            requests::JpmorganSyncResource::from_request_with_context(&item.router_data.request)
                .change_context(requests::JpmorganSyncResource::response_context(
                    item.http_code,
                ))?;
        let (status, response) = match resource {
            requests::JpmorganSyncResource::Payment(_) => {
                let response: responses::JpmorganPaymentsResponse =
                    serde_json::from_value(item.response.0.expose()).change_context(
                        requests::JpmorganSyncResource::response_context(item.http_code),
                    )?;
                let status = AttemptStatus::try_from(&response)?;
                let mut result = build_payments_response_result(&response, item.http_code, status)?;
                if item.router_data.request.connector_feature_data.is_some() {
                    metadata.jpmorgan.resource = None;
                    let preserved = serde_json::to_value(metadata).change_context(
                        requests::JpmorganSyncResource::response_context(item.http_code),
                    )?;
                    if let Ok(PaymentsResponseData::TransactionResponse {
                        mandate_reference,
                        connector_metadata,
                        ..
                    }) = &mut result
                    {
                        // The stored context rides the metadata channel that later requests read back.
                        *mandate_reference = None;
                        *connector_metadata = Some(preserved);
                    }
                }
                (status, result)
            }
            requests::JpmorganSyncResource::Verification(id) => {
                let response: responses::JpmorganVerificationResponse =
                    serde_json::from_value(item.response.0.expose()).change_context(
                        requests::JpmorganSyncResource::response_context(item.http_code),
                    )?;
                if id != response.transaction_id {
                    return Err(requests::JpmorganSyncResource::response_error(
                        item.http_code,
                    ));
                }
                let status = response.status(item.http_code)?;
                (
                    status,
                    response.into_payment_response(item.http_code, None, metadata)?,
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

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response = build_payments_response_result(&item.response, item.http_code, status)?;

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

impl<F> TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::try_from(&item.response)?;
        let response = build_payments_response_result(&item.response, item.http_code, status)?;

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
                MerchantAuthenticationFlowData,
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
                MerchantAuthenticationFlowData,
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
        MerchantAuthenticationFlowData,
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

impl TryFrom<&JpmorganAuthType> for requests::JpmorganMerchant {
    type Error = Error;
    fn try_from(auth: &JpmorganAuthType) -> Result<Self, Self::Error> {
        Ok(Self {
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
            soft_merchant: auth.merchant_purchase_description.clone().map(|d| {
                requests::JpmorganSoftMerchant {
                    merchant_purchase_description: d,
                }
            }),
        })
    }
}

// Parse a network token expiry month to its numeric value.
fn get_network_token_expiry_month_i32(
    expiry_month: &Secret<String>,
    context: IntegrationError,
) -> Result<i32, Error> {
    expiry_month.peek().parse::<i32>().change_context(context)
}
// SetupMandate (initial CIT with credential storage) request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganSetupMandateRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
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

        let request = &router_data.request;
        let context = IntegrationErrorContext {
            suggested_action: Some(
                "Use zero-value, cardholder-initiated setup with a card or a decrypted wallet. Use Authorize for a payment."
                    .to_owned(),
            ),
            ..Default::default()
        };
        let amount = request
            .amount
            .as_ref()
            .map(|money| money.amount)
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: context.clone(),
            })?;
        if amount.get_amount_as_i64() != 0 {
            return Err(IntegrationError::NotSupported {
                message: "Nonzero setup amount".to_owned(),
                connector: "jpmorgan",
                context,
            }
            .into());
        }
        map_capture_method(request.capture_method)?;
        requests::JpmorganCard::validate_apple_pay_storage(&request.payment_method_data)?;
        let recurring = requests::JpmorganRecurring::from_initial_context(
            request.mit_category.as_ref(),
            request.setup_mandate_details.as_ref(),
            router_data
                .request
                .metadata
                .as_ref()
                .map(|data| data.peek()),
        )?;
        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => requests::JpmorganSetupMandateCard {
                card: requests::JpmorganCard {
                    account_number: card.card_number.clone(),
                    expiry: requests::Expiry {
                        month: Secret::new(i32::from(card.get_expiry_month_as_u8()?)),
                        year: Secret::new(i32::from(card.get_expiry_year_4_digit_as_u16()?)),
                    },
                    account_number_type: Some(requests::JpmorganAccountNumberType::Pan),
                    wallet_provider: None,
                    authentication: None,
                },
                cvv: if card.card_cvc.peek().is_empty() {
                    None
                } else {
                    Some(card.card_cvc.clone())
                },
            },
            PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) => match &wallet.payment_data {
                ApplePayPaymentData::Decrypted(data) => requests::JpmorganSetupMandateCard {
                    card: requests::JpmorganCard::from_apple_pay(data)?,
                    cvv: None,
                },
                ApplePayPaymentData::Encrypted(_) => {
                    return Err(IntegrationError::NotSupported {
                        message: "Encrypted Apple Pay verification".to_owned(),
                        connector: "jpmorgan",
                        context,
                    }
                    .into())
                }
            },
            PaymentMethodData::Wallet(WalletData::GooglePay(wallet)) => {
                match &wallet.tokenization_data {
                    domain_types::payment_method_data::GpayTokenizationData::Decrypted(data) => {
                        requests::JpmorganSetupMandateCard {
                            card: requests::JpmorganCard::from_google_pay(data)?,
                            cvv: None,
                        }
                    }
                    domain_types::payment_method_data::GpayTokenizationData::Encrypted(_) => {
                        return Err(IntegrationError::NotSupported {
                            message: "Encrypted Google Pay verification".to_owned(),
                            connector: "jpmorgan",
                            context,
                        }
                        .into())
                    }
                }
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Verification for this payment method".to_owned(),
                    connector: "jpmorgan",
                    context,
                }
                .into())
            }
        };
        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            currency: request.currency,
            merchant: requests::JpmorganMerchant::try_from(&auth)?,
            payment_method_type: requests::JpmorganSetupMandatePaymentMethodType { card },
            recurring_sequence: recurring.map(|details| details.recurring_sequence),
            merchant_order_number: Some(merchant_order_number(
                request.merchant_order_id.as_deref(),
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )),
            initiator_type: requests::JpmorganInitiatorType::Cardholder,
            account_on_file: requests::JpmorganAccountOnFile::ToBeStored,
        })
    }
}

// SetupMandate response transformer
impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<responses::JpmorganSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let verification = item.response;
        if verification.currency != item.router_data.request.currency {
            return Err(requests::JpmorganSyncResource::response_error(
                item.http_code,
            ));
        }
        let status = verification.status(item.http_code)?;
        let metadata = requests::JpmorganRequestMetadata::default();
        let mut mandate_metadata = None;
        if status == AttemptStatus::Charged {
            let recurring = requests::JpmorganRecurring::from_initial_context(
                item.router_data.request.mit_category.as_ref(),
                item.router_data.request.setup_mandate_details.as_ref(),
                item.router_data
                    .request
                    .metadata
                    .as_ref()
                    .map(|data| data.peek()),
            )
            .change_context(requests::JpmorganSyncResource::response_context(
                item.http_code,
            ))?;
            mandate_metadata = Some(requests::JpmorganStoredContext {
                stored_credential: requests::JpmorganStoredCredential {
                    initialization_reference: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    source: requests::JpmorganStorageSource::Verification,
                },
                recurring,
                wallet: initial_wallet_context(
                    &item.router_data.request.payment_method_data,
                    verification.payment_method_type.card.as_ref(),
                ),
            });
        }
        let safetech = if status == AttemptStatus::Charged {
            safetech_mandate(
                &item.router_data.request.payment_method_data,
                &verification.transaction_id,
                verification.payment_method_type.card.as_ref(),
                mandate_metadata
                    .as_ref()
                    .and_then(|context| context.recurring.clone()),
                item.http_code,
            )?
        } else {
            None
        };
        let mut response =
            verification.into_payment_response(item.http_code, mandate_metadata, metadata)?;
        if let Ok(PaymentsResponseData::TransactionResponse {
            mandate_reference, ..
        }) = &mut response
        {
            *mandate_reference = safetech;
        }

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

impl<T: PaymentMethodDataTypes> requests::JpmorganRepeatPaymentRequest<T> {
    fn mit_context() -> IntegrationErrorContext {
        IntegrationErrorContext {
            suggested_action: Some(
                "Supply the original network transaction ID, the same card or decrypted wallet credential, and an explicit mit_category."
                    .to_owned(),
            ),
            ..Default::default()
        }
    }

    fn unsupported(message: &str) -> Error {
        IntegrationError::NotSupported {
            message: message.to_owned(),
            connector: "jpmorgan",
            context: Self::mit_context(),
        }
        .into()
    }

    fn invalid_agreement_context() -> Error {
        IntegrationError::InvalidDataFormat {
            field_name: "metadata.jpmorgan.recurring",
            context: Self::mit_context(),
        }
        .into()
    }

    fn validate_mit_context(request: &RepeatPaymentData<T>) -> Result<(), Error> {
        if let Some(data) = &request.metadata {
            serde_json::from_value::<requests::JpmorganRequestMetadata>(data.peek().clone())
                .map_err(|_| Self::invalid_agreement_context())?;
        }
        if request.off_session == Some(false) || request.authentication_data.is_some() {
            return Err(Self::unsupported(
                "Off-session false or separately authenticated merchant-initiated payments",
            ));
        }
        Ok(())
    }

    fn safetech_credential(
        reference: &MandateReferenceId,
    ) -> Result<Option<requests::JpmorganSafetechCredential>, Error> {
        let MandateReferenceId::ConnectorMandateId(reference) = reference else {
            return Ok(None);
        };
        let metadata = reference.get_mandate_metadata().ok_or_else(|| {
            Self::unsupported("PSP token charges without Safetech credential metadata")
        })?;
        let credential: requests::JpmorganSafetechCredential =
            serde_json::from_value(metadata.peek().clone())
                .map_err(|_| Self::unsupported("Invalid Safetech credential metadata"))?;
        if credential.token.peek().trim().is_empty()
            || credential.handle.trim().is_empty()
            || reference.get_connector_mandate_id().as_ref() != Some(&credential.handle)
            || !(1..=12).contains(credential.expiry.month.peek())
            || !(1000..=9999).contains(credential.expiry.year.peek())
            || credential.original_network_transaction_id.trim().is_empty()
            || credential
                .original_transaction_link_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
            || (credential.network == common_enums::CardNetwork::Mastercard
                && credential.original_transaction_link_id.is_none())
        {
            return Err(Self::unsupported(
                "Incomplete or conflicting Safetech credential metadata",
            ));
        }
        Ok(Some(credential))
    }

    fn network_references(
        reference: &MandateReferenceId,
    ) -> Result<(String, Option<String>), Error> {
        let references: Result<(String, Option<String>), Error> = match reference {
            MandateReferenceId::NetworkMandateId(network) => Ok((
                network.network_transaction_id.clone(),
                network.transaction_link_id.clone(),
            )),
            MandateReferenceId::ConnectorMandateId(_) => {
                let credential = Self::safetech_credential(reference)?
                    .ok_or_else(|| Self::unsupported("Missing Safetech credential"))?;
                Ok((
                    credential.original_network_transaction_id,
                    credential.original_transaction_link_id,
                ))
            }
            MandateReferenceId::NetworkTokenWithNTI(network) => Ok((
                network.network_transaction_id.clone(),
                network.transaction_link_id.clone(),
            )),
        };
        let references = references?;
        if references.0.trim().is_empty()
            || references
                .1
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "mandate_reference.network_transaction_id or transaction_link_id",
                context: Self::mit_context(),
            }
            .into());
        }
        Ok(references)
    }

    fn mit_recurring(
        request: &RepeatPaymentData<T>,
    ) -> Result<Option<requests::JpmorganRecurring>, Error> {
        match request.mit_category {
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "mit_category",
                context: Self::mit_context(),
            }
            .into()),
            Some(common_enums::MitCategory::Unscheduled) => Ok(None),
            Some(
                common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription,
            ) => {
                let recurring = request
                    .metadata
                    .as_ref()
                    .map(|data| {
                        serde_json::from_value::<requests::JpmorganRequestMetadata>(
                            data.peek().clone(),
                        )
                    })
                    .transpose()
                    .map_err(|_| Self::invalid_agreement_context())?
                    .and_then(|feature| feature.jpmorgan.recurring);
                let credential = Self::safetech_credential(&request.mandate_reference)?;
                let stored = credential.and_then(|credential| credential.recurring);
                if recurring
                    .as_ref()
                    .zip(stored.as_ref())
                    .is_some_and(|(feature, stored)| feature != stored)
                {
                    return Err(Self::invalid_agreement_context());
                }
                let recurring =
                    recurring
                        .or(stored)
                        .ok_or_else(|| IntegrationError::MissingRequiredField {
                            field_name: "metadata.jpmorgan.recurring",
                            context: Self::mit_context(),
                        })?;
                if recurring.recurring_sequence != requests::JpmorganRecurringSequence::First
                    || recurring.agreement_id.trim().is_empty()
                    || recurring.agreement_id.chars().count() > JPMORGAN_AGREEMENT_ID_MAX_LENGTH
                    || recurring.is_variable_amount.is_none()
                {
                    return Err(Self::invalid_agreement_context());
                }
                Ok(Some(requests::JpmorganRecurring {
                    recurring_sequence: requests::JpmorganRecurringSequence::Subsequent,
                    agreement_id: recurring.agreement_id,
                    is_variable_amount: recurring.is_variable_amount,
                }))
            }
            Some(
                common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission,
            ) => Err(Self::unsupported(
                "Installment or resubmission merchant-initiated payments",
            )),
        }
    }

    fn stored_wallet(
        request: &RepeatPaymentData<T>,
        provider: &requests::JpmorganWalletProvider,
    ) -> Result<requests::JpmorganStoredWallet, Error> {
        let wallet = request
            .metadata
            .as_ref()
            .and_then(|data| {
                serde_json::from_value::<requests::JpmorganRequestMetadata>(data.peek().clone())
                    .ok()
            })
            .and_then(|metadata| {
                metadata.jpmorgan.wallet.or_else(|| {
                    metadata
                        .jpmorgan
                        .wallet_account_number_type
                        .map(|account_number_type| requests::JpmorganStoredWallet {
                            provider: provider.clone(),
                            account_number_type,
                            network: None,
                        })
                })
            })
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "metadata.jpmorgan.wallet_account_number_type",
                context: Self::mit_context(),
            })?;
        if &wallet.provider != provider
            || !matches!(
                wallet.account_number_type,
                requests::JpmorganAccountNumberType::Pan
                    | requests::JpmorganAccountNumberType::DeviceToken
            )
            || (wallet.provider == requests::JpmorganWalletProvider::ApplePay
                && wallet.account_number_type != requests::JpmorganAccountNumberType::DeviceToken)
        {
            return Err(Self::unsupported(
                "Wallet credentials with conflicting or undocumented account-number types",
            ));
        }
        Ok(wallet)
    }

    fn mit_payment_method(
        request: &RepeatPaymentData<T>,
        original_network_transaction_id: String,
        original_transaction_link_id: Option<String>,
    ) -> Result<requests::JpmorganMitCardByNti<T>, Error> {
        if let Some(credential) = Self::safetech_credential(&request.mandate_reference)? {
            return Ok(requests::JpmorganMitCardByNti {
                account_number: requests::JpmorganMitAccountNumber::Safetech(credential.token),
                expiry: credential.expiry,
                account_number_type: Some(requests::JpmorganAccountNumberType::SafetechToken),
                wallet_provider: None,
                original_network_transaction_id,
                original_transaction_link_id,
                authentication: None,
            });
        }
        let (account_number, expiry, account_number_type, wallet_provider) = match &request
            .payment_method_data
        {
            PaymentMethodData::Card(card) => (
                card.card_number.clone(),
                requests::Expiry {
                    month: Secret::new(i32::from(card.get_expiry_month_as_u8()?)),
                    year: Secret::new(i32::from(card.get_expiry_year_4_digit_as_u16()?)),
                },
                Some(requests::JpmorganAccountNumberType::Pan),
                None,
            ),
            PaymentMethodData::CardWithNoCvc(card) => (
                RawCardNumber(T::inner_from_card_number(card.card_number.clone())),
                requests::Expiry {
                    month: Secret::new(
                        card.get_card_expiry_month_2_digit()?
                            .peek()
                            .parse::<i32>()
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: Self::mit_context(),
                            })?,
                    ),
                    year: Secret::new(
                        card.get_expiry_year_4_digit()
                            .peek()
                            .parse::<i32>()
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: Self::mit_context(),
                            })?,
                    ),
                },
                Some(requests::JpmorganAccountNumberType::Pan),
                None,
            ),
            PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
                card.card_number.clone(),
                requests::Expiry {
                    month: Secret::new(i32::from(*card.get_expiry_month_as_i8()?.peek())),
                    year: Secret::new(
                        card.get_expiry_year_4_digit()
                            .peek()
                            .parse::<i32>()
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: Self::mit_context(),
                            })?,
                    ),
                },
                Some(requests::JpmorganAccountNumberType::Pan),
                None,
            ),
            PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(token) => {
                let wallet_provider = match token.token_source {
                    Some(domain_types::payment_method_data::TokenSource::ApplePay) => {
                        requests::JpmorganWalletProvider::ApplePay
                    }
                    Some(domain_types::payment_method_data::TokenSource::GooglePay) => {
                        requests::JpmorganWalletProvider::GooglePay
                    }
                    None => {
                        return Err(IntegrationError::MissingRequiredField {
                            field_name: "payment_method_data.decrypted_wallet_token.token_source",
                            context: Self::mit_context(),
                        }
                        .into())
                    }
                };
                (
                    RawCardNumber(T::inner_from_card_number(
                        token
                            .decrypted_token
                            .get_card_no()
                            .parse::<cards::CardNumber>()
                            .map_err(|_| IntegrationError::InvalidDataFormat {
                                field_name:
                                    "payment_method_data.decrypted_wallet_token.decrypted_token",
                                context: Self::mit_context(),
                            })?,
                    )),
                    requests::Expiry {
                        month: Secret::new(i32::from(*token.get_expiry_month_as_i8()?.peek())),
                        year: Secret::new(
                            token
                                .get_expiry_year_4_digit()
                                .peek()
                                .parse::<i32>()
                                .change_context(IntegrationError::RequestEncodingFailed {
                                    context: Self::mit_context(),
                                })?,
                        ),
                    },
                    Some(Self::stored_wallet(request, &wallet_provider)?.account_number_type),
                    Some(wallet_provider),
                )
            }
            PaymentMethodData::Wallet(WalletData::ApplePay(wallet)) => match &wallet.payment_data {
                ApplePayPaymentData::Decrypted(data) => {
                    if data.merchant_token_identifier.is_some() {
                        return Err(Self::unsupported("Apple Pay merchant tokens without a documented payment account-number type"));
                    }
                    let account_number_type =
                        Self::stored_wallet(request, &requests::JpmorganWalletProvider::ApplePay)?
                            .account_number_type;
                    (
                        RawCardNumber(T::inner_from_card_number(
                            data.application_primary_account_number.clone(),
                        )),
                        requests::Expiry {
                            month: Secret::new(
                                data.get_expiry_month()
                                    .peek()
                                    .parse::<i32>()
                                    .change_context(IntegrationError::RequestEncodingFailed {
                                        context: Self::mit_context(),
                                    })?,
                            ),
                            year: Secret::new(
                                data.get_four_digit_expiry_year()
                                    .peek()
                                    .parse::<i32>()
                                    .change_context(IntegrationError::RequestEncodingFailed {
                                        context: Self::mit_context(),
                                    })?,
                            ),
                        },
                        Some(account_number_type),
                        Some(requests::JpmorganWalletProvider::ApplePay),
                    )
                }
                ApplePayPaymentData::Encrypted(_) => {
                    return Err(Self::unsupported("Encrypted Apple Pay charges"))
                }
            },
            PaymentMethodData::Wallet(WalletData::GooglePay(wallet)) => {
                match &wallet.tokenization_data {
                    domain_types::payment_method_data::GpayTokenizationData::Decrypted(data) => {
                        let account_number_type = Self::stored_wallet(
                            request,
                            &requests::JpmorganWalletProvider::GooglePay,
                        )?
                        .account_number_type;
                        (
                            RawCardNumber(T::inner_from_card_number(
                                data.application_primary_account_number.clone(),
                            )),
                            requests::Expiry {
                                month: Secret::new(
                                    data.get_expiry_month()
                                        .change_context(IntegrationError::InvalidDataFormat {
                                            field_name: "google_pay.card_exp_month",
                                            context: Self::mit_context(),
                                        })?
                                        .peek()
                                        .parse::<i32>()
                                        .change_context(
                                            IntegrationError::RequestEncodingFailed {
                                                context: Self::mit_context(),
                                            },
                                        )?,
                                ),
                                year: Secret::new(
                                    data.get_four_digit_expiry_year()
                                        .change_context(IntegrationError::InvalidDataFormat {
                                            field_name: "google_pay.card_exp_year",
                                            context: Self::mit_context(),
                                        })?
                                        .peek()
                                        .parse::<i32>()
                                        .change_context(
                                            IntegrationError::RequestEncodingFailed {
                                                context: Self::mit_context(),
                                            },
                                        )?,
                                ),
                            },
                            Some(account_number_type),
                            Some(requests::JpmorganWalletProvider::GooglePay),
                        )
                    }
                    domain_types::payment_method_data::GpayTokenizationData::Encrypted(_) => {
                        return Err(Self::unsupported("Encrypted Google Pay charges"))
                    }
                }
            }
            PaymentMethodData::NetworkToken(token) => (
                RawCardNumber(T::inner_from_card_number(
                    token
                        .token_number
                        .get_card_no()
                        .parse::<cards::CardNumber>()
                        .map_err(|_| IntegrationError::InvalidDataFormat {
                            field_name: "payment_method_data.network_token.token_number",
                            context: Self::mit_context(),
                        })?,
                )),
                requests::Expiry {
                    month: Secret::new(get_network_token_expiry_month_i32(
                        &token.get_network_token_expiry_month(),
                        IntegrationError::RequestEncodingFailed {
                            context: Self::mit_context(),
                        },
                    )?),
                    year: Secret::new(
                        token
                            .get_expiry_year_4_digit()
                            .peek()
                            .parse::<i32>()
                            .change_context(IntegrationError::RequestEncodingFailed {
                                context: Self::mit_context(),
                            })?,
                    ),
                },
                Some(requests::JpmorganAccountNumberType::NetworkToken),
                None,
            ),
            _ => {
                return Err(Self::unsupported(
                    "Charges without a card or decrypted wallet credential",
                ))
            }
        };
        let supplied_network = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card.card_network.clone(),
            PaymentMethodData::CardWithNoCvc(card) => card.card_network.clone(),
            PaymentMethodData::CardDetailsForNetworkTransactionId(card) => {
                card.card_network.clone()
            }
            PaymentMethodData::NetworkToken(token) => token.card_network.clone(),
            PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(token) => {
                token.card_network.clone()
            }
            _ => None,
        };
        let established_network = if let Some(provider) = &wallet_provider {
            Self::stored_wallet(request, provider)?.network
        } else if account_number_type == Some(requests::JpmorganAccountNumberType::Pan) {
            pan_card_network(T::peek_inner(&account_number.0))
        } else {
            None
        };
        if supplied_network
            .as_ref()
            .zip(established_network.as_ref())
            .is_some_and(|(supplied, established)| supplied != established)
        {
            return Err(Self::unsupported(
                "Conflicting card networks on a merchant-initiated payment",
            ));
        }
        let network = supplied_network.or(established_network).ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "payment_method_data.card_network or preserved wallet network",
                context: Self::mit_context(),
            }
        })?;
        if network == common_enums::CardNetwork::Mastercard
            && original_transaction_link_id.is_none()
        {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "mandate_reference.transaction_link_id",
                context: Self::mit_context(),
            }
            .into());
        }
        Ok(requests::JpmorganMitCardByNti {
            account_number: requests::JpmorganMitAccountNumber::Card(account_number),
            expiry,
            account_number_type,
            wallet_provider,
            original_network_transaction_id,
            original_transaction_link_id,
            authentication: match &request.payment_method_data {
                PaymentMethodData::NetworkToken(token) => token
                    .token_cryptogram
                    .as_ref()
                    .map(|cryptogram| {
                        requests::JpmorganCard::<T>::network_token_authentication(token, cryptogram)
                    })
                    .transpose()?,
                _ => None,
            },
        })
    }
}

// RepeatPayment (subsequent MIT) request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        JpmorganRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::JpmorganRepeatPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        item: JpmorganRouterData<
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

        let auth = JpmorganAuthType::try_from(&router_data.connector_config)?;
        let merchant = requests::JpmorganMerchant::try_from(&auth)?;

        Self::validate_mit_context(&router_data.request)?;
        let (original_network_transaction_id, original_transaction_link_id) =
            Self::network_references(&router_data.request.mandate_reference)?;
        let capture_method = map_capture_method(router_data.request.capture_method)?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.amount.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let payment_method_type = requests::JpmorganRepeatPaymentMethodType {
            card: Self::mit_payment_method(
                &router_data.request,
                original_network_transaction_id,
                original_transaction_link_id,
            )?,
        };

        Ok(Self {
            capture_method,
            amount,
            currency: router_data.request.currency,
            merchant,
            payment_method_type,
            recurring: Self::mit_recurring(&router_data.request)?,
            merchant_order_number: router_data.request.merchant_order_id.clone(),
            initiator_type: requests::JpmorganInitiatorType::Merchant,
            account_on_file: requests::JpmorganAccountOnFile::Stored,
            is_amount_final: true,
        })
    }
}

// RepeatPayment response transformer
impl<T: PaymentMethodDataTypes>
    TryFrom<ResponseRouterData<responses::JpmorganPaymentsResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = ResponseError;
    fn try_from(
        item: ResponseRouterData<responses::JpmorganPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.response_status {
            responses::JpmorganTransactionStatus::Success => {
                AttemptStatus::try_from(&item.response)?
            }
            responses::JpmorganTransactionStatus::Denied
            | responses::JpmorganTransactionStatus::Error => AttemptStatus::Failure,
        };
        let mut response = build_payments_response_result(&item.response, item.http_code, status)?;
        if let Ok(PaymentsResponseData::TransactionResponse {
            mandate_reference,
            connector_metadata,
            ..
        }) = &mut response
        {
            *mandate_reference = None;
            if matches!(status, AttemptStatus::Authorized | AttemptStatus::Charged) {
                if let MandateReferenceId::ConnectorMandateId(reference) =
                    &item.router_data.request.mandate_reference
                {
                    *mandate_reference = Some(Box::new(MandateReference {
                        connector_mandate_id: reference.get_connector_mandate_id(),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: reference.get_mandate_metadata(),
                    }));
                }
            }
            *connector_metadata = None;
        }

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

impl requests::JpmorganStoredContext {
    pub(super) fn invalid() -> Error {
        IntegrationError::InvalidDataFormat {
            field_name: "metadata.jpmorgan",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Return successful storage context unchanged for another checkout with the same credential and agreement.".to_owned(),
                ),
                ..Default::default()
            },
        }
        .into()
    }

    pub(super) fn from_value(value: &serde_json::Value) -> Result<Self, Error> {
        let context: Self = serde_json::from_value(value.clone()).map_err(|_| Self::invalid())?;
        context.validate()
    }

    fn validate(self) -> Result<Self, Error> {
        if self
            .stored_credential
            .initialization_reference
            .trim()
            .is_empty()
        {
            return Err(Self::invalid());
        }
        if let Some(recurring) = &self.recurring {
            if recurring.agreement_id.trim().is_empty()
                || recurring.agreement_id.chars().count() > JPMORGAN_AGREEMENT_ID_MAX_LENGTH
                || recurring.is_variable_amount.is_none()
                || recurring.recurring_sequence != requests::JpmorganRecurringSequence::First
            {
                return Err(Self::invalid());
            }
        }
        Ok(self)
    }

    pub(super) fn from_request<T: PaymentMethodDataTypes>(
        request: &PaymentsAuthorizeData<T>,
        reference: &str,
    ) -> Result<Option<Self>, Error> {
        if request
            .metadata
            .as_ref()
            .is_some_and(|data| !data.peek().is_object())
        {
            return Err(Self::invalid());
        }
        let Some(value) = request
            .metadata
            .as_ref()
            .and_then(|data| data.peek().get("jpmorgan"))
        else {
            return Ok(None);
        };
        let supplied: requests::JpmorganPreservedContext =
            serde_json::from_value(value.clone()).map_err(|_| Self::invalid())?;
        if supplied.stored_credential.is_none() {
            requests::JpmorganRecurring::from_initial_context(
                request.mit_category.as_ref(),
                request.setup_mandate_details.as_ref(),
                request.metadata.as_ref().map(|data| data.peek()),
            )?;
            return Ok(None);
        }
        let context = Self::from_value(value)?;
        if context.stored_credential.initialization_reference == reference {
            return Err(Self::invalid());
        }
        match request.mit_category {
            Some(
                common_enums::MitCategory::Recurring | common_enums::MitCategory::Subscription,
            ) if context.recurring.is_none() => return Err(Self::invalid()),
            Some(common_enums::MitCategory::Unscheduled) if context.recurring.is_some() => {
                return Err(Self::invalid())
            }
            Some(
                common_enums::MitCategory::Installment | common_enums::MitCategory::Resubmission,
            ) => return Err(Self::invalid()),
            _ => (),
        }
        if let Some(terms) = request.setup_mandate_details.as_ref() {
            if terms.update_mandate_id.is_some() {
                return Err(Self::invalid());
            }
            requests::JpmorganRecurring::validate_supplied_terms(
                context.recurring.as_ref(),
                Some(terms),
            )?;
        }
        Ok(Some(context))
    }
}

impl requests::JpmorganSyncResource {
    pub(super) fn from_request(request: &PaymentsSyncData) -> Result<Self, Error> {
        Self::from_request_with_context(request).map(|(resource, _)| resource)
    }

    fn from_request_with_context(
        request: &PaymentsSyncData,
    ) -> Result<(Self, requests::JpmorganRequestMetadata), Error> {
        let invalid_context = || IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.jpmorgan.resource",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Return the verification resource type with its verification transaction ID."
                        .to_owned(),
                ),
                ..Default::default()
            },
        };
        let metadata: requests::JpmorganRequestMetadata = request
            .connector_feature_data
            .as_ref()
            .map(|data| serde_json::from_value(data.peek().clone()))
            .transpose()
            .change_context(invalid_context())?
            .unwrap_or_default();
        if let Some(resource) = &metadata.jpmorgan.resource {
            let id = match &request.connector_transaction_id {
                ResponseId::ConnectorTransactionId(id) => {
                    if resource.id.as_ref().is_some_and(|legacy| legacy != id) {
                        return Err(invalid_context().into());
                    }
                    id.clone()
                }
                ResponseId::NoResponseId => resource.id.clone().ok_or_else(invalid_context)?,
                _ => return Err(invalid_context().into()),
            };
            if !(JPMORGAN_TRANSACTION_ID_MIN_LENGTH..=JPMORGAN_TRANSACTION_ID_MAX_LENGTH)
                .contains(&id.len())
                || id.trim() != id
            {
                return Err(invalid_context().into());
            }
            return Ok((Self::Verification(id), metadata));
        }
        if let Some(credential) = &metadata.jpmorgan.stored_credential {
            requests::JpmorganStoredContext {
                stored_credential: credential.clone(),
                recurring: metadata.jpmorgan.recurring.clone(),
                wallet: metadata.jpmorgan.wallet.clone(),
            }
            .validate()?;
        }
        let id = request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Supply the payment ID or the original verification resource context."
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            })?;
        Ok((Self::Payment(id), metadata))
    }

    pub(super) fn url(&self, base: &str) -> String {
        match self {
            Self::Payment(id) => format!("{base}/payments/{}", urlencoding::encode(id)),
            Self::Verification(id) => format!("{base}/verifications/{}", urlencoding::encode(id)),
        }
    }

    pub(super) fn response_context(http_code: u16) -> ConnectorError {
        ConnectorError::ResponseHandlingFailed {
            context: domain_types::errors::ResponseTransformationErrorContext {
                http_status_code: Some(http_code),
                additional_context: Some(
                    "The response does not match the requested JPMorgan resource.".to_owned(),
                ),
            },
        }
    }

    pub(super) fn response_error(http_code: u16) -> ResponseError {
        Self::response_context(http_code).into()
    }
}

impl std::str::FromStr for requests::JpmorganAmountType {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "exact" => Ok(Self::Exact),
            "variable" => Ok(Self::Variable),
            _ => Err(()),
        }
    }
}
