use crate::{
    connectors::payhere::PayhereRouterData, types::ResponseRouterData, utils::is_refund_failure,
};
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{
    consts::NO_ERROR_MESSAGE, crypto::GenerateDigest, types::StringMajorUnit, Email,
};
use domain_types::{
    connector_flow::{RSync, Refund, ServerAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
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
                app_id,
                merchant_id,
                app_secret,
                merchant_secret,
                ..
            } => Ok(Self {
                app_id: app_id.clone(),
                app_secret: app_secret.clone(),
                merchant_id: merchant_id.clone(),
                merchant_secret: merchant_secret.clone(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Provide connector_specific_config.payhere (PayhereConfig) with app_id, app_secret, merchant_id and merchant_secret"
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://support.payhere.lk/api-&-mobile-sdk/merchant-api"
                                .to_string(),
                        ),
                        additional_context: Some(
                            "payhere: expected PayhereConfig connector credentials".to_string(),
                        ),
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
        item: PayhereRouterData<
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
            grant_type: item.router_data.request.grant_type.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereServerAuthenticationTokenResponse {
    pub access_token: Secret<String>,
    pub expires_in: Option<i64>,
    pub token_type: Option<String>,
}

impl TryFrom<ResponseRouterData<PayhereServerAuthenticationTokenResponse, Self>>
    for RouterDataV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PayhereServerAuthenticationTokenResponse, Self>,
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
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub email: Email,
    pub phone: Secret<String>,
    pub address: Secret<String>,
    pub city: Secret<String>,
    pub country: String,
    pub order_id: String,
    pub items: String,
    pub currency: Currency,
    pub amount: StringMajorUnit,
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
                        suggested_action: Some(
                            "Pass wallet.payhere_redirect as payment_method_data; PayHere supports only its own hosted checkout"
                                .to_string(),
                        ),
                        doc_url: Some("https://support.payhere.lk/api-&-mobile-sdk/api".to_string()),
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
                    suggested_action: Some(
                        "Check that connector config carries the PayhereConfig variant with valid app_id/app_secret"
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://support.payhere.lk/api-&-mobile-sdk/merchant-api".to_string(),
                    ),
                    additional_context: Some(
                        "payhere: failed to build auth from connector config".to_string(),
                    ),
                },
            },
        )?;

        let amount = common_utils::types::AmountConvertor::convert(
            &common_utils::types::StringMajorUnitForConnector,
            router_data.request.amount.amount,
            router_data.request.currency,
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send a positive amount in the payment currency; PayHere checkout expects a major-unit decimal string"
                        .to_string(),
                ),
                doc_url: Some("https://support.payhere.lk/api-&-mobile-sdk/api".to_string()),
                additional_context: Some("payhere: amount conversion failed".to_string()),
            },
        })?;

        let hash_failed = || {
            IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Verify merchant_secret matches the PayHere account; the checkout hash is md5(merchant_id + order_id + amount + currency + upper(md5(merchant_secret)))"
                        .to_string(),
                ),
                doc_url: Some("https://support.payhere.lk/api-&-mobile-sdk/api".to_string()),
                additional_context: Some("payhere: failed to compute checkout hash".to_string()),
            },
        }
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
            amount.get_amount_as_string(),
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
        // barclaycard/razorpay/givepayments). The extra change_context names
        // *both* attempted sources — the underlying can only name the last one
        // tried.
        let missing_billing_fallback = |field: &'static str| {
            IntegrationError::MissingRequiredField {
                field_name: field,
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Populate the billing address block (first_name/last_name/email/phone) or the request-level customer email/phone; PayHere hosted checkout requires them"
                            .to_string(),
                    ),
                    doc_url: Some("https://support.payhere.lk/api-&-mobile-sdk/api".to_string()),
                    additional_context: Some(
                        "payhere: required (billing address block or customer object)".to_string(),
                    ),
                },
            }
        };
        let first_name = router_data.resource_common_data.get_billing_first_name()?;
        let last_name = router_data.resource_common_data.get_billing_last_name()?;
        let email = router_data
            .resource_common_data
            .get_billing_email()
            .or_else(|_| router_data.request.get_email())
            .change_context(missing_billing_fallback("billing.email / customer.email"))?;
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
            .change_context(missing_billing_fallback(
                "billing.phone / customer.phone_number",
            ))?;
        // address/city/country are OPTIONAL: send them when present,
        // otherwise send an empty string in the checkout form.
        let address = router_data
            .resource_common_data
            .get_optional_billing_line1()
            .unwrap_or_default();
        let city = router_data
            .resource_common_data
            .get_optional_billing_city()
            .unwrap_or_default();
        let country = router_data
            .resource_common_data
            .get_optional_billing_country()
            .map(|c| c.to_string())
            .unwrap_or_default();

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
            first_name,
            last_name,
            email,
            phone,
            address,
            city,
            country,
            order_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            // PayHere requires a free-text `items` descriptor (single string,
            // not a line-items array). When the caller supplies order line
            // items (UCS `PaymentFlowData.order_details`), join product names
            // — same pattern as grabpay's `build_items`. Fall back to the
            // placeholder from PayHere's docs when none are provided.
            items: router_data
                .resource_common_data
                .order_details
                .as_ref()
                .map(|details| {
                    details
                        .iter()
                        .filter(|d| !d.product_name.is_empty())
                        .map(|d| d.product_name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Order".to_string()),
            currency: router_data.request.currency,
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

    let request = PayherePaymentsRequest::from_router_data(&router_data).map_err(|report| {
        let inner = report.to_string();
        report.change_context(ConnectorError::ResponseHandlingFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(format!(
                    "payhere: failed to build checkout request fields: {inner}"
                )),
            },
        })
    })?;
    let order_id = request.order_id.clone();
    let request_value = serde_json::to_value(&request).map_err(|err| {
        error_stack::report!(ConnectorError::ResponseHandlingFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(format!(
                    "payhere: failed to serialize checkout request for redirect form: {err}"
                )),
            },
        })
    })?;
    let form_object = request_value
        .as_object()
        .ok_or(ConnectorError::ResponseHandlingFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(
                    "payhere: serialized checkout request is not a JSON object".to_string(),
                ),
            },
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
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(
                        "payhere: connector base_url is empty (config/development.toml or superposition override)".to_string(),
                    ),
                },
            }
        ));
    }

    router_data.response = Ok(PaymentsResponseData::TransactionResponse {
        // PayHere issues no payment id at checkout time — unique payment ids
        // only materialize after the payer completes the hosted checkout and
        // appear in sync responses/webhooks. Until then the merchant-sent
        // order_id is the ONLY reference PayHere knows (echoed in webhooks and
        // queried via the Retrieval API). Store it as the connector transaction
        // id so downstream flows (PSync) have a reference to work with.
        resource_id: domain_types::connector_types::ResponseId::ConnectorTransactionId(
            order_id.clone(),
        ),
        redirection_data: Some(Box::new(RedirectForm::Form {
            // `/pay/checkout` is PayHere's documented static hosted-checkout
            // endpoint — the connector does not return a session URL; the
            // base host switches via config for sandbox/production.
            endpoint: format!("{base}/pay/checkout"),
            method: common_utils::Method::Post,
            form_fields,
        })),
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
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

/// PayHere Retrieval API payment status — any status PayHere has ever
/// assigned the payment (incl. terminal post-payment states) counts as
/// "payment acknowledged" for our AttemptStatus mapping; anything else is
/// still in flight.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum PayherePaymentStatus {
    #[serde(rename = "RECEIVED")]
    Received,
    #[serde(rename = "REFUND REQUESTED")]
    RefundRequested,
    #[serde(rename = "REFUND PROCESSING")]
    RefundProcessing,
    #[serde(rename = "REFUNDED")]
    Refunded,
    #[serde(rename = "REFUND FAILED")]
    RefundFailed,
    #[serde(rename = "CHARGEBACKED")]
    Chargebacked,
    #[serde(other)]
    Unknown,
}

/// Trailing per-payment refund block on Retrieval API entries — present (with
/// `date`/`amount`) once a refund has been recorded against the payment,
/// `null`/absent otherwise. Informational only: the entry's payment-level
/// `status` field is authoritative for refund-status mapping.
#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereRefundDetails {
    pub date: String,
    pub amount: common_utils::types::FloatMajorUnit,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayherePaymentData {
    pub payment_id: i64,
    pub order_id: String,
    pub status: PayherePaymentStatus,
    pub currency: Currency,
    pub amount: common_utils::types::FloatMajorUnit,
    /// Optional so PSync is unaffected when PayHere omits the block.
    pub refund: Option<PayhereRefundDetails>,
}

impl TryFrom<ResponseRouterData<PayhereSyncResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PSync,
        PaymentFlowData,
        PaymentsSyncData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PayhereSyncResponse, Self>) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;

        let payment_data = item.response.data.and_then(|mut d| d.pop());

        let status = if let Some(ref data) = payment_data {
            match data.status {
                PayherePaymentStatus::Received
                | PayherePaymentStatus::RefundRequested
                | PayherePaymentStatus::RefundProcessing
                | PayherePaymentStatus::Refunded
                | PayherePaymentStatus::RefundFailed
                | PayherePaymentStatus::Chargebacked => AttemptStatus::Charged,
                PayherePaymentStatus::Unknown => AttemptStatus::Pending,
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
            // that was committed at authorize time. When PayHere has no record
            // yet (Pending, data:null) there is nothing to echo from the
            // response — fall back to the reference committed in the sync
            // request so the callers still receive that same saved id.
            connector_response_reference_id: payment_data
                .as_ref()
                .map(|d| d.order_id.clone())
                .or_else(|| {
                    Some(
                        router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    )
                }),
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

// ---- RSync ----
//
// PayHere documents no dedicated refund-retrieval endpoint: RSync piggybacks
// on the same Retrieval API (`GET /merchant/v1/payment/search?order_id=...`)
// as PSync, reusing the sync response shape. Refund-bearing entries
// additionally carry the optional `refund` block (date/amount) and a refund
// wire status in the payment-level `status` field. A distinct alias name is
// required: `create_all_prerequisites!` keys its per-flow templating types off
// the response-body token, so two flows sharing `PayhereSyncResponse` would
// collide (E0428/E0119) — same idiom as airwallex's `AirwallexRefundSyncResponse`.
pub type PayhereRsyncResponse = PayhereSyncResponse;

/// Exhaustive on purpose — no `_ =>` arm; unknown wire values are absorbed at
/// the deserialization layer by `PayherePaymentStatus::Unknown`.
impl From<PayherePaymentStatus> for RefundStatus {
    fn from(status: PayherePaymentStatus) -> Self {
        match status {
            PayherePaymentStatus::RefundRequested | PayherePaymentStatus::RefundProcessing => {
                Self::Pending
            }
            PayherePaymentStatus::Refunded => Self::Success,
            PayherePaymentStatus::RefundFailed => Self::Failure,
            // Payment-level (non-refund) states prove nothing about a refund —
            // stay non-terminal rather than condemning a refund we can't
            // prove failed.
            PayherePaymentStatus::Received
            | PayherePaymentStatus::Chargebacked
            | PayherePaymentStatus::Unknown => Self::Pending,
        }
    }
}

impl TryFrom<ResponseRouterData<PayhereRsyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PayhereRsyncResponse, Self>) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        // Entry selection mirrors the PSync transformer exactly: take the
        // trailing entry of `data`; empty data / "no records" envelope
        // (`data: null`) leaves the refund Pending.
        let payment_data = item.response.data.and_then(|mut entries| entries.pop());

        let refund_status = payment_data
            .as_ref()
            .map(|entry| RefundStatus::from(entry.status))
            .unwrap_or(RefundStatus::Pending);

        // In-band failure: the search API answers `200 OK` and reports a
        // failed refund only via `data[*].status = "REFUND FAILED"`. Repo
        // convention (see crate::utils::is_refund_failure and the Refund flow
        // below) surfaces that as Err(ErrorResponse { .. }), not an Ok
        // carrying a Failure status.
        let response = if is_refund_failure(refund_status) {
            Err(ErrorResponse {
                code: item.response.status.to_string(),
                message: item.response.msg.clone(),
                reason: Some(item.response.msg.clone()),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Refund(refund_status)),
                connector_transaction_id: Some(
                    router_data.request.connector_transaction_id.clone(),
                ),
                ..Default::default()
            })
        } else {
            Ok(RefundsResponseData {
                // The search wire response carries no refund reference id (the
                // `refund` block holds only date+amount) — echo the reference
                // recorded on the tracker by the Refund API call.
                connector_refund_id: router_data.request.connector_refund_id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
        };

        Ok(Self {
            response,
            ..router_data
        })
    }
}

