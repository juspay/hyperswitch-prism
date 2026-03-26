use common_enums::{enums, Currency};
use common_utils::{pii::Email, types::StringMajorUnit};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId,
    },
    errors::ConnectorError,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use time::Date;

use super::MifinityRouterData;
use crate::{types::ResponseRouterData, utils};
pub mod auth_headers {
    pub const API_VERSION: &str = "api-version";
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MifinityConnectorMetadataObject {
    pub brand_id: Secret<String>,
    pub destination_account_number: Secret<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityPaymentsRequest {
    money: Money,
    client: MifinityClient,
    address: MifinityAddress,
    validation_key: String,
    client_reference: common_utils::id_type::CustomerId,
    trace_id: String,
    description: String,
    destination_account_number: Secret<String>,
    brand_id: Secret<String>,
    return_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_preference: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Money {
    amount: StringMajorUnit,
    currency: Currency,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityClient {
    first_name: Secret<String>,
    last_name: Secret<String>,
    phone: Secret<String>,
    dialing_code: String,
    nationality: enums::CountryAlpha2,
    email_address: Email,
    dob: Secret<Date>,
}

#[derive(Default, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityAddress {
    address_line1: Secret<String>,
    country_code: enums::CountryAlpha2,
    city: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MifinityRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MifinityPaymentsRequest
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: MifinityRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = MifinityAuthType::try_from(&item.router_data.connector_config)?;
        let metadata = MifinityConnectorMetadataObject {
            brand_id: auth
                .brand_id
                .ok_or(ConnectorError::InvalidConnectorConfig { config: "brand_id" })?,
            destination_account_number: auth.destination_account_number.ok_or(
                ConnectorError::InvalidConnectorConfig {
                    config: "destination_account_number",
                },
            )?,
        };
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::Mifinity(data) => {
                    let money = Money {
                        amount: item
                            .connector
                            .amount_converter
                            .convert(
                                item.router_data.request.minor_amount,
                                item.router_data.request.currency,
                            )
                            .change_context(ConnectorError::RequestEncodingFailed)?,
                        currency: item.router_data.request.currency,
                    };
                    let phone_details =
                        item.router_data.resource_common_data.get_billing_phone()?;
                    let billing_country = item
                        .router_data
                        .resource_common_data
                        .get_billing_country()?;
                    let client = MifinityClient {
                        first_name: item
                            .router_data
                            .resource_common_data
                            .get_billing_first_name()?,
                        last_name: item
                            .router_data
                            .resource_common_data
                            .get_billing_last_name()?,
                        phone: phone_details.get_number()?,
                        dialing_code: phone_details.get_country_code()?,
                        nationality: billing_country,
                        email_address: item.router_data.resource_common_data.get_billing_email()?,
                        dob: data.date_of_birth.clone(),
                    };
                    let address = MifinityAddress {
                        address_line1: item.router_data.resource_common_data.get_billing_line1()?,
                        country_code: billing_country,
                        city: item.router_data.resource_common_data.get_billing_city()?,
                    };
                    let validation_key = format!(
                        "payment_validation_key_{}_{}",
                        item.router_data
                            .resource_common_data
                            .merchant_id
                            .get_string_repr(),
                        item.router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone()
                    );
                    let client_reference = item.router_data.request.customer_id.clone().ok_or(
                        ConnectorError::MissingRequiredField {
                            field_name: "client_reference",
                        },
                    )?;
                    let destination_account_number = metadata.destination_account_number;
                    let trace_id = item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone();
                    let brand_id = metadata.brand_id;
                    let language_preference = data.language_preference;
                    Ok(Self {
                        money,
                        client,
                        address,
                        validation_key,
                        client_reference,
                        trace_id: trace_id.clone(),
                        description: trace_id.clone(), //Connector recommend to use the traceId for a better experience in the BackOffice application later.
                        destination_account_number,
                        brand_id,
                        return_url: item.router_data.request.get_router_return_url()?,
                        language_preference,
                    })
                }
                WalletData::AliPayQr(_)
                | WalletData::BluecodeRedirect {}
                | WalletData::AliPayRedirect(_)
                | WalletData::AliPayHkRedirect(_)
                | WalletData::AmazonPayRedirect(_)
                | WalletData::MomoRedirect(_)
                | WalletData::KakaoPayRedirect(_)
                | WalletData::GoPayRedirect(_)
                | WalletData::GcashRedirect(_)
                | WalletData::ApplePay(_)
                | WalletData::ApplePayRedirect(_)
                | WalletData::ApplePayThirdPartySdk(_)
                | WalletData::DanaRedirect {}
                | WalletData::GooglePay(_)
                | WalletData::GooglePayRedirect(_)
                | WalletData::GooglePayThirdPartySdk(_)
                | WalletData::MbWayRedirect(_)
                | WalletData::MobilePayRedirect(_)
                | WalletData::PaypalRedirect(_)
                | WalletData::PaypalSdk(_)
                | WalletData::Paze(_)
                | WalletData::SamsungPay(_)
                | WalletData::TwintRedirect {}
                | WalletData::VippsRedirect {}
                | WalletData::TouchNGoRedirect(_)
                | WalletData::WeChatPayRedirect(_)
                | WalletData::WeChatPayQr(_)
                | WalletData::CashappQr(_)
                | WalletData::SwishQr(_)
                | WalletData::RevolutPay(_)
                | WalletData::MbWay(_)
                | WalletData::Satispay(_)
                | WalletData::Wero(_) => Err(ConnectorError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Mifinity"),
                )
                .into()),
            },
            PaymentMethodData::Card(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::CardToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(ConnectorError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Mifinity"),
                )
                .into())
            }
        }
    }
}

