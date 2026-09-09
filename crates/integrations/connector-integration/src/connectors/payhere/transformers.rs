use crate::{connectors::payhere::PayhereRouterData, types::ResponseRouterData};
use common_enums::AttemptStatus;
use common_utils::crypto::GenerateDigest;
use domain_types::{
    connector_flow::ServerAuthenticationToken,
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PayhereAuthType {
    pub app_id: Secret<String>,
    pub app_secret: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PayhereAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match item {
            ConnectorSpecificConfig::Payhere {
                api_key,
                key1,
                api_secret,
                key2,
                ..
            } => Ok(Self {
                app_id: api_key.clone(),
                app_secret: api_secret.clone(),
                merchant_id: key1.clone(),
                merchant_secret: key2.clone(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: None,
                    }
                }
            )),
        }
    }
}

// ---- Access Token ----

#[derive(Debug, Serialize)]
pub struct PayhereServerAuthenticationTokenRequest {
    pub grant_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayhereRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for PayhereServerAuthenticationTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        _: PayhereRouterData<
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
            grant_type: "client_credentials".to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereServerAuthenticationTokenResponse {
    pub access_token: Secret<String>,
    pub expires_in: Option<i64>,
    pub token_type: Option<String>,
}

impl
    TryFrom<
        ResponseRouterData<
            PayhereServerAuthenticationTokenResponse,
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        >,
    >
    for RouterDataV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            PayhereServerAuthenticationTokenResponse,
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;
        router_data.response = Ok(ServerAuthenticationTokenResponseData {
            access_token: item.response.access_token,
            expires_in: item.response.expires_in,
            token_type: item.response.token_type,
        });
        Ok(router_data)
    }
}

// ---- Authorize ----

/// Signed field set for PayHere's hosted checkout (`POST {base_url}/pay/checkout`),
/// generated locally — see the connector doc comment: the checkout session must
/// belong to the payer's browser, so Authorize returns these fields as a
/// `RedirectForm::Form` rather than posting them itself. `hash`
/// (`upper(md5(merchant_id + order_id + amount + currency + upper(md5(merchant_secret))))`)
/// is PayHere's request signature.
#[derive(Debug, Serialize)]
pub struct PayherePaymentsRequest {
    pub merchant_id: String,
    pub return_url: String,
    pub cancel_url: String,
    pub notify_url: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub country: String,
    pub order_id: String,
    pub items: String,
    pub currency: String,
    pub amount: String,
    pub hash: String,
}

type PayhereAuthorizeRouterData<T> = RouterDataV2<
    domain_types::connector_flow::Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<T>,
    PaymentsResponseData,
>;

