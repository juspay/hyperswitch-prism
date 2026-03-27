use std::collections::HashMap;

use common_utils::{ext_traits::ValueExt, id_type, request::Method, types::FloatMajorUnit, Email};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
    errors::{ConnectorResponseTransformationError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::cashtocode::CashtocodeRouterData, types::ResponseRouterData};

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashtocodePaymentsRequest {
    amount: FloatMajorUnit,
    transaction_id: String,
    user_id: Secret<id_type::CustomerId>,
    currency: common_enums::Currency,
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    user_alias: Secret<id_type::CustomerId>,
    requested_url: String,
    cancel_url: String,
    email: Option<Email>,
    mid: Secret<String>,
}

fn get_mid(
    connector_config: &ConnectorSpecificConfig,
    payment_method_type: Option<common_enums::PaymentMethodType>,
    currency: common_enums::Currency,
) -> Result<Secret<String>, Report<IntegrationError>> {
    let cashtocode_auth = CashtocodeAuth::try_from((connector_config, &currency))
        .attach_printable_lazy(|| {
            format!("failed to fetch cashtocode credentials for currency '{currency}'")
        })?;

    match payment_method_type {
        Some(common_enums::PaymentMethodType::ClassicReward) => cashtocode_auth
            .merchant_id_classic
            .ok_or(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })
            .attach_printable("missing merchant_id_classic in cashtocode credentials"),
        Some(common_enums::PaymentMethodType::Evoucher) => cashtocode_auth
            .merchant_id_evoucher
            .ok_or(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })
            .attach_printable("missing merchant_id_evoucher in cashtocode credentials"),
        _ => Err(IntegrationError::FailedToObtainAuthType {
            context: Default::default(),
        })
        .attach_printable("unsupported payment method type for cashtocode"),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CashtocodeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for CashtocodePaymentsRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: CashtocodeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let customer_id = item.router_data.resource_common_data.get_customer_id()?;
        let url = item.router_data.request.get_router_return_url()?;
        let mid = get_mid(
            &item.router_data.connector_config,
            item.router_data.request.payment_method_type,
            item.router_data.request.currency,
        )?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        match item.router_data.resource_common_data.payment_method {
            common_enums::PaymentMethod::Reward => Ok(Self {
                amount,
                transaction_id: item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id,
                currency: item.router_data.request.currency,
                user_id: Secret::new(customer_id.to_owned()),
                first_name: None,
                last_name: None,
                user_alias: Secret::new(customer_id),
                requested_url: url.to_owned(),
                cancel_url: url,
                email: item.router_data.request.email.clone(),
                mid,
            }),
            _ => Err(IntegrationError::not_implemented("Payment methods".to_string()).into()),
        }
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct CashtocodeAuthType {
    pub auths: HashMap<common_enums::Currency, CashtocodeAuth>,
}

