use std::collections::BTreeMap;

use common_enums::{Currency, PayoutStatus};
use common_utils::errors::CustomResult;
use common_utils::types::MinorUnit;
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer, ServerAuthenticationToken},
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payouts::{
        payout_method_data::{BankRedirect, PayoutMethodData},
        payouts_types::{
            PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
            PayoutTransferResponse,
        },
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use josekit::jws::{JwsHeader, ES512};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

const GRANT_TYPE: &str = "client_credentials";
const SCOPE: &str = "payments";

pub struct TruelayerAuthType {
    client_id: Secret<String>,
    client_secret: Secret<String>,
    merchant_account_id: Option<Secret<String>>,
    private_key: Option<Secret<String>>,
    kid: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for TruelayerAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Truelayer {
                client_id,
                client_secret,
                merchant_account_id,
                private_key,
                kid,
                ..
            } => Ok(Self {
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                merchant_account_id: merchant_account_id.clone(),
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

pub struct TruelayerPayoutMetadata {
    pub merchant_account_id: Secret<String>,
    pub private_key: Secret<String>,
    pub kid: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TruelayerPayoutMetadata {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        let auth = TruelayerAuthType::try_from(config)?;
        Ok(Self {
            merchant_account_id: auth.merchant_account_id.ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "merchant_account_id",
                    context: Default::default(),
                },
            )?,
            private_key: auth
                .private_key
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "private_key",
                    context: Default::default(),
                })?,
            kid: auth.kid.ok_or(IntegrationError::MissingRequiredField {
                field_name: "kid",
                context: Default::default(),
            })?,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TruelayerServerAuthenticationTokenRequest {
    grant_type: String,
    client_id: Secret<String>,
    client_secret: Secret<String>,
    scope: String,
}

impl
    TryFrom<
        &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    > for TruelayerServerAuthenticationTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TruelayerAuthType::try_from(&req.connector_config)?;
        Ok(Self {
            grant_type: GRANT_TYPE.to_string(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
            scope: SCOPE.to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerServerAuthenticationTokenResponse {
    access_token: Secret<String>,
    expires_in: i64,
    token_type: Option<String>,
}

impl TryFrom<ResponseRouterData<TruelayerServerAuthenticationTokenResponse, Self>>
    for RouterDataV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TruelayerServerAuthenticationTokenResponse, Self>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerAccessTokenErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
    pub error_details: Option<TruelayerErrorDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerErrorDetails {
    pub reason: Option<String>,
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

#[derive(Debug, Serialize, PartialEq)]
pub struct TruelayerPayoutRequest {
    merchant_account_id: Secret<String>,
    amount_in_minor: MinorUnit,
    currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<TruelayerPayoutRequestMetadata>,
    beneficiary: TruelayerBeneficiary,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct TruelayerPayoutRequestMetadata {
    reference_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TruelayerBeneficiary {
    #[serde(rename = "type")]
    _type: TruelayerBeneficiaryType,
    reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_holder_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_identifier: Option<TruelayerAccountIdentifier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payment_source_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TruelayerBeneficiaryType {
    ExternalAccount,
    PaymentSource,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TruelayerAccountIdentifier {
    #[serde(rename = "type")]
    _type: String,
    iban: Secret<String>,
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for TruelayerPayoutRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let reference = normalize_connector_request_reference_id(
            &req.resource_common_data.connector_request_reference_id,
        );

        let metadata = TruelayerPayoutMetadata::try_from(&req.connector_config)?;
        let request_metadata = Some(TruelayerPayoutRequestMetadata {
            reference_id: Some(
                req.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        });

        let beneficiary = match req.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::BankRedirect(BankRedirect::OpenBankingUk(open_banking))) => {
                TruelayerBeneficiary {
                    _type: TruelayerBeneficiaryType::ExternalAccount,
                    reference: reference.clone(),
                    account_holder_name: Some(open_banking.account_holder_name.clone()),
                    account_identifier: Some(TruelayerAccountIdentifier {
                        _type: "iban".to_string(),
                        iban: open_banking.iban.clone(),
                    }),
                    payment_source_id: None,
                    user_id: None,
                }
            }
            Some(PayoutMethodData::Passthrough(passthrough)) => {
                let user_id = req
                    .request
                    .customer
                    .as_ref()
                    .and_then(|customer| customer.connector_customer_id.clone())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "customer.connector_customer_id",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "TrueLayer closed-loop payouts require the TrueLayer user id"
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })?;

                TruelayerBeneficiary {
                    _type: TruelayerBeneficiaryType::PaymentSource,
                    reference: reference.clone(),
                    account_holder_name: None,
                    account_identifier: None,
                    payment_source_id: Some(passthrough.psp_token.clone()),
                    user_id: Some(user_id),
                }
            }
            Some(_) | None => {
                return Err(IntegrationError::connector_feature_not_supported(
                    "truelayer",
                    "selected payout method",
                    Default::default(),
                )
                .into());
            }
        };

        Ok(Self {
            merchant_account_id: metadata.merchant_account_id,
            amount_in_minor: req.request.amount,
            currency: req.request.destination_currency,
            metadata: request_metadata,
            beneficiary,
        })
    }
}

fn normalize_connector_request_reference_id(reference_id: &str) -> String {
    reference_id
        .chars()
        .map(|character| if character == '_' { '-' } else { character })
        .collect()
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPayoutResponse {
    id: String,
}

impl TryFrom<ResponseRouterData<TruelayerPayoutResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TruelayerPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: PayoutStatus::Initiated,
                connector_payout_id: Some(item.response.id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TruelayerPayoutSyncType {
    Sync(TruelayerPayoutSyncResponse),
    Webhook(TruelayerPayoutsWebhookBody),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPayoutSyncResponse {
    id: String,
    merchant_account_id: Secret<String>,
    amount_in_minor: MinorUnit,
    currency: Currency,
    beneficiary: TruelayerBeneficiary,
    scheme_id: Option<String>,
    status: TruelayerPayoutStatus,
    failed_at: Option<String>,
    failure_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TruelayerPayoutStatus {
    Pending,
    AuthorizationRequired,
    Failed,
    Authorizing,
    Authorized,
    Executed,
}

impl From<TruelayerPayoutStatus> for PayoutStatus {
    fn from(status: TruelayerPayoutStatus) -> Self {
        match status {
            TruelayerPayoutStatus::Pending
            | TruelayerPayoutStatus::AuthorizationRequired
            | TruelayerPayoutStatus::Authorizing => Self::Pending,
            TruelayerPayoutStatus::Authorized | TruelayerPayoutStatus::Executed => Self::Success,
            TruelayerPayoutStatus::Failed => Self::Failure,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TruelayerPayoutsWebhookEvent {
    PayoutExecuted,
    PayoutFailed,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPayoutsWebhookBody {
    #[serde(rename = "type")]
    pub _type: TruelayerPayoutsWebhookEvent,
    pub event_version: i32,
    pub event_id: String,
    pub payout_id: String,
    pub executed_at: Option<String>,
    pub failed_at: Option<String>,
    pub failure_reason: Option<String>,
    pub scheme_id: Option<String>,
}

impl TryFrom<ResponseRouterData<TruelayerPayoutSyncType, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TruelayerPayoutSyncType, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            TruelayerPayoutSyncType::Sync(sync_response) => {
                let status = PayoutStatus::from(sync_response.status);
                build_payout_get_router_data(
                    item.router_data,
                    item.http_code,
                    sync_response.id,
                    status,
                    sync_response.failure_reason,
                    None,
                )
            }
            TruelayerPayoutSyncType::Webhook(webhook_response) => {
                let status = match webhook_response._type {
                    TruelayerPayoutsWebhookEvent::PayoutExecuted => PayoutStatus::Success,
                    TruelayerPayoutsWebhookEvent::PayoutFailed => PayoutStatus::Failure,
                };
                build_payout_get_router_data(
                    item.router_data,
                    item.http_code,
                    webhook_response.payout_id,
                    status,
                    webhook_response.failure_reason,
                    Some(webhook_response.event_id),
                )
            }
        }
    }
}

fn build_payout_get_router_data(
    router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    http_code: u16,
    connector_payout_id: String,
    status: PayoutStatus,
    failure_reason: Option<String>,
    connector_response_reference_id: Option<String>,
) -> Result<
    RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    error_stack::Report<ConnectorError>,
> {
    let response = if status == PayoutStatus::Failure {
        let reason =
            failure_reason.unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());
        Err(ErrorResponse {
            code: reason.clone(),
            message: reason.clone(),
            reason: Some(reason),
            attempt_status: None,
            connector_transaction_id: Some(connector_payout_id),
            status_code: http_code,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    } else {
        Ok(PayoutGetResponse {
            merchant_payout_id: router_data.request.merchant_payout_id.clone(),
            payout_status: status,
            connector_payout_id: Some(connector_payout_id),
            status_code: http_code,
        })
    };

    let _ = connector_response_reference_id;

    Ok(RouterDataV2 {
        response,
        ..router_data
    })
}

pub fn generate_tl_signature(
    method: String,
    path: &str,
    headers: &BTreeMap<String, String>,
    body: Option<&str>,
    private_key: String,
    kid: &str,
) -> CustomResult<String, IntegrationError> {
    let payload = build_payload(method, path, headers, body);
    let pem = domain_types::utils::base64_decode(private_key).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?;

    let signer =
        ES512
            .signer_from_pem(&pem)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

    let tl_headers = headers.keys().cloned().collect::<Vec<_>>().join(",");

    let mut header = JwsHeader::new();
    header.set_algorithm("ES512");
    header.set_key_id(kid);
    header
        .set_claim("tl_version", Some("2".into()))
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;
    header
        .set_claim("tl_headers", Some(tl_headers.into()))
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;

    let jws = josekit::jws::serialize_compact(payload.as_bytes(), &header, &signer)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;

    let parts: Vec<&str> = jws.split('.').collect();

    match (parts.first(), parts.get(2)) {
        (Some(first), Some(third)) => Ok(format!("{first}..{third}")),
        _ => Err(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
        .into()),
    }
}

fn build_payload(
    method: String,
    path: &str,
    headers: &BTreeMap<String, String>,
    body: Option<&str>,
) -> String {
    let mut payload = format!("{} {}\n", method.to_uppercase(), path.trim_end_matches('/'));

    for (key, value) in headers {
        payload.push_str(&format!("{key}: {value}\n"));
    }

    if let Some(body) = body {
        payload.push_str(body);
    }

    payload
}