impl PayherePaymentsRequest {
    fn from_router_data<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
    >(
        router_data: &PayhereAuthorizeRouterData<T>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        use hyperswitch_masking::ExposeInterface;

        match &router_data.request.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Wallet(
                domain_types::payment_method_data::WalletData::PayhereRedirect {},
            ) => {}
            _ => {
                return Err(error_stack::report!(IntegrationError::InvalidWallet {
                    context: IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some(
                            "payhere: only the payhere_redirect wallet is supported".to_string(),
                        ),
                    },
                }))
            }
        }

        let auth = PayhereAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: None,
                },
            },
        )?;

        let amount = common_utils::types::AmountConvertor::convert(
            &common_utils::types::StringMajorUnitForConnector,
            router_data.request.minor_amount,
            router_data.request.currency,
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: None,
                doc_url: None,
                additional_context: Some("payhere: amount conversion failed".to_string()),
            },
        })?
        .get_amount_as_string();

        let hash_failed = || IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: None,
                doc_url: None,
                additional_context: Some("payhere: failed to compute checkout hash".to_string()),
            },
        };
        let hash_secret = common_utils::crypto::Md5
            .generate_digest(auth.merchant_secret.expose().as_bytes())
            .change_context(hash_failed())?;
        let hash_secret_upper = hex::encode(hash_secret).to_uppercase();
        let message = format!(
            "{}{}{}{}{}",
            auth.merchant_id.peek(),
            router_data
                .resource_common_data
                .connector_request_reference_id,
            amount,
            router_data.request.currency,
            hash_secret_upper
        );
        let final_hash = common_utils::crypto::Md5
            .generate_digest(message.as_bytes())
            .change_context(hash_failed())?;
        let hash = hex::encode(final_hash).to_uppercase();

        // The hosted checkout requires real customer billing details — fail
        // closed when they are missing instead of posting fabricated data.
        // Email/phone first come from the billing address block; when it lacks
        // them, fall back to the request-level customer channel (same idiom as
        // barclaycard/razorpay/givepayments).
        let missing_billing = |field: &'static str| IntegrationError::MissingRequiredField {
            field_name: field,
            context: IntegrationErrorContext {
                suggested_action: None,
                doc_url: None,
                additional_context: Some(
                    "payhere: required (billing address block or customer object)".to_string(),
                ),
            },
        };
        let first_name = router_data
            .resource_common_data
            .get_billing_first_name()
            .change_context(missing_billing("billing.first_name"))?;
        let last_name = router_data
            .resource_common_data
            .get_billing_last_name()
            .change_context(missing_billing("billing.last_name"))?;
        let email = router_data
            .resource_common_data
            .get_billing_email()
            .or_else(|_| router_data.request.get_email())
            .change_context(missing_billing("billing.email / customer.email"))?;
        let phone = router_data
            .resource_common_data
            .get_billing_phone_number()
            .or_else(|_| {
                router_data
                    .request
                    .customer
                    .as_ref()
                    .and_then(|customer| {
                        customer.customer_phone_number.clone().map(|number| {
                            match &customer.customer_phone_country_code {
                                Some(country_code) => {
                                    Secret::new(format!("{country_code}{}", number.peek()))
                                }
                                None => number,
                            }
                        })
                    })
                    .ok_or_else(domain_types::utils::missing_field_err(
                        "customer.phone_number",
                    ))
            })
            .change_context(missing_billing("billing.phone / customer.phone_number"))?;
        let address = router_data
            .resource_common_data
            .get_billing_line1()
            .change_context(missing_billing("billing.line1"))?;
        let city = router_data
            .resource_common_data
            .get_billing_city()
            .change_context(missing_billing("billing.city"))?;
        let country = router_data
            .resource_common_data
            .get_billing_country()
            .change_context(missing_billing("billing.country"))?
            .to_string();

        Ok(Self {
            merchant_id: auth.merchant_id.peek().to_string(),
            return_url: router_data
                .request
                .router_return_url
                .clone()
                .unwrap_or_default(),
            cancel_url: router_data
                .request
                .router_return_url
                .clone()
                .unwrap_or_default(),
            notify_url: router_data.request.webhook_url.clone().unwrap_or_default(),
            first_name: first_name.expose().to_string(),
            last_name: last_name.expose().to_string(),
            email: email.peek().to_string(),
            phone: phone.expose().to_string(),
            address: address.expose().to_string(),
            city: city.expose().to_string(),
            country,
            order_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            items: "Order".to_string(),
            currency: router_data.request.currency.to_string(),
            amount,
            hash,
        })
    }
}

