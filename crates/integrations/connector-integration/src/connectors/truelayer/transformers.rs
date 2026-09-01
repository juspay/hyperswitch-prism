use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use common_enums::{self, AttemptStatus, CountryAlpha2, Currency};
use common_utils::{consts, pii, request::Method, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, RSync, Refund, ServerAuthenticationToken, VerifyWebhookSource, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsResponseData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        VerifyWebhookSourceFlowData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        BankRedirectData, DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::RedirectForm,
    router_response_types::{VerifyWebhookSourceResponseData, VerifyWebhookStatus},
    utils::is_payment_failure,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use openssl::{
    bn::{BigNum, BigNumContext},
    ec::{EcGroup, EcKey, EcPoint},
    ecdsa::EcdsaSig,
    hash::{hash, MessageDigest},
    nid::Nid,
    pkey::Public,
};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::collections::HashMap;

use crate::{connectors::truelayer::TruelayerRouterData, types::ResponseRouterData, utils};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};
const GRANT_TYPE: &str = "client_credentials";
const SCOPE: &str = "payments";
const SIG_BYTES_EXPECTED_LENGTH: usize = 132;
const P521_COORDINATE_BYTE_LEN: usize = 66;
const PREFIX: &str = "/api";
const SCHEME_SELECTION_TYPE: &str = "instant_preferred";
const PAYMENT_METHOD_TYPE: &str = "bank_transfer";
const BENEFICIARY_TYPE: &str = "merchant_account";