// Auth Struct
pub struct MifinityAuthType {
    pub(super) key: Secret<String>,
    pub(super) brand_id: Option<Secret<String>>,
    pub(super) destination_account_number: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for MifinityAuthType {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Mifinity {
                key,
                brand_id,
                destination_account_number,
                ..
            } => Ok(Self {
                key: key.to_owned(),
                brand_id: brand_id.clone(),
                destination_account_number: destination_account_number.clone(),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MifinityPaymentsResponse {
    payload: Vec<MifinityPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MifinityPayload {
    trace_id: String,
    initialization_token: Secret<String>,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MifinityPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MifinityPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payload = item.response.payload.first();
        match payload {
            Some(payload) => {
                let trace_id = payload.trace_id.clone();
                let initialization_token = payload.initialization_token.clone();
                Ok(Self {
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(trace_id.clone()),
                        redirection_data: Some(Box::new(RedirectForm::Mifinity {
                            initialization_token: initialization_token.expose(),
                        })),
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: Some(trace_id),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: enums::AttemptStatus::AuthenticationPending,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            None => Ok(Self {
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
                    status: enums::AttemptStatus::AuthenticationPending,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MifinityPsyncResponse {
    payload: Vec<MifinityPsyncPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MifinityPsyncPayload {
    status: MifinityPaymentStatus,
    payment_response: Option<PaymentResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentResponse {
    trace_id: Option<String>,
    client_reference: Option<String>,
    validation_key: Option<String>,
    transaction_reference: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MifinityPaymentStatus {
    Successful,
    Pending,
    Failed,
    NotCompleted,
}

impl<F> TryFrom<ResponseRouterData<MifinityPsyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<MifinityPsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payload = item.response.payload.first();

        match payload {
            Some(payload) => {
                let status = payload.status.clone();
                let payment_response = payload.payment_response.clone();

                match payment_response {
                    Some(payment_response) => {
                        let transaction_reference = payment_response.transaction_reference.clone();

                        Ok(Self {
                            response: Ok(PaymentsResponseData::TransactionResponse {
                                resource_id: ResponseId::ConnectorTransactionId(
                                    transaction_reference,
                                ),
                                redirection_data: None,
                                mandate_reference: None,
                                connector_metadata: None,
                                network_txn_id: None,
                                connector_response_reference_id: None,
                                incremental_authorization_allowed: None,
                                status_code: item.http_code,
                            }),
                            resource_common_data: PaymentFlowData {
                                status: enums::AttemptStatus::from(status),
                                ..item.router_data.resource_common_data
                            },
                            ..item.router_data
                        })
                    }
                    None => Ok(Self {
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
                            status: enums::AttemptStatus::from(status),
                            ..item.router_data.resource_common_data
                        },
                        ..item.router_data
                    }),
                }
            }
            None => Ok(Self {
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
                    status: enums::AttemptStatus::Unspecified,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
        }
    }
}

impl From<MifinityPaymentStatus> for enums::AttemptStatus {
    fn from(item: MifinityPaymentStatus) -> Self {
        match item {
            MifinityPaymentStatus::Successful => Self::Charged,
            MifinityPaymentStatus::Failed => Self::Failure,
            MifinityPaymentStatus::NotCompleted => Self::AuthenticationPending,
            MifinityPaymentStatus::Pending => Self::Pending,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct MifinityErrorResponse {
    pub errors: Vec<MifinityErrorList>,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityErrorList {
    #[serde(rename = "type")]
    pub error_type: String,
    pub error_code: String,
    pub message: String,
    pub field: Option<String>,
}