pub(crate) fn handle_authorize_response<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    data: &PayhereAuthorizeRouterData<T>,
    _event_builder: Option<&mut common_utils::events::Event>,
    _res: domain_types::router_response_types::Response,
) -> common_utils::errors::CustomResult<PayhereAuthorizeRouterData<T>, ConnectorError> {
    let mut router_data = data.clone();

    let request = PayherePaymentsRequest::from_router_data(&router_data).change_context(
        ConnectorError::ResponseHandlingFailed {
            context: Default::default(),
        },
    )?;
    let order_id = request.order_id.clone();
    let request_value =
        serde_json::to_value(&request).change_context(ConnectorError::ResponseHandlingFailed {
            context: Default::default(),
        })?;
    let form_object = request_value
        .as_object()
        .ok_or(ConnectorError::ResponseHandlingFailed {
            context: Default::default(),
        })?;
    let form_fields = form_object
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_str()
                .map(|string| (key.clone(), string.to_string()))
        })
        .collect();

    // The checkout host must come from the configured base URL — the same
    // config fed by superposition overrides — never from a hardcoded table.
    let base = router_data
        .resource_common_data
        .connectors
        .payhere
        .base_url
        .trim_end_matches('/');
    if base.is_empty() {
        return Err(error_stack::report!(
            ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            }
        ));
    }

    router_data.response = Ok(PaymentsResponseData::TransactionResponse {
        // PayHere issues no payment id at checkout — the merchant-sent order_id
        // is the only reference PayHere knows (echoed in webhooks and queried
        // via the Retrieval API). Store it as the connector transaction id so
        // downstream flows (PSync) have a reference to work with.
        resource_id: domain_types::connector_types::ResponseId::ConnectorTransactionId(
            order_id.clone(),
        ),
        redirection_data: Some(Box::new(RedirectForm::Form {
            endpoint: format!("{base}/pay/checkout"),
            method: common_utils::Method::Post,
            form_fields,
        })),
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: Some(order_id),
        network_txn_link_id: None,
        payment_account_reference: None,
        splits: None,
        incremental_authorization_allowed: None,
        status_code: 200, // HTTP OK since it's local
    });
    router_data.resource_common_data.status = AttemptStatus::AuthenticationPending;
    Ok(router_data)
}
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PayhereSyncResponse {
    pub status: i32,
    pub msg: String,
    pub data: Option<Vec<PayherePaymentData>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayherePaymentData {
    pub payment_id: i64,
    pub order_id: String,
    pub status: String,
    pub currency: String,
    pub amount: f64,
}

impl
    TryFrom<
        ResponseRouterData<
            PayhereSyncResponse,
            RouterDataV2<
                domain_types::connector_flow::PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        >,
    >
    for RouterDataV2<
        domain_types::connector_flow::PSync,
        PaymentFlowData,
        PaymentsSyncData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            PayhereSyncResponse,
            RouterDataV2<
                domain_types::connector_flow::PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;

        let payment_data = item.response.data.and_then(|mut d| d.pop());

        let status = if let Some(ref data) = payment_data {
            match data.status.as_str() {
                "RECEIVED" | "REFUND REQUESTED" | "REFUND PROCESSING" | "REFUNDED"
                | "CHARGEBACKED" => AttemptStatus::Charged,
                _ => AttemptStatus::Pending,
            }
        } else {
            AttemptStatus::Pending
        };

        router_data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: domain_types::connector_types::ResponseId::ConnectorTransactionId(
                payment_data
                    .as_ref()
                    .map(|d| d.payment_id.to_string())
                    .unwrap_or_default(),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            // Surface PayHere's order_id as the reference id so callers that
            // match the sync response against their tracker (euler's
            // mandatory-PSync integrity check compares this field) see the id
            // that was committed at authorize time.
            connector_response_reference_id: payment_data.as_ref().map(|d| d.order_id.clone()),
            network_txn_link_id: None,
            payment_account_reference: None,
            splits: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        });
        router_data.resource_common_data.status = status;

        Ok(router_data)
    }
}

// ---- Webhook ----

#[derive(Debug, Deserialize)]
pub struct PayhereWebhookPayload {
    pub merchant_id: String,
    pub order_id: String,
    pub payment_id: Option<i64>,
    pub payhere_amount: String,
    pub payhere_currency: String,
    pub status_code: i32,
    pub md5sig: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereErrorResponse {
    pub status: i32,
    pub msg: String,
}

pub fn get_status_from_code(code: i32) -> AttemptStatus {
    match code {
        2 => AttemptStatus::Charged,
        0 => AttemptStatus::Pending,
        -1 | -2 => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    }
}
