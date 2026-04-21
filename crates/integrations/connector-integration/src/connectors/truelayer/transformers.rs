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
    payment_method_data::{BankRedirectData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::RedirectForm,
    router_response_types::{VerifyWebhookSourceResponseData, VerifyWebhookStatus},
    utils::is_payment_failure,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use openssl::{
    bn::{BigNum, BigNumContext},
    ec::{EcGroup, EcKey, EcPoint},
    ecdsa::EcdsaSig,
    hash::{hash, MessageDigest},
    nid::Nid,
    pkey::Public,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{connectors::truelayer::TruelayerRouterData, types::ResponseRouterData, utils};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};
const GRANT_TYPE: &str = "client_credentials";
const SCOPE: &str = "payments";
const SIG_BYTES_EXPECTED_LENGTH: usize = 132;
const P521_COORDINATE_BYTE_LEN: usize = 66;
const PREFIX: &str = "/api";

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
                PaymentFlowData,
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
                PaymentFlowData,
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
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
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

#[derive(Debug, Clone, Serialize, PartialEq)]
struct ProviderSelection {
    #[serde(rename = "type")]
    _type: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct Beneficiary {
    #[serde(rename = "type")]
    _type: String,
    merchant_account_id: Secret<String>,
    account_holder_name: Secret<String>,
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
            PaymentMethodData::BankRedirect(BankRedirectData::OpenBanking { .. }) => {
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

                let payment_method = PaymentMethod {
                    _type: "bank_transfer".to_string(),
                    provider_selection: ProviderSelection {
                        _type: "user_selected".to_string(),
                    },
                    beneficiary: Beneficiary {
                        _type: "merchant_account".to_string(),
                        merchant_account_id: metadata.merchant_account_id.clone(),
                        account_holder_name: metadata.account_holder_name.clone(),
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
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.phone",
                        context: Default::default(),
                    })?;

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
                (utils::get_unimplemented_payment_method_error_message("Truelayer")).into(),
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
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.id),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
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
                    connector_customer: Some(item.response.user.id.clone()),
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                    redirection_data: redirection_data.map(Box::new),
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(item.response.id),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
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
                            connector_response_reference_id: Some(response.id),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
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
                        attempt_status: Some(status),
                        connector_transaction_id: Some(response.id),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
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
                            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: Some(response.id),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
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
                            connector_response_reference_id: Some(response.payment_id.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
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
                        attempt_status: Some(status),
                        connector_transaction_id: Some(response.payment_id.clone()),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
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
                            connector_response_reference_id: Some(response.payment_id.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
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
                    })
                } else {
                    Ok(RefundsResponseData {
                        connector_refund_id: rsync_response.id,
                        refund_status: status,
                        status_code: item.http_code,
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
            ("webhook source verification failed".to_string()).into(),
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
                ("webhook decoding failed".to_string()).into(),
                Default::default(),
            ))?;
    if sig_bytes.len() != SIG_BYTES_EXPECTED_LENGTH {
        return Err(IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        )
        .into());
    }

    let r = BigNum::from_slice(
        sig_bytes
            .get(0..66)
            .ok_or(IntegrationError::NotImplemented(
                ("webhook decoding failed".to_string()).into(),
                Default::default(),
            ))?,
    )
    .change_context(IntegrationError::NotImplemented(
        ("webhook decoding failed".to_string()).into(),
        Default::default(),
    ))?;
    let s = BigNum::from_slice(sig_bytes.get(66..).ok_or(IntegrationError::NotImplemented(
        ("webhook decoding failed".to_string()).into(),
        Default::default(),
    ))?)
    .change_context(IntegrationError::NotImplemented(
        ("webhook decoding failed".to_string()).into(),
        Default::default(),
    ))?;
    let der_sig = EcdsaSig::from_private_components(r, s)
        .change_context(IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        ))?
        .to_der()
        .change_context(IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
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
        IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        ),
    )?;

    let ecdsa_sig =
        EcdsaSig::from_der(&der_sig).change_context(IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        ))?;

    let valid =
        ecdsa_sig
            .verify(&digest, &ec_key)
            .change_context(IntegrationError::NotImplemented(
                ("webhook decoding failed".to_string()).into(),
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
        IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        ),
    )?;
    let mut ctx = BigNumContext::new().change_context(IntegrationError::NotImplemented(
        ("webhook decoding failed".to_string()).into(),
        Default::default(),
    ))?;
    let point = EcPoint::from_bytes(&group, &sec1, &mut ctx).change_context(
        IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        ),
    )?;
    let ec_key =
        EcKey::from_public_key(&group, &point).change_context(IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        ))?;
    ec_key
        .check_key()
        .change_context(IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
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
                        ("webhook decoding failed".to_string()).into(),
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
                    ("webhook signature not found".to_string()).into(),
                    Default::default(),
                ))?;
        let tl_signature = tl_signature_header.as_str();
        let parts: Vec<&str> = tl_signature.splitn(3, '.').collect();

        let header_b64 = parts.first().ok_or(IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        ))?;
        let signature_b64 = parts.get(2).ok_or(IntegrationError::NotImplemented(
            ("webhook decoding failed".to_string()).into(),
            Default::default(),
        ))?;

        let header_json =
            URL_SAFE_NO_PAD
                .decode(header_b64)
                .change_context(IntegrationError::NotImplemented(
                    ("webhook decoding failed".to_string()).into(),
                    Default::default(),
                ))?;
        let jws_header: JwsHeaderWebhooks = serde_json::from_slice(&header_json).change_context(
            IntegrationError::NotImplemented(
                ("webhook decoding failed".to_string()).into(),
                Default::default(),
            ),
        )?;

        let jwk = item
            .response
            .keys
            .into_iter()
            .find(|k| k.kid == jws_header.kid && k.kty == "EC")
            .ok_or(IntegrationError::NotImplemented(
                ("webhook source verification failed".to_string()).into(),
                Default::default(),
            ))?;

        let x_raw = URL_SAFE_NO_PAD
            .decode(jwk.x.ok_or(IntegrationError::NotImplemented(
                ("webhook decoding failed".to_string()).into(),
                Default::default(),
            ))?)
            .change_context(IntegrationError::NotImplemented(
                ("webhook decoding failed".to_string()).into(),
                Default::default(),
            ))?;
        let y_raw = URL_SAFE_NO_PAD
            .decode(jwk.y.ok_or(IntegrationError::NotImplemented(
                ("webhook decoding failed".to_string()).into(),
                Default::default(),
            ))?)
            .change_context(IntegrationError::NotImplemented(
                ("webhook decoding failed".to_string()).into(),
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