#[derive(Default, Debug, Deserialize)]
pub struct CashtocodeAuth {
    pub password_classic: Option<Secret<String>>,
    pub password_evoucher: Option<Secret<String>>,
    pub username_classic: Option<Secret<String>>,
    pub username_evoucher: Option<Secret<String>>,
    pub merchant_id_classic: Option<Secret<String>>,
    pub merchant_id_evoucher: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for CashtocodeAuthType {
    type Error = Report<IntegrationError>; // Assuming ErrorStack is the appropriate error type

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Cashtocode { auth_key_map, .. } => Ok(Self {
                auths: auth_key_map
                    .iter()
                    .map(|(currency, auth_value)| {
                        let auth = auth_value
                            .to_owned()
                            .parse_value::<CashtocodeAuth>("CashtocodeAuth")
                            .change_context(IntegrationError::FailedToObtainAuthType {
                                context: Default::default(),
                            })?;
                        Ok((*currency, auth))
                    })
                    .collect::<Result<_, Self::Error>>()?,
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl TryFrom<(&ConnectorSpecificConfig, &common_enums::Currency)> for CashtocodeAuth {
    type Error = Report<IntegrationError>;

    fn try_from(
        value: (&ConnectorSpecificConfig, &common_enums::Currency),
    ) -> Result<Self, Self::Error> {
        let (auth_type, currency) = value;

        match auth_type {
            ConnectorSpecificConfig::Cashtocode { auth_key_map, .. } => {
                let identity_auth_key =
                    auth_key_map
                        .get(currency)
                        .ok_or(IntegrationError::CurrencyNotSupported {
                            message: currency.to_string(),
                            connector: "CashToCode",
                            context: Default::default(),
                        })?;

                identity_auth_key
                    .to_owned()
                    .parse_value::<Self>("CashtocodeAuth")
                    .change_context(IntegrationError::FailedToObtainAuthType {
                        context: Default::default(),
                    })
            }
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CashtocodePaymentStatus {
    Succeeded,
    #[default]
    Processing,
}

impl From<CashtocodePaymentStatus> for common_enums::AttemptStatus {
    fn from(item: CashtocodePaymentStatus) -> Self {
        match item {
            CashtocodePaymentStatus::Succeeded => Self::Charged,
            CashtocodePaymentStatus::Processing => Self::AuthenticationPending,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct CashtocodeErrors {
    pub message: String,
    pub path: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CashtocodePaymentsResponse {
    CashtoCodeError(CashtocodeErrorResponse),
    CashtoCodeData(CashtocodePaymentsResponseData),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashtocodePaymentsResponseData {
    pub pay_url: url::Url,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashtocodePaymentsSyncResponse {
    pub transaction_id: String,
    pub amount: FloatMajorUnit,
}

fn get_redirect_form_data(
    payment_method_type: common_enums::PaymentMethodType,
    response_data: CashtocodePaymentsResponseData,
    http_code: u16,
) -> Result<RedirectForm, Report<ConnectorResponseTransformationError>> {
    match payment_method_type {
        common_enums::PaymentMethodType::ClassicReward => Ok(RedirectForm::Form {
            //redirect form is manually constructed because the connector for this pm type expects query params in the url
            endpoint: response_data.pay_url.to_string(),
            method: Method::Post,
            form_fields: Default::default(),
        }),
        common_enums::PaymentMethodType::Evoucher => Ok(RedirectForm::from((
            //here the pay url gets parsed, and query params are sent as formfields as the connector expects
            response_data.pay_url,
            Method::Get,
        ))),
        _ => Err(Report::new(
            ConnectorResponseTransformationError::unexpected_response_error_with_context(
                http_code,
                Some(utils::get_unimplemented_payment_method_error_message(
                    "CashToCode",
                )),
            ),
        )),
    }
}

impl<
        F,
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<CashtocodePaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorResponseTransformationError>;
    fn try_from(
        item: ResponseRouterData<CashtocodePaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let (status, response) = match response {
            CashtocodePaymentsResponse::CashtoCodeError(error_data) => (
                common_enums::AttemptStatus::Failure,
                Err(ErrorResponse {
                    code: error_data.error.to_string(),
                    status_code: item.http_code,
                    message: error_data.error_description.clone(),
                    reason: Some(error_data.error_description),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
            ),
            CashtocodePaymentsResponse::CashtoCodeData(response_data) => {
                let payment_method_type =
                    router_data.request.payment_method_type.ok_or_else(|| {
                        Report::new(
                            ConnectorResponseTransformationError::response_handling_failed_with_context(
                                http_code,
                                Some(
                                    "authorize: payment_method_type missing on request".to_string(),
                                ),
                            ),
                        )
                    })?;
                let redirection_data =
                    get_redirect_form_data(payment_method_type, response_data, http_code)?;
                (
                    common_enums::AttemptStatus::AuthenticationPending,
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                        ),
                        redirection_data: Some(Box::new(redirection_data)),
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: http_code,
                    }),
                )
            }
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response,
            ..router_data
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CashtocodeErrorResponse {
    pub error: serde_json::Value,
    pub error_description: String,
    pub errors: Option<Vec<CashtocodeErrors>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashtocodeIncomingWebhook {
    pub amount: FloatMajorUnit,
    pub currency: String,
    pub foreign_transaction_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub transaction_id: String,
}