pub struct TruelayerAuthType {
    pub(super) client_id: Secret<String>,
    pub(super) client_secret: Secret<String>,
    pub(super) merchant_account_id: Option<Secret<String>>,
    pub(super) account_holder_name: Option<Secret<String>>,
    pub(super) private_key: Option<Secret<String>>,
    pub(super) kid: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TruelayerServerAuthenticationTokenRequestData {
    grant_type: String,
    client_id: Secret<String>,
    client_secret: Secret<String>,
    scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerAccessTokenErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
    pub error_details: Option<ErrorDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerErrorResponse {
    #[serde(rename = "type")]
    pub _type: String,
    pub title: String,
    pub status: i32,
    pub trace_id: String,
    pub detail: String,
    pub errors: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorDetails {
    pub reason: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TruelayerAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Truelayer {
                client_id,
                client_secret,
                merchant_account_id,
                account_holder_name,
                private_key,
                kid,
                ..
            } => Ok(Self {
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
                merchant_account_id: merchant_account_id.clone(),
                account_holder_name: account_holder_name.clone(),
                private_key: private_key.clone(),
                kid: kid.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TruelayerRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for TruelayerServerAuthenticationTokenRequestData
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: TruelayerRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TruelayerAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            grant_type: GRANT_TYPE.to_string(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
            scope: SCOPE.to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerServerAuthenticationTokenResponseData {
    access_token: Secret<String>,
    expires_in: i64,
    token_type: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<TruelayerServerAuthenticationTokenResponseData, Self>>
    for RouterDataV2<F, MerchantAuthenticationFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<TruelayerServerAuthenticationTokenResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in),
                token_type: item.response.token_type,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruelayerMetadata {
    merchant_account_id: Secret<String>,
    account_holder_name: Secret<String>,
    pub private_key: Secret<String>,
    pub kid: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TruelayerMetadata {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        let auth = TruelayerAuthType::try_from(auth_type)?;
        Self::try_from(&auth)
    }
}

impl TryFrom<&TruelayerAuthType> for TruelayerMetadata {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth: &TruelayerAuthType) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_account_id: auth.merchant_account_id.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "merchant_account_id",
                    context: Default::default(),
                },
            )?,
            account_holder_name: auth.account_holder_name.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "account_holder_name",
                    context: Default::default(),
                },
            )?,
            private_key: auth.private_key.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "private_key",
                    context: Default::default(),
                },
            )?,
            kid: auth
                .kid
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "kid",
                    context: Default::default(),
                })?,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TruelayerPaymentsRequestData {
    amount_in_minor: MinorUnit,
    currency: Currency,
    hosted_page: HostedPage,
    payment_method: PaymentMethod,
    user: User,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct HostedPage {
    return_uri: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct PaymentMethod {
    #[serde(rename = "type")]
    _type: String,
    provider_selection: ProviderSelection,
    beneficiary: Beneficiary,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, PartialEq)]
struct ProviderSelection {
    #[serde(rename = "type")]
    _type: ProviderSelectionType,
    provider_id: Option<String>,
    remitter: Option<Remitter>,
    scheme_selection: Option<SchemeSelection>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct SchemeSelection {
    #[serde(rename = "type")]
    _type: String,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Remitter {
    account_identifier: TruelayerAccountIdentifier,
    account_holder_name: Option<Secret<String>>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerAccountIdentifier {
    #[serde(rename = "type")]
    identifier_type: TruelayerAccountIdentifierType,
    sort_code: Option<Secret<String>>,
    account_number: Option<Secret<String>>,
    iban: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ProviderSelectionType {
    UserSelected,
    Preselected,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, PartialEq)]
struct Beneficiary {
    #[serde(rename = "type")]
    _type: String,
    merchant_account_id: Secret<String>,
    account_holder_name: Secret<String>,
    reference: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: Secret<String>,
    email: Option<pii::Email>,
    phone: Option<Secret<String>>,
    address: Option<Address>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct Address {
    address_line1: Secret<String>,
    address_line2: Option<Secret<String>>,
    city: Secret<String>,
    state: Secret<String>,
    zip: Option<Secret<String>>,
    country_code: CountryAlpha2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPaymentsResponseData {
    id: String,
    user: UserIdResponse,
    resource_token: Option<Secret<String>>,
    status: TruelayerPaymentStatus,
    hosted_page: Option<HostedPageResponse>,
    failure_reason: Option<String>,
    failure_stage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct UserIdResponse {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TruelayerPaymentStatus {
    AuthorizationRequired,
    Settled,
    Failed,
    Authorized,
    Authorizing,
    AttemptFailed,
    Executed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct HostedPageResponse {
    uri: String,
}

fn normalize_connector_request_reference_id(reference_id: &str) -> String {
    reference_id
        .chars()
        .map(|c| if c == '_' { '-' } else { c })
        .collect()
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TruelayerRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TruelayerPaymentsRequestData
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: TruelayerRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(BankRedirectData::OpenBanking {
                account_number,
                sort_code,
                iban,
                account_holder_name,
                additional_details,
                ..
            }) => {
                let currency = item.router_data.request.currency;
                let amount_in_minor = item.router_data.request.amount;

                let hosted_page = HostedPage {
                    return_uri: item.router_data.request.router_return_url.clone().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "return_url",
                            context: Default::default(),
                        },
                    )?,
                };

                let metadata = TruelayerMetadata::try_from(&item.router_data.connector_config)?;

                let provider_id = additional_details
                    .as_ref()
                    .and_then(|details| details.peek().get("provider_id"))
                    .and_then(|pid| pid.as_str())
                    .map(|s| s.to_string());

                let provider_selection = if provider_id.is_some()
                    && account_holder_name.is_some()
                    && ((account_number.is_some() && sort_code.is_some()) || iban.is_some())
                {
                    ProviderSelection {
                        _type: ProviderSelectionType::Preselected,
                        provider_id: provider_id.clone(),
                        remitter: Some(Remitter {
                            account_holder_name: account_holder_name.clone(),
                            account_identifier: if account_number.is_some() && sort_code.is_some() {
                                TruelayerAccountIdentifier {
                                    identifier_type:
                                        TruelayerAccountIdentifierType::SortCodeAccountNumber,
                                    sort_code: sort_code.clone(),
                                    account_number: account_number.clone(),
                                    iban: None,
                                }
                            } else {
                                TruelayerAccountIdentifier {
                                    identifier_type: TruelayerAccountIdentifierType::Iban,
                                    sort_code: None,
                                    account_number: None,
                                    iban: iban.clone(),
                                }
                            },
                        }),
                        scheme_selection: Some(SchemeSelection {
                            _type: SCHEME_SELECTION_TYPE.to_string(),
                        }),
                    }
                } else {
                    ProviderSelection {
                        _type: ProviderSelectionType::UserSelected,
                        provider_id: None,
                        remitter: None,
                        scheme_selection: None,
                    }
                };

                let payment_method = PaymentMethod {
                    _type: PAYMENT_METHOD_TYPE.to_string(),
                    provider_selection,
                    beneficiary: Beneficiary {
                        _type: BENEFICIARY_TYPE.to_string(),
                        merchant_account_id: metadata.merchant_account_id.clone(),
                        account_holder_name: metadata.account_holder_name.clone(),
                        reference: normalize_connector_request_reference_id(
                            &item
                                .router_data
                                .resource_common_data
                                .connector_request_reference_id,
                        ),
                    },
                };

                let email = item.router_data.request.email.clone().or_else(|| {
                    item.router_data
                        .resource_common_data
                        .get_optional_billing_email()
                });

                let phone = item
                    .router_data
                    .resource_common_data
                    .address
                    .get_payment_billing()
                    .map(|billing| billing.get_phone_with_country_code())
                    .transpose()
                    .ok()
                    .flatten();

                // Ensure at least one is present
                if email.is_none() && phone.is_none() {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "either billing.email/customer_email or billing.phone",
                        context: Default::default(),
                    }
                    .into());
                }

                let address = item
                    .router_data
                    .resource_common_data
                    .get_optional_billing()
                    .and_then(get_address);

                let user = User {
                    id: item
                        .router_data
                        .resource_common_data
                        .get_connector_customer_id()
                        .ok(),
                    name: item
                        .router_data
                        .request
                        .customer_name
                        .clone()
                        .map(Secret::new)
                        .or_else(|| {
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                        })
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "billing.first_name or customer_name",
                            context: Default::default(),
                        })?,
                    email,
                    phone,
                    address,
                };

                Ok(Self {
                    amount_in_minor,
                    currency,
                    hosted_page,
                    payment_method,
                    user,
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Truelayer"),
                Default::default(),
            )
            .into()),
        }
    }
}

impl<F, T> TryFrom<ResponseRouterData<TruelayerPaymentsResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<TruelayerPaymentsResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_attempt_status(item.response.status.clone());

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: item
                    .response
                    .failure_reason
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .failure_reason
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: item.response.failure_reason.clone(),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        } else {
            let redirection_url = item
                .response
                .hosted_page
                .as_ref()
                .map(|hosted_page| hosted_page.uri.clone())
                .ok_or_else(|| {
                    error_stack::report!(
                        utils::unexpected_response_fail(
                            item.http_code
                        , "truelayer: unexpected response for this operation; retry with idempotency keys and check connector status.")
                    )
                })?;

            let redirection_data = Some(RedirectForm::Form {
                endpoint: redirection_url,
                method: Method::Get,
                form_fields: Default::default(),
            });

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                    redirection_data: redirection_data.map(Box::new),
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.id),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TruelayerPSyncResponseData {
    PSyncResponse(TruelayerPSyncResponse),
    WebhookResponse(TruelayerWebhookBody),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPSyncResponse {
    id: String,
    amount_in_minor: MinorUnit,
    currency: Currency,
    user: Option<UserIdResponse>,
    status: TruelayerPaymentStatus,
    failure_reason: Option<String>,
    failure_stage: Option<String>,
    payment_source: Option<TruelayerPaymentSource>,
    payment_method: Option<TruelayerPaymentMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPaymentMethod {
    provider_selection: Option<TruelayerProviderSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerProviderSelection {
    provider_id: Option<String>,
}

fn map_truelayer_provider_id_to_bank_name(
    provider_id: &str,
) -> Result<common_enums::BankNames, String> {
    match provider_id.trim() {
        "xs2a-bank-austria" => Ok(common_enums::BankNames::BankAustria),
        "xs2a-bawag-psk" => Ok(common_enums::BankNames::BawagPsk),
        "xs2a-easybank" => Ok(common_enums::BankNames::EasyBank),
        "xs2a-erste-at" => Ok(common_enums::BankNames::ErsteBank),
        "xs2a-ing-at" | "xs2a-ing-be" | "xs2a-ing-germany" | "xs2a-ing-spain" | "xs2a-ing-it" => {
            Ok(common_enums::BankNames::Ing)
        }
        "xs2a-raiffeisen-at" => Ok(common_enums::BankNames::Raiffeisenbank),
        "ob-revolut-at" | "ob-revolut-be" | "ob-revolut-de" | "ob-revolut-es" | "ob-revolut-ee"
        | "ob-revolut-fi" | "ob-revolut-fr" | "ob-revolut" | "ob-revolut-ie" | "ob-revolut-is" => {
            Ok(common_enums::BankNames::Revolut)
        }
        "xs2a-sparda-bank-wien" => Ok(common_enums::BankNames::SpardaBankWien),
        "xs2a-argenta-be" => Ok(common_enums::BankNames::Argenta),
        "xs2a-belfius" => Ok(common_enums::BankNames::Belfius),
        "stet-beobank" => Ok(common_enums::BankNames::Beobank),
        "stet-bnp-paribas-fortis-be" => Ok(common_enums::BankNames::BnpParibasFortis),
        "xs2a-cbc-be" => Ok(common_enums::BankNames::CbcBanque),
        "stet-fintro" => Ok(common_enums::BankNames::Fintro),
        "stet-hello-bank-be" | "stet-bnp-paribas-hello-bank" | "xs2a-hello-bank-it" => {
            Ok(common_enums::BankNames::HelloBank)
        }
        "xs2a-kbc-be" => Ok(common_enums::BankNames::Kbc),
        "xs2a-kbc-brussels-be" => Ok(common_enums::BankNames::KbcBrussels),
        "xs2a-n26-be" | "xs2a-n26-de" | "xs2a-n26-es" | "xs2a-n26-fi" | "xs2a-n26-fr"
        | "xs2a-n26-it" | "xs2a-n26-nl" => Ok(common_enums::BankNames::N26),
        "xs2a-triodos-be" => Ok(common_enums::BankNames::Triodos),
        "ob-transferwise-be" | "ob-transferwise-de" | "ob-transferwise-es"
        | "ob-transferwise-fr" | "ob-transferwise" | "ob-transferwise-ie" => {
            Ok(common_enums::BankNames::Wise)
        }
        "xs2a-comdirect" => Ok(common_enums::BankNames::Comdirect),
        "xs2a-commerzbank" => Ok(common_enums::BankNames::Commerzbank),
        "xs2a-deutsche-bank" => Ok(common_enums::BankNames::DeutscheBank),
        "xs2a-deutschekredit-de" => Ok(common_enums::BankNames::Dkb),
        "xs2a-hypovereinsbank" => Ok(common_enums::BankNames::HypoVereinsbank),
        "xs2a-postbank-de" => Ok(common_enums::BankNames::PostBank),
        "xs2a-santander-de" | "ob-santander" => Ok(common_enums::BankNames::Santander),
        "xs2a-sparkasse" => Ok(common_enums::BankNames::Sparkasse),
        "xs2a-targobank-de" => Ok(common_enums::BankNames::TargoBank),
        "xs2a-volksbanken-de" => Ok(common_enums::BankNames::VolksbankenRaiffeisenbanken),
        "xs2a-redsys-banco-sabadell" => Ok(common_enums::BankNames::BancoDeSabadell),
        "xs2a-redsys-banco-santander" => Ok(common_enums::BankNames::BancoSantander),
        "xs2a-bankinter-es" => Ok(common_enums::BankNames::Bankinter),
        "xs2a-redsys-bbva" | "xs2a-redsys-bbva-it" => Ok(common_enums::BankNames::Bbva),
        "xs2a-redsys-caixabank" => Ok(common_enums::BankNames::Caixa),
        "xs2a-grupo-caja-rural" => Ok(common_enums::BankNames::CajaRural),
        "xs2a-cajamar" => Ok(common_enums::BankNames::Cajamar),
        "xs2a-evo-banco" => Ok(common_enums::BankNames::EvoBanco),
        "xs2a-ibercaja" => Ok(common_enums::BankNames::Ibercaja),
        "xs2a-imaginbank" => Ok(common_enums::BankNames::Imagin),
        "xs2a-kutxabank" => Ok(common_enums::BankNames::Kutxabank),
        "xs2a-laboral-kutxa" => Ok(common_enums::BankNames::LaboralKutxa),
        "xs2a-openbank" => Ok(common_enums::BankNames::Openbank),
        "xs2a-unicaja-banco" => Ok(common_enums::BankNames::Unicaja),
        "xs2a-aktia-fi" => Ok(common_enums::BankNames::Aktia),
        "xs2a-alandsbanken-fi" => Ok(common_enums::BankNames::Alandsbanken),
        "xs2a-danske-fi" | "ob-danske" => Ok(common_enums::BankNames::DanskeBank),
        "xs2a-handelsbanken-fi" => Ok(common_enums::BankNames::Handelsbanken),
        "xs2a-nordea-fi" => Ok(common_enums::BankNames::Nordea),
        "xs2a-oma-sp-fi" => Ok(common_enums::BankNames::OmaSp),
        "xs2a-op-fi" => Ok(common_enums::BankNames::Op),
        "xs2a-pop-pankki-fi" => Ok(common_enums::BankNames::PopPankki),
        "xs2a-s-pankki-fi" => Ok(common_enums::BankNames::SPankki),
        "xs2a-saastopankki-fi" => Ok(common_enums::BankNames::Saastopankki),
        "stet-allianz" => Ok(common_enums::BankNames::AllianzBanque),
        "stet-arkea-banque-entreprises" => {
            Ok(common_enums::BankNames::ArkeaBanqueEntreprisesEtInstitutionnels)
        }
        "stet-arkea-banque-privee" => Ok(common_enums::BankNames::ArkeaBanquePrivee),
        "stet-axa" => Ok(common_enums::BankNames::AxaBanque),
        "stet-banque-de-savoie" => Ok(common_enums::BankNames::BanqueDeSavoie),
        "stet-banque-populaire" => Ok(common_enums::BankNames::BanquePopulaire),
        "stet-bnp-paribas-ma-banque" => Ok(common_enums::BankNames::BnpParibas),
        "stet-boursorama" => Ok(common_enums::BankNames::BoursoBank),
        "stet-bpe" => Ok(common_enums::BankNames::Bpe),
        "stet-caisse-d-epargne" => Ok(common_enums::BankNames::CaisseDEpargne),
        "stet-cic" => Ok(common_enums::BankNames::Cic),
        "stet-credit-agricole" | "xs2a-credit-agricole-it" => {
            Ok(common_enums::BankNames::CreditAgricole)
        }
        "stet-credit-mutuel" => Ok(common_enums::BankNames::CreditMutuel),
        "stet-credit-mutuel-de-bretagne" => Ok(common_enums::BankNames::CreditMutuelDeBretagne),
        "stet-credit-mutuel-de-sud-ouest" => Ok(common_enums::BankNames::CreditMutuelDuSudOuest),
        "stet-fortuneo" => Ok(common_enums::BankNames::Fortuneo),
        "stet-banque-postale" => Ok(common_enums::BankNames::LaBanquePostale),
        "stet-banque-postale-business" => Ok(common_enums::BankNames::LaBanquePostaleBusiness),
        "stet-lcl" => Ok(common_enums::BankNames::Lcl),
        "stet-monabanq" => Ok(common_enums::BankNames::Monabanq),
        "stet-societe-generale" => Ok(common_enums::BankNames::SocieteGenerale),
        "ob-aib-gb" => Ok(common_enums::BankNames::AlliedIrishBank),
        "ob-aib-gb-corporate" => Ok(common_enums::BankNames::AlliedIrishBankCorporate),
        "ob-amex" => Ok(common_enums::BankNames::AmericanExpress),
        "ob-boi" => Ok(common_enums::BankNames::BankOfIrelandUk),
        "ob-bos" => Ok(common_enums::BankNames::BankOfScotland),
        "ob-bos-business" => Ok(common_enums::BankNames::BankOfScotlandBusiness),
        "ob-barclaycard" => Ok(common_enums::BankNames::Barclaycard),
        "ob-barclays" => Ok(common_enums::BankNames::Barclays),
        "ob-barclays-business" => Ok(common_enums::BankNames::BarclaysBusiness),
        "ob-capital-one" => Ok(common_enums::BankNames::CapitalOne),
        "ob-chase" => Ok(common_enums::BankNames::Chase),
        "ob-clydesdale-bank" => Ok(common_enums::BankNames::ClydesdaleBank),
        "ob-coutts" => Ok(common_enums::BankNames::Coutts),
        "ob-danske-business" => Ok(common_enums::BankNames::DanskeBankBusiness),
        "ob-first-direct" => Ok(common_enums::BankNames::FirstDirect),
        "ob-halifax" => Ok(common_enums::BankNames::Halifax),
        "ob-hsbc" => Ok(common_enums::BankNames::Hsbc),
        "ob-hsbc-business" => Ok(common_enums::BankNames::HsbcBusiness),
        "ob-lloyds" => Ok(common_enums::BankNames::Lloyds),
        "ob-lloyds-business" => Ok(common_enums::BankNames::LloydsBusiness),
        "ob-lloyds-commercial" => Ok(common_enums::BankNames::LloydsCommercial),
        "ob-ms" => Ok(common_enums::BankNames::MSBank),
        "ob-mbna" => Ok(common_enums::BankNames::Mbna),
        "ob-mettle" => Ok(common_enums::BankNames::MettleBank),
        "ob-monzo" => Ok(common_enums::BankNames::Monzo),
        "ob-nationwide" => Ok(common_enums::BankNames::Nationwide),
        "ob-natwest" => Ok(common_enums::BankNames::NatWest),
        "ob-natwest-business" => Ok(common_enums::BankNames::NatWestBankline),
        "ob-rbs" => Ok(common_enums::BankNames::RoyalBankOfScotland),
        "ob-rbs-business" => Ok(common_enums::BankNames::RoyalBankOfScotlandBankline),
        "ob-santander-business" => Ok(common_enums::BankNames::SantanderBusiness),
        "ob-santander-personal" => Ok(common_enums::BankNames::SantanderPersonal),
        "ob-starling" => Ok(common_enums::BankNames::Starling),
        "ob-tesco" => Ok(common_enums::BankNames::TescoBank),
        "ob-tide" => Ok(common_enums::BankNames::Tide),
        "ob-tsb" => Ok(common_enums::BankNames::Tsb),
        "ob-ulster" => Ok(common_enums::BankNames::UlsterBank),
        "ob-ulster-business" => Ok(common_enums::BankNames::UlsterBankline),
        "ob-virgin-money" => Ok(common_enums::BankNames::VirginMoney),
        "ob-virgin-money-merged" => Ok(common_enums::BankNames::VirginMoneyMerged),
        "ob-yorkshire-bank" => Ok(common_enums::BankNames::YorkshireBank),
        "ob-cashplus" => Ok(common_enums::BankNames::Zempler),
        "ob-aib" => Ok(common_enums::BankNames::Aib),
        "ob-aib-business" => Ok(common_enums::BankNames::AibBusiness),
        "ob-boi-ie" => Ok(common_enums::BankNames::BankOfIreland),
        "ob-boi-ie-business" => Ok(common_enums::BankNames::BankOfIrelandBusiness),
        "ob-ebs" => Ok(common_enums::BankNames::Ebs),
        "ob-ptsb" => Ok(common_enums::BankNames::Ptsb),
        "mybank-allianz-bank-financial-advisors-spa-it" => {
            Ok(common_enums::BankNames::AllianzBankFinancialAdvisorsSpa)
        }
        "mybank-alto-adige-it" => Ok(common_enums::BankNames::AltoAdige),
        "mybank-alto-adige-banca-suedtirol-bank-it" => {
            Ok(common_enums::BankNames::AltoAdigeBancaSuedtirolBank)
        }
        "mybank-banca-360-credito-cooperativo-fvg-it" => {
            Ok(common_enums::BankNames::Banca360CreditoCooperativoFvg)
        }
        "mybank-banca-adria-colli-euganei-it" => {
            Ok(common_enums::BankNames::BancaAdriaColliEuganei)
        }
        "mybank-banca-agricola-popolare-di-ragusa-it" => {
            Ok(common_enums::BankNames::BancaAgricolaPopolareDiRagusa)
        }
        "mybank-banca-alpi-marittime-cc-carru-it" => {
            Ok(common_enums::BankNames::BancaAlpiMarittimeCcCarru)
        }
        "mybank-banca-alta-toscana-it" => Ok(common_enums::BankNames::BancaAltaToscana),
        "mybank-banca-annia-it" => Ok(common_enums::BankNames::BancaAnnia),
        "mybank-banca-centro-emilia-it" => Ok(common_enums::BankNames::BancaCentroEmilia),
        "mybank-banca-centro-lazio-it" => Ok(common_enums::BankNames::BancaCentroLazio),
        "mybank-banca-centro-toscana-umbria-it" => {
            Ok(common_enums::BankNames::BancaCentroToscanaUmbria)
        }
        "mybank-banca-centropadana-it" => Ok(common_enums::BankNames::BancaCentropadana),
        "xs2a-cesare-ponti-it" => Ok(common_enums::BankNames::BancaCesarePonti),
        "mybank-banca-del-catanzarese-it" => Ok(common_enums::BankNames::BancaDelCatanzarese),
        "mybank-banca-del-cilento-di-sassano-e-v-it" => {
            Ok(common_enums::BankNames::BancaDelCilentoDiSassanoEV)
        }
        "mybank-banca-del-piceno-it" => Ok(common_enums::BankNames::BancaDelPiceno),
        "mybank-banca-del-piemonte-it" => Ok(common_enums::BankNames::BancaDelPiemonte),
        "mybank-banca-del-territorio-lombardo-it" => {
            Ok(common_enums::BankNames::BancaDelTerritorioLombardo)
        }
        "mybank-banca-del-veneto-centrale-it" => {
            Ok(common_enums::BankNames::BancaDelVenetoCentrale)
        }
        "mybank-banca-della-marca-credcooperativo-it" => {
            Ok(common_enums::BankNames::BancaDellaMarcaCredcooperativo)
        }
        "mybank-banca-delle-terre-venete-it" => Ok(common_enums::BankNames::BancaDelleTerreVenete),
        "mybank-banca-di-alba-credito-cooperativo-it" => {
            Ok(common_enums::BankNames::BancaDiAlbaCreditoCooperativo)
        }
        "mybank-banca-di-anghiari-e-stia-cc-it" => {
            Ok(common_enums::BankNames::BancaDiAnghiariEStiaCc)
        }
        "mybank-banca-di-bologna-it" => Ok(common_enums::BankNames::BancaDiBologna),
        "mybank-banca-di-caraglio-it" => Ok(common_enums::BankNames::BancaDiCaraglio),
        "mybank-banca-di-credito-popolare-scpa-it" => {
            Ok(common_enums::BankNames::BancaDiCreditoPopolareScpa)
        }
        "mybank-banca-di-imola-spa-it" => Ok(common_enums::BankNames::BancaDiImolaSpa),
        "mybank-banca-di-pesaro-it" => Ok(common_enums::BankNames::BancaDiPesaro),
        "mybank-banca-di-pescia-e-cascina-it" => Ok(common_enums::BankNames::BancaDiPesciaECascina),
        "mybank-banca-di-piacenza-scpa-it" => Ok(common_enums::BankNames::BancaDiPiacenzaScpa),
        "mybank-banca-di-taranto-bcc-it" => Ok(common_enums::BankNames::BancaDiTarantoBcc),
        "mybank-banca-di-udine-credito-coop-it" => {
            Ok(common_enums::BankNames::BancaDiUdineCreditoCoop)
        }
        "mybank-banca-don-rizzo-it" => Ok(common_enums::BankNames::BancaDonRizzo),
        "mybank-banca-fideuram-it" => Ok(common_enums::BankNames::BancaFideuram),
        "mybank-banca-finnat-euramerica-spa-it" => {
            Ok(common_enums::BankNames::BancaFinnatEuramericaSpa)
        }
        "mybank-banca-generali-spa-it" => Ok(common_enums::BankNames::BancaGeneraliSpa),
        "mybank-banca-lazio-nord-it" => Ok(common_enums::BankNames::BancaLazioNord),
        "mybank-banca-malatestiana-it" => Ok(common_enums::BankNames::BancaMalatestiana),
        "xs2a-mps-it" | "mybank-banca-monte-dei-paschi-di-siena-it" => {
            Ok(common_enums::BankNames::BancaMonteDeiPaschiDiSiena)
        }
        "mybank-banca-passadore-it" => Ok(common_enums::BankNames::BancaPassadore),
        "mybank-banca-patavina-it" => Ok(common_enums::BankNames::BancaPatavina),
        "mybank-banca-patrimoni-sella-it" => Ok(common_enums::BankNames::BancaPatrimoniSella),
        "mybank-banca-per-il-trentinoaltoadige-it" => {
            Ok(common_enums::BankNames::BancaPerIlTrentinoaltoadige)
        }
        "mybank-banca-popolare-del-lazio-scpa-it" => {
            Ok(common_enums::BankNames::BancaPopolareDelLazioScpa)
        }
        "mybank-banca-popolare-dell-alto-adige-it" => {
            Ok(common_enums::BankNames::BancaPopolareDellAltoAdige)
        }
        "xs2a-banca-popolare-sondrio-it" | "mybank-banca-popolare-di-sondrio-it" => {
            Ok(common_enums::BankNames::BancaPopolareDiSondrio)
        }
        "mybank-banca-popolare-pugliese-it" => Ok(common_enums::BankNames::BancaPopolarePugliese),
        "mybank-banca-popolare-valconca-scpa-it" => {
            Ok(common_enums::BankNames::BancaPopolareValconcaScpa)
        }
        "mybank-banca-san-francesco-credito-coop-it" => {
            Ok(common_enums::BankNames::BancaSanFrancescoCreditoCoop)
        }
        "xs2a-banca-sella-it" | "mybank-banca-sella-it" => Ok(common_enums::BankNames::BancaSella),
        "mybank-banca-sistema-spa-it" => Ok(common_enums::BankNames::BancaSistemaSpa),
        "mybank-banca-sviluppo-cooperaz-credito-it" => {
            Ok(common_enums::BankNames::BancaSviluppoCooperazCredito)
        }
        "mybank-banca-tema-it" => Ok(common_enums::BankNames::BancaTema),
        "mybank-banca-terre-etrusche-e-di-maremma-it" => {
            Ok(common_enums::BankNames::BancaTerreEtruscheEDiMaremma)
        }
        "mybank-banca-territori-del-monviso-it" => {
            Ok(common_enums::BankNames::BancaTerritoriDelMonviso)
        }
        "mybank-banca-valsabbina-it" => Ok(common_enums::BankNames::BancaValsabbina),
        "mybank-banca-veronese-cc-di-concamarise-it" => {
            Ok(common_enums::BankNames::BancaVeroneseCcDiConcamarise)
        }
        "mybank-banco-azzoaglio-it" => Ok(common_enums::BankNames::BancoAzzoaglio),
        "mybank-banco-bpm-spa-servizio-webank-it" => {
            Ok(common_enums::BankNames::BancoBpmSpaServizioWebank)
        }
        "mybank-banco-bpm-spa-servizio-youweb-it" => {
            Ok(common_enums::BankNames::BancoBpmSpaServizioYouweb)
        }
        "mybank-banco-bpm-spa-youbusiness-web-it" => {
            Ok(common_enums::BankNames::BancoBpmSpaYoubusinessWeb)
        }
        "xs2a-banco-bpm-we-bank-it" => Ok(common_enums::BankNames::BancoBpmWeBank),
        "xs2a-banco-bpm-you-web-it" => Ok(common_enums::BankNames::BancoBpmYouWeb),
        "mybank-banco-desio-brianza-it" => Ok(common_enums::BankNames::BancoDesioBrianza),
        "xs2a-banco-di-sardegna-it" => Ok(common_enums::BankNames::BancoDiSardegna),
        "mybank-banco-marchigiano-it" => Ok(common_enums::BankNames::BancoMarchigiano),
        "xs2a-banco-posta-it" => Ok(common_enums::BankNames::BancoPosta),
        "mybank-bcc-abruzzese-cappelle-sul-tavo-it" => {
            Ok(common_enums::BankNames::BccAbruzzeseCappelleSulTavo)
        }
        "mybank-bcc-abruzzi-e-molise-it" => Ok(common_enums::BankNames::BccAbruzziEMolise),
        "mybank-bcc-adriatico-teramano-it" => Ok(common_enums::BankNames::BccAdriaticoTeramano),
        "mybank-bcc-agro-bresciano-it" => Ok(common_enums::BankNames::BccAgroBresciano),
        "mybank-bcc-agro-pontino-it" => Ok(common_enums::BankNames::BccAgroPontino),
        "mybank-bcc-alberobello-sammichele-monopoli-it" => {
            Ok(common_enums::BankNames::BccAlberobelloSammicheleMonopoli)
        }
        "mybank-bcc-alto-tirreno-della-calabria-it" => {
            Ok(common_enums::BankNames::BccAltoTirrenoDellaCalabria)
        }
        "mybank-bcc-anagni-it" => Ok(common_enums::BankNames::BccAnagni),
        "mybank-bcc-basilicata-it" => Ok(common_enums::BankNames::BccBasilicata),
        "mybank-bcc-bellegra-it" => Ok(common_enums::BankNames::BccBellegra),
        "mybank-bcc-brescia-it" => Ok(common_enums::BankNames::BccBrescia),
        "mybank-bcc-brianza-e-laghi-it" => Ok(common_enums::BankNames::BccBrianzaELaghi),
        "mybank-bcc-campania-centro-it" => Ok(common_enums::BankNames::BccCampaniaCentro),
        "mybank-bcc-capaccio-paestum-it" => Ok(common_enums::BankNames::BccCapaccioPaestum),
        "mybank-bcc-castelli-romani-e-tuscolo-it" => {
            Ok(common_enums::BankNames::BccCastelliRomaniETuscolo)
        }
        "mybank-bcc-centro-calabria-it" => Ok(common_enums::BankNames::BccCentroCalabria),
        "mybank-bcc-conversano-it" => Ok(common_enums::BankNames::BccConversano),
        "mybank-bcc-degli-ulivi-terra-di-bari-it" => {
            Ok(common_enums::BankNames::BccDegliUliviTerraDiBari)
        }
        "mybank-bcc-dei-castelli-e-degli-iblei-it" => {
            Ok(common_enums::BankNames::BccDeiCastelliEDegliIblei)
        }
        "mybank-bcc-dei-colli-albani-it" => Ok(common_enums::BankNames::BccDeiColliAlbani),
        "mybank-bcc-del-circeo-e-privernate-it" => {
            Ok(common_enums::BankNames::BccDelCirceoEPrivernate)
        }
        "mybank-bcc-del-garda-it" => Ok(common_enums::BankNames::BccDelGarda),
        "mybank-bcc-del-metauro-it" => Ok(common_enums::BankNames::BccDelMetauro),
        "mybank-bcc-del-velino-it" => Ok(common_enums::BankNames::BccDelVelino),
        "mybank-bcc-dell-alta-murgia-it" => Ok(common_enums::BankNames::BccDellAltaMurgia),
        "mybank-bcc-della-provincia-romana-it" => {
            Ok(common_enums::BankNames::BccDellaProvinciaRomana)
        }
        "mybank-bcc-della-romagna-occidentale-it" => {
            Ok(common_enums::BankNames::BccDellaRomagnaOccidentale)
        }
        "mybank-bcc-delle-madonie-it" => Ok(common_enums::BankNames::BccDelleMadonie),
        "mybank-bcc-di-altofonte-e-caccamo-it" => {
            Ok(common_enums::BankNames::BccDiAltofonteECaccamo)
        }
        "mybank-bcc-di-aquara-it" => Ok(common_enums::BankNames::BccDiAquara),
        "mybank-bcc-di-arborea-it" => Ok(common_enums::BankNames::BccDiArborea),
        "mybank-bcc-di-bari-it" => Ok(common_enums::BankNames::BccDiBari),
        "mybank-bcc-di-barlassina-it" => Ok(common_enums::BankNames::BccDiBarlassina),
        "mybank-bcc-di-bene-vagienna-it" => Ok(common_enums::BankNames::BccDiBeneVagienna),
        "mybank-bcc-di-binasco-it" => Ok(common_enums::BankNames::BccDiBinasco),
        "mybank-bcc-di-buccino-e-comuni-cilentani-it" => {
            Ok(common_enums::BankNames::BccDiBuccinoEComuniCilentani)
        }
        "mybank-bcc-di-busto-garolfo-e-buguggiate-it" => {
            Ok(common_enums::BankNames::BccDiBustoGarolfoEBuguggiate)
        }
        "mybank-bcc-di-cagliari-it" => Ok(common_enums::BankNames::BccDiCagliari),
        "mybank-bcc-di-canosa-loconia-it" => Ok(common_enums::BankNames::BccDiCanosaLoconia),
        "mybank-bcc-di-caravaggio-it" => Ok(common_enums::BankNames::BccDiCaravaggio),
        "mybank-bcc-di-cassano-delle-murge-e-tolve-it" => {
            Ok(common_enums::BankNames::BccDiCassanoDelleMurgeETolve)
        }
        "mybank-bcc-di-cherasco-it" => Ok(common_enums::BankNames::BccDiCherasco),
        "mybank-bcc-di-filottrano-it" => Ok(common_enums::BankNames::BccDiFilottrano),
        "mybank-bcc-di-flumeri-it" => Ok(common_enums::BankNames::BccDiFlumeri),
        "mybank-bcc-di-gambatesa-it" => Ok(common_enums::BankNames::BccDiGambatesa),
        "mybank-bcc-di-gaudiano-di-lavello-it" => {
            Ok(common_enums::BankNames::BccDiGaudianoDiLavello)
        }
        "mybank-bcc-di-leverano-it" => Ok(common_enums::BankNames::BccDiLeverano),
        "mybank-bcc-di-locorotondo-it" => Ok(common_enums::BankNames::BccDiLocorotondo),
        "mybank-bcc-di-montepaone-it" => Ok(common_enums::BankNames::BccDiMontepaone),
        "mybank-bcc-di-napoli-it" => Ok(common_enums::BankNames::BccDiNapoli),
        "mybank-bcc-di-ostra-e-morro-d-alba-it" => {
            Ok(common_enums::BankNames::BccDiOstraEMorroDAlba)
        }
        "mybank-bcc-di-ostuni-it" => Ok(common_enums::BankNames::BccDiOstuni),
        "mybank-bcc-di-pachino-it" => Ok(common_enums::BankNames::BccDiPachino),
        "mybank-bcc-di-pergola-e-corinaldo-it" => {
            Ok(common_enums::BankNames::BccDiPergolaECorinaldo)
        }
        "mybank-bcc-di-pianfei-e-rocca-de-baldi-it" => {
            Ok(common_enums::BankNames::BccDiPianfeiERoccaDeBaldi)
        }
        "mybank-bcc-di-pontassieve-it" => Ok(common_enums::BankNames::BccDiPontassieve),
        "mybank-bcc-di-recanati-e-colmurano-it" => {
            Ok(common_enums::BankNames::BccDiRecanatiEColmurano)
        }
        "mybank-bcc-di-roma-it" => Ok(common_enums::BankNames::BccDiRoma),
        "mybank-bcc-di-san-giovanni-rotondo-it" => {
            Ok(common_enums::BankNames::BccDiSanGiovanniRotondo)
        }
        "mybank-bcc-di-san-marzano-di-san-giuseppe-it" => {
            Ok(common_enums::BankNames::BccDiSanMarzanoDiSanGiuseppe)
        }
        "mybank-bcc-di-santeramo-in-colle-it" => Ok(common_enums::BankNames::BccDiSanteramoInColle),
        "mybank-bcc-di-sarsina-it" => Ok(common_enums::BankNames::BccDiSarsina),
        "mybank-bcc-di-scafati-e-cetara-it" => Ok(common_enums::BankNames::BccDiScafatiECetara),
        "mybank-bcc-di-smarco-dei-cavoti-it" => Ok(common_enums::BankNames::BccDiSmarcoDeiCavoti),
        "mybank-bcc-di-spello-e-del-velino-it" => {
            Ok(common_enums::BankNames::BccDiSpelloEDelVelino)
        }
        "mybank-bcc-di-terra-d-otranto-it" => Ok(common_enums::BankNames::BccDiTerraDOtranto),
        "mybank-bcc-felsinea-it" => Ok(common_enums::BankNames::BccFelsinea),
        "mybank-bcc-g-toniolo-di-san-cataldo-it" => {
            Ok(common_enums::BankNames::BccGTonioloDiSanCataldo)
        }
        "mybank-bcc-gran-sasso-d-italia-it" => Ok(common_enums::BankNames::BccGranSassoDItalia),
        "mybank-bcc-la-riscossa-di-regalbuto-it" => {
            Ok(common_enums::BankNames::BccLaRiscossaDiRegalbuto)
        }
        "mybank-bcc-lodi-it" => Ok(common_enums::BankNames::BccLodi),
        "mybank-bcc-milano-it" => Ok(common_enums::BankNames::BccMilano),
        "mybank-bcc-monte-pruno-it" => Ok(common_enums::BankNames::BccMontePruno),
        "mybank-bcc-nettuno-it" => Ok(common_enums::BankNames::BccNettuno),
        "mybank-bcc-oglio-e-serio-it" => Ok(common_enums::BankNames::BccOglioESerio),
        "mybank-bcc-pordenonese-e-monsile-it" => {
            Ok(common_enums::BankNames::BccPordenoneseEMonsile)
        }
        "mybank-bcc-pratola-peligna-it" => Ok(common_enums::BankNames::BccPratolaPeligna),
        "mybank-bcc-prealpi-san-biagio-it" => Ok(common_enums::BankNames::BccPrealpiSanBiagio),
        "mybank-bcc-ravenna-forli-imola-it" => Ok(common_enums::BankNames::BccRavennaForliImola),
        "mybank-bcc-san-giuseppe-di-mussomeli-it" => {
            Ok(common_enums::BankNames::BccSanGiuseppeDiMussomeli)
        }
        "mybank-bcc-terra-di-lavoro-it" => Ok(common_enums::BankNames::BccTerraDiLavoro),
        "mybank-bcc-triuggio-valle-del-lambro-it" => {
            Ok(common_enums::BankNames::BccTriuggioValleDelLambro)
        }
        "mybank-bcc-valdarno-fiorentino-it" => Ok(common_enums::BankNames::BccValdarnoFiorentino),
        "mybank-bcc-valdostana-it" => Ok(common_enums::BankNames::BccValdostana),
        "mybank-bcc-valle-del-torto-it" => Ok(common_enums::BankNames::BccValleDelTorto),
        "mybank-bcc-veneta-it" => Ok(common_enums::BankNames::BccVeneta),
        "mybank-bcc-venezia-giulia-it" => Ok(common_enums::BankNames::BccVeneziaGiulia),
        "mybank-bcc-versilia-lunigiana-e-garfagnana-it" => {
            Ok(common_enums::BankNames::BccVersiliaLunigianaEGarfagnana)
        }
        "mybank-bcc-vicentino-pojana-maggiore-it" => {
            Ok(common_enums::BankNames::BccVicentinoPojanaMaggiore)
        }
        "xs2a-bibanca-it" => Ok(common_enums::BankNames::BiBanca),
        "mybank-blu-banca-spa-it" => Ok(common_enums::BankNames::BluBancaSpa),
        "xs2a-bnl-it" => Ok(common_enums::BankNames::Bnl),
        "mybank-bozen-it" => Ok(common_enums::BankNames::Bozen),
        "xs2a-bper-it" => Ok(common_enums::BankNames::BperBanca),
        "mybank-bvr-banca-banche-venete-riunite-it" => {
            Ok(common_enums::BankNames::BvrBancaBancheVeneteRiunite)
        }
        "mybank-cassa-centrale-banca-it" => Ok(common_enums::BankNames::CassaCentraleBanca),
        "mybank-cassa-di-risparmio-di-bolzano-it" => {
            Ok(common_enums::BankNames::CassaDiRisparmioDiBolzano)
        }
        "mybank-cassa-di-risparmio-di-fermo-spa-it" => {
            Ok(common_enums::BankNames::CassaDiRisparmioDiFermoSpa)
        }
        "mybank-cassa-di-risparmio-di-savigliano-it" => {
            Ok(common_enums::BankNames::CassaDiRisparmioDiSavigliano)
        }
        "mybank-cassa-padana-it" => Ok(common_enums::BankNames::CassaPadana),
        "mybank-cassa-rurale-alta-valsugana-it" => {
            Ok(common_enums::BankNames::CassaRuraleAltaValsugana)
        }
        "mybank-cassa-rurale-alto-garda-rovereto-it" => {
            Ok(common_enums::BankNames::CassaRuraleAltoGardaRovereto)
        }
        "mybank-cassa-rurale-di-ledro-it" => Ok(common_enums::BankNames::CassaRuraleDiLedro),
        "mybank-cassa-rurale-di-treviglio-it" => {
            Ok(common_enums::BankNames::CassaRuraleDiTreviglio)
        }
        "mybank-cassa-rurale-fvg-it" => Ok(common_enums::BankNames::CassaRuraleFvg),
        "mybank-cassa-rurale-renon-it" => Ok(common_enums::BankNames::CassaRuraleRenon),
        "mybank-cassa-rurale-val-di-fiemme-it" => {
            Ok(common_enums::BankNames::CassaRuraleValDiFiemme)
        }
        "mybank-cassa-rurale-val-di-sole-it" => Ok(common_enums::BankNames::CassaRuraleValDiSole),
        "mybank-cassa-rurale-vallagarina-it" => Ok(common_enums::BankNames::CassaRuraleVallagarina),
        "mybank-cassa-rurale-valsugana-e-tesino-it" => {
            Ok(common_enums::BankNames::CassaRuraleValsuganaETesino)
        }
        "mybank-castagneto-banca-1910-it" => Ok(common_enums::BankNames::CastagnetoBanca1910),
        "mybank-centromarca-banca-it" => Ok(common_enums::BankNames::CentromarcaBanca),
        "mybank-chiantibanca-credito-cooperativo-it" => {
            Ok(common_enums::BankNames::ChiantibancaCreditoCooperativo)
        }
        "mybank-cortinabanca-it" => Ok(common_enums::BankNames::Cortinabanca),
        "mybank-cr-val-di-non-rotaliana-e-giovo-it" => {
            Ok(common_enums::BankNames::CrValDiNonRotalianaEGiovo)
        }
        "mybank-cra-bcc-di-cantu-it" => Ok(common_enums::BankNames::CraBccDiCantu),
        "mybank-cra-di-borgo-san-giacomo-it" => Ok(common_enums::BankNames::CraDiBorgoSanGiacomo),
        "mybank-cra-di-boves-it" => Ok(common_enums::BankNames::CraDiBoves),
        "mybank-cra-di-paliano-it" => Ok(common_enums::BankNames::CraDiPaliano),
        "xs2a-credem-it" => Ok(common_enums::BankNames::Credem),
        "mybank-credifriuli-it" => Ok(common_enums::BankNames::Credifriuli),
        "mybank-credito-cooperativo-agrigentino-it" => {
            Ok(common_enums::BankNames::CreditoCooperativoAgrigentino)
        }
        "mybank-credito-cooperativo-mediocrati-it" => {
            Ok(common_enums::BankNames::CreditoCooperativoMediocrati)
        }
        "mybank-credito-cooperativo-romagnolo-it" => {
            Ok(common_enums::BankNames::CreditoCooperativoRomagnolo)
        }
        "mybank-credito-di-romagna-it" => Ok(common_enums::BankNames::CreditoDiRomagna),
        "mybank-credito-lombardo-veneto-it" => Ok(common_enums::BankNames::CreditoLombardoVeneto),
        "mybank-desio-it" => Ok(common_enums::BankNames::Desio),
        "mybank-emilbanca-cc-it" => Ok(common_enums::BankNames::EmilbancaCc),
        "xs2a-fineco-it" => Ok(common_enums::BankNames::Fineco),
        "mybank-fpb-cassa-di-fassa-primiero-belluno-it" => {
            Ok(common_enums::BankNames::FpbCassaDiFassaPrimieroBelluno)
        }
        "xs2a-hype-it" => Ok(common_enums::BankNames::Hype),
        "mybank-iccrea-banca-spa-it" => Ok(common_enums::BankNames::IccreaBancaSpa),
        "xs2a-illimity-it" => Ok(common_enums::BankNames::Illimity),
        "mybank-imprebanca-spa-it" => Ok(common_enums::BankNames::ImprebancaSpa),
        "xs2a-intesa-sanpaolo-it" | "mybank-intesa-sanpaolo-it" => {
            Ok(common_enums::BankNames::IntesaSanpaolo)
        }
        "mybank-intesa-sanpaolo-inbiz-it" => Ok(common_enums::BankNames::IntesaSanpaoloInbiz),
        "mybank-intesa-sanpaolo-private-banking-spa-it" => {
            Ok(common_enums::BankNames::IntesaSanpaoloPrivateBankingSpa)
        }
        "xs2a-isybank-it" => Ok(common_enums::BankNames::Isybank),
        "mybank-la-cassa-di-ravenna-spa-it" => Ok(common_enums::BankNames::LaCassaDiRavennaSpa),
        "mybank-la-cassa-rurale-it" => Ok(common_enums::BankNames::LaCassaRurale),
        "mybank-lis-pay-spa-it" => Ok(common_enums::BankNames::LisPaySpa),
        "xs2a-mooney-it" => Ok(common_enums::BankNames::Mooney),
        "mybank-mps-it" => Ok(common_enums::BankNames::Mps),
        "xs2a-postepay-evolution-it" => Ok(common_enums::BankNames::PostePayEvolution),
        "mybank-primacassa-fvg-it" => Ok(common_enums::BankNames::PrimacassaFvg),
        "mybank-raiffeisen-algund-it" => Ok(common_enums::BankNames::RaiffeisenAlgund),
        "mybank-raiffeisen-alta-pusteria-it" => Ok(common_enums::BankNames::RaiffeisenAltaPusteria),
        "mybank-raiffeisen-alta-venosta-it" => Ok(common_enums::BankNames::RaiffeisenAltaVenosta),
        "mybank-raiffeisen-alto-adige-it" => Ok(common_enums::BankNames::RaiffeisenAltoAdige),
        "mybank-raiffeisen-bassa-atesina-it" => Ok(common_enums::BankNames::RaiffeisenBassaAtesina),
        "mybank-raiffeisen-bassa-valle-isarco-it" => {
            Ok(common_enums::BankNames::RaiffeisenBassaValleIsarco)
        }
        "mybank-raiffeisen-bassa-venosta-it" => Ok(common_enums::BankNames::RaiffeisenBassaVenosta),
        "mybank-raiffeisen-bolzano-it" => Ok(common_enums::BankNames::RaiffeisenBolzano),
        "mybank-raiffeisen-bozen-it" => Ok(common_enums::BankNames::RaiffeisenBozen),
        "mybank-raiffeisen-bruneck-it" => Ok(common_enums::BankNames::RaiffeisenBruneck),
        "mybank-raiffeisen-brunico-it" => Ok(common_enums::BankNames::RaiffeisenBrunico),
        "mybank-raiffeisen-campo-di-trens-it" => {
            Ok(common_enums::BankNames::RaiffeisenCampoDiTrens)
        }
        "mybank-raiffeisen-cassa-centr-alto-adige-it" => {
            Ok(common_enums::BankNames::RaiffeisenCassaCentrAltoAdige)
        }
        "mybank-raiffeisen-castelrottoortisei-it" => {
            Ok(common_enums::BankNames::RaiffeisenCastelrottoortisei)
        }
        "mybank-raiffeisen-deutschnofenaldein-it" => {
            Ok(common_enums::BankNames::RaiffeisenDeutschnofenaldein)
        }
        "mybank-raiffeisen-dobbiaco-it" => Ok(common_enums::BankNames::RaiffeisenDobbiaco),
        "mybank-raiffeisen-eisacktal-it" => Ok(common_enums::BankNames::RaiffeisenEisacktal),
        "mybank-raiffeisen-etschtal-it" => Ok(common_enums::BankNames::RaiffeisenEtschtal),
        "mybank-raiffeisen-freienfeld-it" => Ok(common_enums::BankNames::RaiffeisenFreienfeld),
        "mybank-raiffeisen-funes-it" => Ok(common_enums::BankNames::RaiffeisenFunes),
        "mybank-raiffeisen-gadertal-it" => Ok(common_enums::BankNames::RaiffeisenGadertal),
        "mybank-raiffeisen-groeden-it" => Ok(common_enums::BankNames::RaiffeisenGroeden),
        "mybank-raiffeisen-hochpustertal-it" => {
            Ok(common_enums::BankNames::RaiffeisenHochpustertal)
        }
        "mybank-raiffeisen-kastelruthstulrich-it" => {
            Ok(common_enums::BankNames::RaiffeisenKastelruthstulrich)
        }
        "mybank-raiffeisen-laas-it" => Ok(common_enums::BankNames::RaiffeisenLaas),
        "mybank-raiffeisen-laces-it" => Ok(common_enums::BankNames::RaiffeisenLaces),
        "mybank-raiffeisen-lagundo-it" => Ok(common_enums::BankNames::RaiffeisenLagundo),
        "mybank-raiffeisen-lana-it" => Ok(common_enums::BankNames::RaiffeisenLana),
        "mybank-raiffeisen-landesbank-suedtirol-it" => {
            Ok(common_enums::BankNames::RaiffeisenLandesbankSuedtirol)
        }
        "mybank-raiffeisen-lasa-it" => Ok(common_enums::BankNames::RaiffeisenLasa),
        "mybank-raiffeisen-latsch-it" => Ok(common_enums::BankNames::RaiffeisenLatsch),
        "mybank-raiffeisen-marlengo-it" => Ok(common_enums::BankNames::RaiffeisenMarlengo),
        "mybank-raiffeisen-marling-it" => Ok(common_enums::BankNames::RaiffeisenMarling),
        "mybank-raiffeisen-meran-it" => Ok(common_enums::BankNames::RaiffeisenMeran),
        "mybank-raiffeisen-merano-it" => Ok(common_enums::BankNames::RaiffeisenMerano),
        "mybank-raiffeisen-monguelfocasiestesido-it" => {
            Ok(common_enums::BankNames::RaiffeisenMonguelfocasiestesido)
        }
        "mybank-raiffeisen-niederdorf-it" => Ok(common_enums::BankNames::RaiffeisenNiederdorf),
        "mybank-raiffeisen-vintl-it" => Ok(common_enums::BankNames::RaiffeisenVintl),
        "mybank-raiffeisen-nova-levante-it" => Ok(common_enums::BankNames::RaiffeisenNovaLevante),
        "xs2a-rabobank" => Ok(common_enums::BankNames::Rabobank),
        "xs2a-knab-nl" => Ok(common_enums::BankNames::Knab),
        "mybank-raiffeisen-nova-ponentealdino-it" => {
            Ok(common_enums::BankNames::RaiffeisenNovaPonentealdino)
        }
        "mybank-raiffeisen-obervinschgau-it" => {
            Ok(common_enums::BankNames::RaiffeisenObervinschgau)
        }
        "mybank-raiffeisen-oltradige-it" => Ok(common_enums::BankNames::RaiffeisenOltradige),
        "mybank-raiffeisen-parcines-it" => Ok(common_enums::BankNames::RaiffeisenParcines),
        "mybank-raiffeisen-partschins-it" => Ok(common_enums::BankNames::RaiffeisenPartschins),
        "mybank-raiffeisen-passeier-it" => Ok(common_enums::BankNames::RaiffeisenPasseier),
        "mybank-raiffeisen-pradtaufers-it" => Ok(common_enums::BankNames::RaiffeisenPradtaufers),
        "mybank-raiffeisen-pratotubre-it" => Ok(common_enums::BankNames::RaiffeisenPratotubre),
        "mybank-raiffeisen-salorno-it" => Ok(common_enums::BankNames::RaiffeisenSalorno),
        "mybank-raiffeisen-salurn-it" => Ok(common_enums::BankNames::RaiffeisenSalurn),
        "mybank-raiffeisen-san-martino-in-passiria-it" => {
            Ok(common_enums::BankNames::RaiffeisenSanMartinoInPassiria)
        }
        "mybank-raiffeisen-sarntal-it" => Ok(common_enums::BankNames::RaiffeisenSarntal),
        "mybank-raiffeisen-scena-it" => Ok(common_enums::BankNames::RaiffeisenScena),
        "mybank-raiffeisen-schenna-it" => Ok(common_enums::BankNames::RaiffeisenSchenna),
        "mybank-raiffeisen-schlanders-it" => Ok(common_enums::BankNames::RaiffeisenSchlanders),
        "mybank-raiffeisen-schlernrosengarten-it" => {
            Ok(common_enums::BankNames::RaiffeisenSchlernrosengarten)
        }
        "mybank-raiffeisen-silandro-it" => Ok(common_enums::BankNames::RaiffeisenSilandro),

        "mybank-raiffeisen-suedtirol-it" => Ok(common_enums::BankNames::RaiffeisenSuedtirol),
        "mybank-raiffeisen-taufererahrntal-it" => {
            Ok(common_enums::BankNames::RaiffeisenTaufererahrntal)
        }
        "mybank-raiffeisen-tesimo-it" => Ok(common_enums::BankNames::RaiffeisenTesimo),
        "mybank-raiffeisen-tirol-it" => Ok(common_enums::BankNames::RaiffeisenTirol),
        "mybank-raiffeisen-tirolo-it" => Ok(common_enums::BankNames::RaiffeisenTirolo),
        "mybank-raiffeisen-tisens-it" => Ok(common_enums::BankNames::RaiffeisenTisens),
        "mybank-raiffeisen-toblach-it" => Ok(common_enums::BankNames::RaiffeisenToblach),
        "mybank-raiffeisen-turesaurina-it" => Ok(common_enums::BankNames::RaiffeisenTuresaurina),
        "mybank-raiffeisen-ueberetsch-it" => Ok(common_enums::BankNames::RaiffeisenUeberetsch),
        "mybank-raiffeisen-ultenstpankrazlaurein-it" => {
            Ok(common_enums::BankNames::RaiffeisenUltenstpankrazlaurein)
        }
        "mybank-raiffeisen-ultimospancrlaur-it" => {
            Ok(common_enums::BankNames::RaiffeisenUltimospancrlaur)
        }
        "mybank-raiffeisen-untereisacktal-it" => {
            Ok(common_enums::BankNames::RaiffeisenUntereisacktal)
        }
        "mybank-raiffeisen-unterland-it" => Ok(common_enums::BankNames::RaiffeisenUnterland),
        "mybank-raiffeisen-untervinschgau-it" => {
            Ok(common_enums::BankNames::RaiffeisenUntervinschgau)
        }
        "mybank-raiffeisen-val-badia-it" => Ok(common_enums::BankNames::RaiffeisenValBadia),
        "mybank-raiffeisen-val-gardena-it" => Ok(common_enums::BankNames::RaiffeisenValGardena),
        "mybank-raiffeisen-val-passiria-it" => Ok(common_enums::BankNames::RaiffeisenValPassiria),
        "mybank-raiffeisen-val-sarentino-it" => Ok(common_enums::BankNames::RaiffeisenValSarentino),
        "mybank-raiffeisen-valle-isarco-it" => Ok(common_enums::BankNames::RaiffeisenValleIsarco),
        "mybank-raiffeisen-vandoies-it" => Ok(common_enums::BankNames::RaiffeisenVandoies),
        "mybank-raiffeisen-villabassa-it" => Ok(common_enums::BankNames::RaiffeisenVillabassa),
        "mybank-raiffeisen-villnoess-it" => Ok(common_enums::BankNames::RaiffeisenVillnoess),
        "mybank-raiffeisen-welsberggsiestaisten-it" => {
            Ok(common_enums::BankNames::RaiffeisenWelsberggsiestaisten)
        }
        "mybank-raiffeisen-welschnofen-it" => Ok(common_enums::BankNames::RaiffeisenWelschnofen),
        "mybank-raiffeisen-wipptal-it" => Ok(common_enums::BankNames::RaiffeisenWipptal),
        "mybank-raiffeisenkasse-ritten-it" => Ok(common_enums::BankNames::RaiffeisenkasseRitten),
        "mybank-riviera-banca-it" => Ok(common_enums::BankNames::RivieraBanca),
        "mybank-romagna-banca-it" => Ok(common_enums::BankNames::RomagnaBanca),
        "mybank-sella-it" => Ok(common_enums::BankNames::Sella),
        "mybank-sicilbanca-it" => Ok(common_enums::BankNames::Sicilbanca),
        "mybank-solution-bank-it" => Ok(common_enums::BankNames::SolutionBank),
        "mybank-suedtiroler-it" => Ok(common_enums::BankNames::Suedtiroler),
        "mybank-suedtiroler-sparkasse-it" => Ok(common_enums::BankNames::SuedtirolerSparkasse),
        "mybank-suedtiroler-volksbank-it" => Ok(common_enums::BankNames::SuedtirolerVolksbank),
        "xs2a-unicredit-it" => Ok(common_enums::BankNames::Unicredit),
        "mybank-unicredit-online-banking-it" => Ok(common_enums::BankNames::UnicreditOnlineBanking),
        "mybank-unicredit-uniweb-corporate-it" => {
            Ok(common_enums::BankNames::UnicreditUniwebCorporate)
        }
        "mybank-valpolicella-benaco-banca-it" => {
            Ok(common_enums::BankNames::ValpolicellaBenacoBanca)
        }
        "mybank-volksbank-it" => Ok(common_enums::BankNames::Volksbank),
        "mybank-volksbank-banca-popolare-it" => Ok(common_enums::BankNames::VolksbankBancaPopolare),
        "xs2a-widiba-it" => Ok(common_enums::BankNames::Widiba),
        "mybank-zkb-credcoopdi-trieste-e-gorizia-it" => {
            Ok(common_enums::BankNames::ZkbCredcoopdiTriesteEGorizia)
        }
        "xs2a-abn-amro" => Ok(common_enums::BankNames::AbnAmro),
        "xs2a-asn" => Ok(common_enums::BankNames::Asn),
        "xs2a-regiobank" => Ok(common_enums::BankNames::Regiobank),
        "xs2a-sns" => Ok(common_enums::BankNames::Sns),
        "xs2a-seb-se" => Ok(common_enums::BankNames::Seb),
        "xs2a-swedbank-se" => Ok(common_enums::BankNames::Swedbank),
        "mock-payments-gb-redirect" => Ok(common_enums::BankNames::MockUkPayments),
        _ => Err(format!("Unknown provider_id: {provider_id}")),
    }
}

impl<F, T> TryFrom<ResponseRouterData<TruelayerPSyncResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<TruelayerPSyncResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            TruelayerPSyncResponseData::PSyncResponse(response) => {
                let status = get_attempt_status(response.status.clone());

                if is_payment_failure(status)
                    && response.failure_reason == Some("canceled".to_string())
                {
                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: AttemptStatus::Voided,
                            ..item.router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: Some(response.id),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                        ..item.router_data
                    })
                } else if is_payment_failure(status) {
                    let error_response = ErrorResponse {
                        code: response
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                        message: response
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                        reason: response.failure_reason.clone(),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(status)),
                        connector_transaction_id: Some(response.id),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    };

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            ..item.router_data.resource_common_data
                        },
                        response: Err(error_response),
                        ..item.router_data
                    })
                } else {
                    let account_holder_name = response
                        .payment_source
                        .as_ref()
                        .and_then(|s| s.account_holder_name.clone());

                    let mut sort_code: Option<Secret<String>> = None;
                    let mut account_number: Option<Secret<String>> = None;
                    let mut iban: Option<Secret<String>> = None;

                    if let Some(source) = response.payment_source.as_ref() {
                        for identifier in source.account_identifiers.iter().flatten() {
                            match identifier.identifier_type {
                                TruelayerAccountIdentifierType::SortCodeAccountNumber => {
                                    sort_code = identifier.sort_code.clone();
                                    account_number = identifier.account_number.clone();
                                }
                                TruelayerAccountIdentifierType::Iban => {
                                    iban = identifier.iban.clone();
                                }
                                TruelayerAccountIdentifierType::Unknown => {}
                            }
                        }
                    }

                    let provider_id = response
                        .payment_method
                        .as_ref()
                        .and_then(|pm| pm.provider_selection.as_ref())
                        .and_then(|ps| ps.provider_id.clone());

                    let bank_name = provider_id.as_ref().and_then(|pid| {
                        map_truelayer_provider_id_to_bank_name(pid)
                            .map_err(|error| {
                                tracing::warn!(
                                    %error,
                                    provider_id = %pid,
                                    "Failed to map TrueLayer provider_id to BankNames"
                                );
                            })
                            .ok()
                    });

                    let has_returned_open_banking_details = bank_name.is_some()
                        || (account_holder_name.is_some()
                            && ((account_number.is_some() && sort_code.is_some())
                                || iban.is_some()));

                    let additional_details = provider_id
                        .map(|pid| Secret::new(serde_json::json!({ "provider_id": pid })));

                    let connector_returned_payment_method_details =
                        if has_returned_open_banking_details {
                            Some(PaymentMethodData::<DefaultPCIHolder>::BankRedirect(
                                BankRedirectData::OpenBanking {
                                    bank_name,
                                    account_number,
                                    sort_code,
                                    iban,
                                    account_holder_name,
                                    additional_details,
                                },
                            ))
                        } else {
                            None
                        };

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            sender_payment_instrument_id: response
                                .payment_source
                                .and_then(|source| source.id),
                            connector_returned_payment_method_details,
                            ..item.router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: Some(response.id),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                        ..item.router_data
                    })
                }
            }
            TruelayerPSyncResponseData::WebhookResponse(response) => {
                let status =
                    get_truelayer_payment_webhook_status(response._type).map_err(|_| {
                        utils::response_handling_fail_for_connector(item.http_code, "truelayer")
                    })?;
                if is_payment_failure(status)
                    && response.failure_reason == Some("canceled".to_string())
                {
                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: AttemptStatus::Voided,
                            ..item.router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                response.payment_id.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: Some(response.payment_id.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                        ..item.router_data
                    })
                } else if is_payment_failure(status) {
                    let error_response = ErrorResponse {
                        code: response
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                        message: response
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                        reason: response.failure_reason.clone(),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(status)),
                        connector_transaction_id: Some(response.payment_id.clone()),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    };

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            ..item.router_data.resource_common_data
                        },
                        response: Err(error_response),
                        ..item.router_data
                    })
                } else {
                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            ..item.router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                response.payment_id.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: Some(response.payment_id.clone()),
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
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TruelayerRefundRequest {
    amount_in_minor: MinorUnit,
    reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerRefundResponse {
    id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TruelayerRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TruelayerRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: TruelayerRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let reference = item
            .router_data
            .request
            .connector_transaction_id
            .chars()
            .take(35)
            .collect::<String>();

        Ok(Self {
            amount_in_minor: item.router_data.request.minor_refund_amount,
            reference,
        })
    }
}

impl TryFrom<ResponseRouterData<TruelayerRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TruelayerRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.to_string(),
                refund_status: common_enums::RefundStatus::Pending,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TruelayerRefundStatus {
    Pending,
    Authorized,
    Executed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TruelayerRsyncResponse {
    RsyncResponse(TruelayerRsyncResponseData),
    WebhookResponse(TruelayerWebhookBody),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerRsyncResponseData {
    id: String,
    amount_in_minor: MinorUnit,
    currency: Currency,
    reference: String,
    status: TruelayerRefundStatus,
    created_at: Option<String>,
    failed_at: Option<String>,
    failure_reason: Option<String>,
}

impl TryFrom<ResponseRouterData<TruelayerRsyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<TruelayerRsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            TruelayerRsyncResponse::RsyncResponse(rsync_response) => {
                let status = get_refund_status(rsync_response.status.clone());

                let response = if utils::is_refund_failure(status) {
                    Err(ErrorResponse {
                        code: rsync_response
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                        message: rsync_response
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                        reason: rsync_response.failure_reason.clone(),
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: Some(rsync_response.id),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    })
                } else {
                    Ok(RefundsResponseData {
                        connector_refund_id: rsync_response.id,
                        refund_status: status,
                        status_code: item.http_code,
                        acquirer_reference_number: None,
                    })
                };

                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
            TruelayerRsyncResponse::WebhookResponse(webhook_response) => {
                let status =
                    get_truelayer_refund_webhook_status(webhook_response._type).map_err(|_| {
                        utils::response_handling_fail_for_connector(item.http_code, "truelayer")
                    })?;
                let response = if utils::is_refund_failure(status) {
                    Err(ErrorResponse {
                        code: webhook_response
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                        message: webhook_response
                            .failure_reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                        reason: webhook_response.failure_reason.clone(),
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: webhook_response.refund_id,
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    })
                } else {
                    Ok(RefundsResponseData {
                        connector_refund_id: webhook_response.refund_id.ok_or_else(|| {
                            error_stack::report!(
                                utils::unexpected_response_fail(
                                    item.http_code
                                , "truelayer: unexpected response for this operation; retry with idempotency keys and check connector status.")
                            )
                        })?,
                        refund_status: status,
                        status_code: item.http_code,
                        acquirer_reference_number: None,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerVoidResponseData {
    id: Option<String>,
}

impl TryFrom<ResponseRouterData<TruelayerVoidResponseData, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TruelayerVoidResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::VoidInitiated;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
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
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

fn get_address(billing: &domain_types::payment_address::Address) -> Option<Address> {
    billing.address.clone().and_then(|address| {
        match (
            address.line1.as_ref(),
            address.city.as_ref(),
            address.state.as_ref(),
            address.country.as_ref(),
        ) {
            (Some(line1), Some(city), Some(state), Some(&country)) => Some(Address {
                address_line1: line1.clone(),
                address_line2: address.line2.clone(),
                city: city.clone(),
                state: state.clone(),
                zip: address.zip.clone(),
                country_code: country,
            }),
            _ => None,
        }
    })
}

fn get_attempt_status(item: TruelayerPaymentStatus) -> AttemptStatus {
    match item {
        TruelayerPaymentStatus::Authorized | TruelayerPaymentStatus::Executed => {
            AttemptStatus::Authorized
        }
        TruelayerPaymentStatus::Settled => AttemptStatus::Charged,
        TruelayerPaymentStatus::AuthorizationRequired => AttemptStatus::AuthenticationPending,
        TruelayerPaymentStatus::Failed | TruelayerPaymentStatus::AttemptFailed => {
            AttemptStatus::Failure
        }
        TruelayerPaymentStatus::Authorizing => AttemptStatus::Pending,
    }
}

fn get_refund_status(item: TruelayerRefundStatus) -> common_enums::RefundStatus {
    match item {
        TruelayerRefundStatus::Pending | TruelayerRefundStatus::Authorized => {
            common_enums::RefundStatus::Pending
        }
        TruelayerRefundStatus::Executed => common_enums::RefundStatus::Success,
        TruelayerRefundStatus::Failed => common_enums::RefundStatus::Failure,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TruelayerWebhookEventType {
    PaymentAuthorized,
    PaymentFailed,
    PaymentSettled,
    PaymentExecuted,
    PaymentCreditable,
    PaymentSettlementStalled,
    RefundExecuted,
    RefundFailed,
    PaymentDisputed,
    PaymentReversed,
    PaymentFundsReceived,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerWebhookEventTypeBody {
    #[serde(rename = "type")]
    pub _type: TruelayerWebhookEventType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerWebhookBody {
    #[serde(rename = "type")]
    pub _type: TruelayerWebhookEventType,
    pub event_version: i32,
    pub event_id: String,
    pub payment_id: String,
    pub refund_id: Option<String>,
    pub failure_reason: Option<String>,
    pub failure_stage: Option<String>,
    pub user_id: Option<String>,
    pub payment_source: Option<TruelayerPaymentSource>,
}

/// Discriminator for the type of account identifier provided in a payment source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TruelayerAccountIdentifierType {
    SortCodeAccountNumber,
    Iban,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPaymentSource {
    pub id: Option<String>,
    pub account_holder_name: Option<Secret<String>>,
    pub account_identifiers: Option<Vec<TruelayerAccountIdentifier>>,
}

pub fn get_webhook_event(
    event: TruelayerWebhookEventType,
) -> domain_types::connector_types::EventType {
    match event {
        TruelayerWebhookEventType::PaymentExecuted
        | TruelayerWebhookEventType::PaymentAuthorized
        | TruelayerWebhookEventType::PaymentCreditable
        | TruelayerWebhookEventType::PaymentFundsReceived
        | TruelayerWebhookEventType::PaymentSettlementStalled => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        TruelayerWebhookEventType::PaymentSettled => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        TruelayerWebhookEventType::PaymentFailed => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        TruelayerWebhookEventType::RefundExecuted => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        TruelayerWebhookEventType::RefundFailed => {
            domain_types::connector_types::EventType::RefundFailure
        }
        TruelayerWebhookEventType::PaymentReversed => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        TruelayerWebhookEventType::PaymentDisputed | TruelayerWebhookEventType::Unknown => {
            domain_types::connector_types::EventType::IncomingWebhookEventUnspecified
        }
    }
}

pub fn get_truelayer_payment_webhook_status(
    event: TruelayerWebhookEventType,
) -> Result<AttemptStatus, WebhookError> {
    match event {
        TruelayerWebhookEventType::PaymentAuthorized => Ok(AttemptStatus::Authorized),
        TruelayerWebhookEventType::PaymentCreditable
        | TruelayerWebhookEventType::PaymentFundsReceived
        | TruelayerWebhookEventType::PaymentSettlementStalled
        | TruelayerWebhookEventType::PaymentExecuted => Ok(AttemptStatus::Pending),
        TruelayerWebhookEventType::PaymentSettled => Ok(AttemptStatus::Charged),
        TruelayerWebhookEventType::PaymentFailed => Ok(AttemptStatus::Failure),
        TruelayerWebhookEventType::PaymentReversed => Ok(AttemptStatus::Voided),
        TruelayerWebhookEventType::PaymentDisputed
        | TruelayerWebhookEventType::Unknown
        | TruelayerWebhookEventType::RefundExecuted
        | TruelayerWebhookEventType::RefundFailed => Err(WebhookError::WebhookBodyDecodingFailed),
    }
}

pub fn get_truelayer_refund_webhook_status(
    event: TruelayerWebhookEventType,
) -> Result<common_enums::RefundStatus, WebhookError> {
    match event {
        TruelayerWebhookEventType::RefundExecuted => Ok(common_enums::RefundStatus::Success),
        TruelayerWebhookEventType::RefundFailed => Ok(common_enums::RefundStatus::Failure),
        TruelayerWebhookEventType::PaymentAuthorized
        | TruelayerWebhookEventType::PaymentFailed
        | TruelayerWebhookEventType::PaymentSettled
        | TruelayerWebhookEventType::PaymentCreditable
        | TruelayerWebhookEventType::PaymentDisputed
        | TruelayerWebhookEventType::PaymentExecuted
        | TruelayerWebhookEventType::PaymentFundsReceived
        | TruelayerWebhookEventType::PaymentReversed
        | TruelayerWebhookEventType::PaymentSettlementStalled
        | TruelayerWebhookEventType::Unknown => Err(WebhookError::WebhookBodyDecodingFailed),
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JwsHeaderWebhooks {
    pub jku: Option<String>,
    kid: String,
    tl_headers: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
struct Jwk {
    kid: String,
    kty: String,
    x: Option<String>,
    y: Option<String>,
}

fn pad_to(bytes: Vec<u8>, target: usize) -> Result<Vec<u8>, IntegrationError> {
    match bytes.len().cmp(&target) {
        std::cmp::Ordering::Equal => Ok(bytes),
        std::cmp::Ordering::Less => {
            let mut padded = vec![0u8; target - bytes.len()];
            padded.extend(bytes);
            Ok(padded)
        }
        std::cmp::Ordering::Greater => Err(IntegrationError::NotImplemented(
            "webhook source verification failed".to_string(),
            Default::default(),
        )),
    }
}

pub const ALLOWED_JKUS: &[&str] = &[
    "https://webhooks.truelayer.com/.well-known/jwks",
    "https://webhooks.truelayer-sandbox.com/.well-known/jwks",
];

fn convert_p163_signature_to_der(
    signature_b64: &str,
) -> Result<Vec<u8>, error_stack::Report<IntegrationError>> {
    let sig_bytes =
        URL_SAFE_NO_PAD
            .decode(signature_b64)
            .change_context(IntegrationError::NotImplemented(
                "webhook decoding failed".to_string(),
                Default::default(),
            ))?;
    if sig_bytes.len() != SIG_BYTES_EXPECTED_LENGTH {
        return Err(IntegrationError::NotImplemented(
            "webhook decoding failed".to_string(),
            Default::default(),
        )
        .into());
    }

    let r = BigNum::from_slice(
        sig_bytes
            .get(0..66)
            .ok_or(IntegrationError::NotImplemented(
                "webhook decoding failed".to_string(),
                Default::default(),
            ))?,
    )
    .change_context(IntegrationError::NotImplemented(
        "webhook decoding failed".to_string(),
        Default::default(),
    ))?;
    let s = BigNum::from_slice(sig_bytes.get(66..).ok_or(IntegrationError::NotImplemented(
        "webhook decoding failed".to_string(),
        Default::default(),
    ))?)
    .change_context(IntegrationError::NotImplemented(
        "webhook decoding failed".to_string(),
        Default::default(),
    ))?;
    let der_sig = EcdsaSig::from_private_components(r, s)
        .change_context(IntegrationError::NotImplemented(
            "webhook decoding failed".to_string(),
            Default::default(),
        ))?
        .to_der()
        .change_context(IntegrationError::NotImplemented(
            "webhook decoding failed".to_string(),
            Default::default(),
        ))?;
    Ok(der_sig)
}

fn verify_ecdsa_signature_and_digest(
    der_sig: Vec<u8>,
    signing_input: &str,
    ec_key: EcKey<Public>,
) -> Result<bool, error_stack::Report<IntegrationError>> {
    let digest = hash(MessageDigest::sha512(), signing_input.as_bytes()).change_context(
        IntegrationError::NotImplemented("webhook decoding failed".to_string(), Default::default()),
    )?;

    let ecdsa_sig = EcdsaSig::from_der(&der_sig).change_context(
        IntegrationError::NotImplemented("webhook decoding failed".to_string(), Default::default()),
    )?;

    let valid =
        ecdsa_sig
            .verify(&digest, &ec_key)
            .change_context(IntegrationError::NotImplemented(
                "webhook decoding failed".to_string(),
                Default::default(),
            ))?;

    Ok(valid)
}

fn build_uncompressed_ec1_point(
    x: Vec<u8>,
    y: Vec<u8>,
) -> Result<EcKey<Public>, error_stack::Report<IntegrationError>> {
    let mut sec1 = vec![0x04u8];
    sec1.extend(pad_to(x, P521_COORDINATE_BYTE_LEN)?);
    sec1.extend(pad_to(y, P521_COORDINATE_BYTE_LEN)?);

    let group = EcGroup::from_curve_name(Nid::SECP521R1).change_context(
        IntegrationError::NotImplemented("webhook decoding failed".to_string(), Default::default()),
    )?;
    let mut ctx = BigNumContext::new().change_context(IntegrationError::NotImplemented(
        "webhook decoding failed".to_string(),
        Default::default(),
    ))?;
    let point = EcPoint::from_bytes(&group, &sec1, &mut ctx).change_context(
        IntegrationError::NotImplemented("webhook decoding failed".to_string(), Default::default()),
    )?;
    let ec_key = EcKey::from_public_key(&group, &point).change_context(
        IntegrationError::NotImplemented("webhook decoding failed".to_string(), Default::default()),
    )?;
    ec_key
        .check_key()
        .change_context(IntegrationError::NotImplemented(
            "webhook decoding failed".to_string(),
            Default::default(),
        ))?;
    Ok(ec_key)
}

fn verify_signature(
    body: &[u8],
    jws_header: JwsHeaderWebhooks,
    header_b64: &str,
    signature_b64: &str,
    headers: &HashMap<String, String>,
    ec_key: EcKey<Public>,
    webhook_uri: &str,
) -> Result<bool, error_stack::Report<IntegrationError>> {
    let tl_headers_str = jws_header.tl_headers.unwrap_or_default();
    let mut payload: Vec<u8> = format!("{} {}\n", "POST".to_uppercase(), webhook_uri).into_bytes();

    if !tl_headers_str.is_empty() {
        let lower_headers: HashMap<String, &String> =
            headers.iter().map(|(k, v)| (k.to_lowercase(), v)).collect();
        for header_name in tl_headers_str.split(',') {
            let name = header_name.trim();
            let value =
                lower_headers
                    .get(&name.to_lowercase())
                    .ok_or(IntegrationError::NotImplemented(
                        "webhook decoding failed".to_string(),
                        Default::default(),
                    ))?;
            payload.extend_from_slice(format!("{}: {}\n", name, value).as_bytes());
        }
    }
    payload.extend_from_slice(body);

    // signing_input = base64url(header) + "." + base64url(payload)
    let signing_input = format!("{}.{}", header_b64, URL_SAFE_NO_PAD.encode(&payload));

    // Convert P1363 signature (r || s, 66 bytes each) to DER
    let der_sig = convert_p163_signature_to_der(signature_b64)?;

    // SHA-512 digest + ECDSA verify
    let valid = verify_ecdsa_signature_and_digest(der_sig, &signing_input, ec_key)?;

    Ok(valid)
}

impl TryFrom<ResponseRouterData<Jwks, Self>>
    for RouterDataV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    >
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: ResponseRouterData<Jwks, Self>) -> Result<Self, Self::Error> {
        let body = item.router_data.request.webhook_body.as_ref();
        let headers = item.router_data.request.webhook_headers.clone();

        let tl_signature_header =
            headers
                .get("tl-signature")
                .ok_or(IntegrationError::NotImplemented(
                    "webhook signature not found".to_string(),
                    Default::default(),
                ))?;
        let tl_signature = tl_signature_header.as_str();
        let parts: Vec<&str> = tl_signature.splitn(3, '.').collect();

        let header_b64 = parts.first().ok_or(IntegrationError::NotImplemented(
            "webhook decoding failed".to_string(),
            Default::default(),
        ))?;
        let signature_b64 = parts.get(2).ok_or(IntegrationError::NotImplemented(
            "webhook decoding failed".to_string(),
            Default::default(),
        ))?;

        let header_json =
            URL_SAFE_NO_PAD
                .decode(header_b64)
                .change_context(IntegrationError::NotImplemented(
                    "webhook decoding failed".to_string(),
                    Default::default(),
                ))?;
        let jws_header: JwsHeaderWebhooks = serde_json::from_slice(&header_json).change_context(
            IntegrationError::NotImplemented(
                "webhook decoding failed".to_string(),
                Default::default(),
            ),
        )?;

        let jwk = item
            .response
            .keys
            .into_iter()
            .find(|k| k.kid == jws_header.kid && k.kty == "EC")
            .ok_or(IntegrationError::NotImplemented(
                "webhook source verification failed".to_string(),
                Default::default(),
            ))?;

        let x_raw = URL_SAFE_NO_PAD
            .decode(jwk.x.ok_or(IntegrationError::NotImplemented(
                "webhook decoding failed".to_string(),
                Default::default(),
            ))?)
            .change_context(IntegrationError::NotImplemented(
                "webhook decoding failed".to_string(),
                Default::default(),
            ))?;
        let y_raw = URL_SAFE_NO_PAD
            .decode(jwk.y.ok_or(IntegrationError::NotImplemented(
                "webhook decoding failed".to_string(),
                Default::default(),
            ))?)
            .change_context(IntegrationError::NotImplemented(
                "webhook decoding failed".to_string(),
                Default::default(),
            ))?;

        let ec_key = build_uncompressed_ec1_point(x_raw, y_raw)?;

        let webhook_uri = item.router_data.request.webhook_uri.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "webhook_uri",
                context: Default::default(),
            },
        )?;

        let valid = verify_signature(
            body,
            jws_header.clone(),
            header_b64,
            signature_b64,
            &headers,
            ec_key.clone(),
            &(PREFIX.to_owned() + &webhook_uri),
        )? || verify_signature(
            body,
            jws_header.clone(),
            header_b64,
            signature_b64,
            &headers,
            ec_key.clone(),
            &webhook_uri,
        )?;

        Ok(Self {
            response: Ok(VerifyWebhookSourceResponseData {
                verify_webhook_status: if valid {
                    VerifyWebhookStatus::SourceVerified
                } else {
                    VerifyWebhookStatus::SourceNotVerified
                },
            }),
            ..item.router_data
        })
    }
}