// ---- Refund ----

/// PayHere Refund API request body — `POST /merchant/v1/payment/refund`.
/// `amount` is PayHere's 2-decimal major-unit string (`"100.50"`, decimal
/// point included), which is exactly what `StringMajorUnit` renders for LKR /
/// USD. (`StringTwoDecimalUnit` would render `"10050"` — it *drops* the
/// decimal point — so it is NOT the right wrapper here.)
#[derive(Debug, Serialize)]
pub struct PayhereRefundRequest {
    pub payment_id: String,
    pub description: String,
    pub amount: StringMajorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayhereRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PayhereRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: PayhereRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // ASSUMPTION (id swap across flows): PayHere checkout returns no
        // payment id, so Authorize stores the merchant `order_id` as
        // connector_transaction_id. The mandatory post-checkout PSync then
        // rewrites the tracked resource_id to the NUMERIC PayHere payment_id
        // (see the PSync response transformer below: resource_id =
        // `payment_id.to_string()`). By refund time,
        // `connector_transaction_id` is therefore expected to hold the numeric
        // payment_id that /merchant/v1/payment/refund requires.
        let payment_id = router_data.request.connector_transaction_id.clone();

        let amount = common_utils::types::AmountConvertor::convert(
            &common_utils::types::StringMajorUnitForConnector,
            router_data.request.refund_amount.amount,
            router_data.request.currency,
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send a positive refund amount in the payment currency; PayHere refund expects a 2-decimal major-unit string"
                        .to_string(),
                ),
                doc_url: Some(
                    "https://support.payhere.lk/api-&-mobile-sdk/merchant-api".to_string(),
                ),
                additional_context: Some(
                    "payhere: refund amount conversion failed".to_string(),
                ),
            },
        })?;

        Ok(Self {
            payment_id,
            // PayHere requires a free-text refund reason on every call.
            description: router_data
                .request
                .reason
                .clone()
                .unwrap_or_else(|| "Refund".to_string()),
            amount,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereRefundResponse {
    pub status: i32,
    pub msg: Option<String>,
    /// Unique refund reference number (→ `connector_refund_id`); `null` only
    /// for authorization refunds, which are out of scope.
    pub data: Option<i64>,
}

/// PayHere Refund API response-level status (`status` is a bare integer on
/// the wire, so a serde enum can't model it — cf. `get_status_from_code`,
/// which does the same job for webhook/payment status codes). Only `-1` is a
/// terminal refund failure; a code PayHere adds later stays non-terminal
/// (`Pending`) rather than condemning a refund we can't prove failed.
pub fn get_refund_status_from_code(code: i32) -> RefundStatus {
    match code {
        // 0 = "error initiating refund" per PayHere docs — retryable.
        0 => RefundStatus::Pending,
        1 => RefundStatus::Success,
        -1 => RefundStatus::Failure,
        _ => RefundStatus::Pending,
    }
}

impl TryFrom<ResponseRouterData<PayhereRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PayhereRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = get_refund_status_from_code(item.response.status);

        // In-band failure: PayHere answers `200 OK` with `status: -1` for a
        // refund it refused to process. Repo convention (see
        // crate::utils::is_refund_failure and the refund flow pattern) is to
        // surface that as Err(ErrorResponse { .. }), not Ok with a Failure
        // status.
        let response = if is_refund_failure(refund_status) {
            Err(ErrorResponse {
                code: item.response.status.to_string(),
                message: item
                    .response
                    .msg
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.msg.clone(),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Refund(refund_status)),
                connector_transaction_id: Some(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                ..Default::default()
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item
                    .response
                    .data
                    .map(|reference| reference.to_string())
                    .unwrap_or_default(),
                refund_status,
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

/// PayHere speaks two error dialects:
/// - the business-envelope `{status, msg}` (e.g. refund `status: -1`,
///   oauth-token `status: -2`), and
/// - the Spring-OAuth `{error, error_description}` body its OAuth-protected
///   merchant endpoints (`/merchant/v1/payment/*`) return on 401 for an
///   invalid/expired Bearer token.
///
/// Both deserialize into this one struct; `build_error_response` picks
/// whichever pair is populated.
#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereErrorResponse {
    pub status: Option<i32>,
    pub msg: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

pub fn get_status_from_code(code: i32) -> AttemptStatus {
    match code {
        2 => AttemptStatus::Charged,
        0 => AttemptStatus::Pending,
        -1 | -2 => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    }
}
