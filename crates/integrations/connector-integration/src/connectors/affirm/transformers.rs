use std::fmt::Debug;

use common_enums::{AttemptStatus, CountryAlpha2, Currency, RefundStatus};
use common_utils::{request::Method, types::MinorUnit, Email};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{PayLaterData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{connectors::affirm::AffirmRouterData, types::ResponseRouterData};

// ===== AUTH =====

#[derive(Debug, Clone)]
pub struct AffirmAuthType {
    pub public_key: Secret<String>,
    pub private_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AffirmAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Affirm {
                public_key,
                private_key,
                ..
            } => Ok(Self {
                public_key: public_key.to_owned(),
                private_key: private_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Expected Affirm public_key and private_key".to_string()
                        ),
                        ..Default::default()
                    }
                }
            )),
        }
    }
}

// ===== ERROR =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffirmErrorResponse {
    pub status_code: Option<serde_json::Value>,
    pub message: Option<String>,
    pub code: Option<String>,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub field: Option<String>,
}

// ===== STATUS =====

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AffirmTransactionStatus {
    Authorized,
    Captured,
    PartiallyCaptured,
    Voided,
    Refunded,
    PartiallyRefunded,
    Disputed,
    DisputeRefunded,
    Declined,
    #[serde(other)]
    Unknown,
}

impl From<AffirmTransactionStatus> for AttemptStatus {
    fn from(status: AffirmTransactionStatus) -> Self {
        match status {
            AffirmTransactionStatus::Authorized => Self::Authorized,
            AffirmTransactionStatus::Captured => Self::Charged,
            AffirmTransactionStatus::PartiallyCaptured => Self::PartialCharged,
            AffirmTransactionStatus::Voided => Self::Voided,
            // Full/partial refunds and disputes need manual reconciliation — surface as
            // Unresolved (mirrors hyperswitch's upstream Affirm mapping, which groups
            // Refunded | PartiallyRefunded | Disputed | DisputeRefunded => Unresolved).
            // There is no AttemptStatus::Refunded; the refund itself is tracked separately
            // via the Refund/RSync flow (RefundStatus).
            AffirmTransactionStatus::Refunded
            | AffirmTransactionStatus::Disputed
            | AffirmTransactionStatus::DisputeRefunded
            | AffirmTransactionStatus::PartiallyRefunded => Self::Unresolved,
            AffirmTransactionStatus::Declined => Self::Failure,
            // An unrecognised/unmapped Affirm status is surfaced as `Unknown` (not `Pending`)
            // so a genuinely unknown state is never silently treated as still-processing
            // (mirrors the hyperswitch connector's `Unknown => AttemptStatus::Unknown`).
            AffirmTransactionStatus::Unknown => Self::Unknown,
        }
    }
}

// ===== AUTHORIZE (two-step hosted-checkout flow) =====
//
// Affirm BNPL is a two-call flow, mapped onto the single UCS `Authorize` flow
// (UCS re-invokes Authorize with `redirect_response` populated rather than having
// a separate CompleteAuthorize flow):
//
//   1. INITIATE (no checkout_token yet): POST /api/v2/checkout/direct with
//      merchant + billing (+ shipping) + total  ->  { checkout_id, redirect_url }.
//      We return AuthenticationPending and redirect the shopper to `redirect_url`.
//   2. COMPLETE (checkout_token present in redirect_response): POST /api/v1/transactions
//      with { transaction_id: checkout_token, order_id, currency, total }  ->  the
//      authorized transaction.

/// Returns the Affirm `checkout_token` produced by the hosted-checkout redirect,
/// if present. It arrives either in `redirect_response.params` (single value) or in
/// `redirect_response.payload` under the `checkout_token` key. `None` means we are
/// still on the INITIATE leg (the shopper has not been redirected yet).
pub fn affirm_checkout_token<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Option<Secret<String>> {
    let redirect = router_data.request.redirect_response.as_ref()?;
    if let Some(payload) = redirect.payload.as_ref() {
        if let Some(token) = payload
            .clone()
            .expose()
            .as_object()
            .and_then(|map| map.get("checkout_token"))
            .and_then(|val| val.as_str())
        {
            return Some(Secret::new(token.to_string()));
        }
    }
    if let Some(params) = redirect.params.as_ref() {
        let raw = params.clone().expose();
        if !raw.is_empty() {
            return Some(Secret::new(raw));
        }
    }
    None
}

/// Affirm only supports the PayLater (Affirm BNPL redirect) payment method.
fn ensure_affirm_paylater<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    match &router_data.request.payment_method_data {
        PaymentMethodData::PayLater(PayLaterData::AffirmRedirect {}) => Ok(()),
        _ => Err(error_stack::report!(
            errors::IntegrationError::NotImplemented(
                "Affirm only supports the PayLater (Affirm) payment method".to_string(),
                errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Only PayLater(AffirmRedirect) is supported by the Affirm connector."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            )
        )),
    }
}

// ----- Shared address / party structs for the checkout-create (initiate) body -----

#[derive(Debug, Serialize)]
pub struct Merchant {
    pub public_api_key: Secret<String>,
    pub user_confirmation_url: String,
    pub user_cancel_url: String,
}

#[derive(Debug, Serialize)]
pub struct Name {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct Address {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zipcode: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<CountryAlpha2>,
}

#[derive(Debug, Serialize)]
pub struct Party {
    pub name: Name,
    pub address: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
}

/// INITIATE body — POST /api/v2/checkout/direct.
#[derive(Debug, Serialize)]
pub struct AffirmCheckoutRequest {
    pub merchant: Merchant,
    pub billing: Party,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<Party>,
    pub total: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
}

/// COMPLETE body — POST /api/v1/transactions.
#[derive(Debug, Serialize)]
pub struct AffirmTransactionRequest {
    pub transaction_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<MinorUnit>,
}

/// Authorize request body — either the checkout-create (initiate) leg or the
/// transaction-create (complete) leg. Untagged so each serialises to its own flat
/// JSON object.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AffirmPaymentsRequest {
    Checkout(Box<AffirmCheckoutRequest>),
    Transaction(AffirmTransactionRequest),
}

fn build_billing_party<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<Party, error_stack::Report<errors::IntegrationError>> {
    let common = &router_data.resource_common_data;
    Ok(Party {
        name: Name {
            first: Some(common.get_billing_first_name()?),
            last: Some(common.get_billing_last_name()?),
            full: common.get_optional_billing_full_name(),
        },
        address: Address {
            line1: common.get_optional_billing_line1(),
            line2: common.get_optional_billing_line2(),
            city: common.get_optional_billing_city(),
            state: common.get_optional_billing_state(),
            zipcode: common.get_optional_billing_zip(),
            country: common.get_optional_billing_country(),
        },
        phone_number: common.get_optional_billing_phone_number(),
        email: common.get_optional_billing_email(),
    })
}

fn build_shipping_party<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Option<Party> {
    let common = &router_data.resource_common_data;
    // Only emit shipping when a shipping address was actually supplied.
    common.get_optional_shipping()?;
    Some(Party {
        name: Name {
            first: common.get_optional_shipping_first_name(),
            last: common.get_optional_shipping_last_name(),
            full: common.get_optional_shipping_full_name(),
        },
        address: Address {
            line1: common.get_optional_shipping_line1(),
            line2: common.get_optional_shipping_line2(),
            city: common.get_optional_shipping_city(),
            state: common.get_optional_shipping_state(),
            zipcode: common.get_optional_shipping_zip(),
            country: common.get_optional_shipping_country(),
        },
        phone_number: common.get_optional_shipping_phone_number(),
        email: common.get_optional_shipping_email(),
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AffirmRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AffirmPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: AffirmRouterData<
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
        ensure_affirm_paylater(router_data)?;

        let order_id = Some(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        match affirm_checkout_token(router_data) {
            // COMPLETE leg: charge the token returned by the hosted-checkout redirect.
            Some(transaction_id) => Ok(Self::Transaction(AffirmTransactionRequest {
                transaction_id,
                order_id,
                currency: Some(router_data.request.currency),
                total: Some(router_data.request.minor_amount),
            })),
            // INITIATE leg: create the checkout and mint the redirect URL.
            None => {
                let auth = AffirmAuthType::try_from(&router_data.connector_config)?;
                let merchant = Merchant {
                    public_api_key: auth.public_key,
                    user_confirmation_url: router_data
                        .request
                        .complete_authorize_url
                        .clone()
                        .ok_or_else(|| {
                            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                                field_name: "complete_authorize_url",
                                context: errors::IntegrationErrorContext {
                                    additional_context: Some(
                                        "Affirm INITIATE needs complete_authorize_url to build the hosted-checkout user_confirmation_url."
                                            .to_string(),
                                    ),
                                    ..Default::default()
                                },
                            })
                        })?,
                    user_cancel_url: router_data.request.router_return_url.clone().ok_or_else(
                        || {
                            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                                field_name: "router_return_url",
                                context: errors::IntegrationErrorContext {
                                    additional_context: Some(
                                        "Affirm INITIATE needs router_return_url to build the hosted-checkout user_cancel_url."
                                            .to_string(),
                                    ),
                                    ..Default::default()
                                },
                            })
                        },
                    )?,
                };
                Ok(Self::Checkout(Box::new(AffirmCheckoutRequest {
                    merchant,
                    billing: build_billing_party(router_data)?,
                    shipping: build_shipping_party(router_data),
                    total: router_data.request.minor_amount,
                    order_id,
                })))
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub amount: Option<MinorUnit>,
    pub currency: Option<String>,
    pub created: Option<String>,
}

/// INITIATE response — the created checkout plus the URL to send the shopper to.
#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmCheckoutResponse {
    pub checkout_id: String,
    pub redirect_url: String,
}

/// COMPLETE response — the authorized transaction.
#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmTransactionResponse {
    pub id: String,
    pub status: AffirmTransactionStatus,
    pub amount: Option<MinorUnit>,
    pub amount_refunded: Option<MinorUnit>,
    pub currency: Option<String>,
    pub order_id: Option<String>,
    pub checkout_id: Option<String>,
    pub events: Option<Vec<AffirmEvent>>,
}

/// Authorize response — discriminated by the presence of `redirect_url`, which
/// only the checkout-create (initiate) leg returns.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AffirmPaymentsResponse {
    Checkout(AffirmCheckoutResponse),
    Transaction(Box<AffirmTransactionResponse>),
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AffirmPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AffirmPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            // INITIATE: send the shopper to Affirm's hosted checkout.
            AffirmPaymentsResponse::Checkout(resp) => {
                let redirect_url = Url::parse(&resp.redirect_url).change_context(
                    errors::ConnectorError::ResponseHandlingFailed {
                        context: errors::ResponseTransformationErrorContext {
                            http_status_code: Some(item.http_code),
                            additional_context: Some(
                                "Affirm returned an unparseable hosted-checkout redirect_url."
                                    .to_string(),
                            ),
                        },
                    },
                )?;
                let redirection_data = RedirectForm::from((redirect_url, Method::Get));

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::AuthenticationPending,
                        connector_order_id: Some(resp.checkout_id.clone()),
                        ..item.router_data.resource_common_data.clone()
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(resp.checkout_id),
                        redirection_data: Some(Box::new(redirection_data)),
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                    }),
                    ..item.router_data.clone()
                })
            }
            // COMPLETE: the transaction was authorized.
            AffirmPaymentsResponse::Transaction(resp) => {
                let status = AttemptStatus::from(resp.status.clone());
                let transaction_id = resp.id.clone();

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_order_id: Some(transaction_id.clone()),
                        ..item.router_data.resource_common_data.clone()
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: resp.order_id.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                    }),
                    ..item.router_data.clone()
                })
            }
        }
    }
}

// ===== PSYNC =====
// GET /api/v1/transactions/{id}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmSyncResponse {
    pub id: String,
    pub status: AffirmTransactionStatus,
    pub order_id: Option<String>,
    pub amount_refunded: Option<MinorUnit>,
    pub events: Option<Vec<AffirmEvent>>,
}

impl TryFrom<ResponseRouterData<AffirmSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<AffirmSyncResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== CAPTURE =====
// POST /api/v1/transactions/{id}/capture  -> returns capture event { id, type, amount }
// `amount` is sent so partial captures are honoured; omit-for-full is not assumed.

#[derive(Debug, Serialize)]
pub struct AffirmCaptureRequest {
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AffirmRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AffirmCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: AffirmRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Send the merchant reference as `order_id` (parity with hyperswitch's Affirm
        // capture, which uses connector_request_reference_id — the same source the
        // Authorize/INITIATE leg uses). Empty reference → omit the field.
        let order_id = match item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone()
        {
            ref_id if ref_id.is_empty() => None,
            ref_id => Some(ref_id),
        };
        Ok(Self {
            amount: item.router_data.request.minor_amount_to_capture,
            order_id,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmCaptureResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub amount: Option<MinorUnit>,
    pub order_id: Option<String>,
}

impl TryFrom<ResponseRouterData<AffirmCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AffirmCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // A successful (HTTP 2xx) capture event settles the funds.
        let connector_transaction_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::ConnectorError::ResponseHandlingFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "Affirm capture response is missing the connector transaction id."
                            .to_string(),
                    ),
                },
            })?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== VOID =====
// POST /api/v1/transactions/{id}/void -> returns void event { id, type, amount }

#[derive(Debug, Serialize)]
pub struct AffirmVoidRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AffirmRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AffirmVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: AffirmRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            reference_id: item.router_data.resource_common_data.reference_id.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmVoidResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub amount: Option<MinorUnit>,
}

impl TryFrom<ResponseRouterData<AffirmVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<AffirmVoidResponse, Self>) -> Result<Self, Self::Error> {
        let connector_transaction_id = item.router_data.request.connector_transaction_id.clone();

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== REFUND =====
// POST /api/v1/transactions/{id}/refund -> returns refund event { id, type, amount }

#[derive(Debug, Serialize)]
pub struct AffirmRefundRequest {
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AffirmRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for AffirmRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: AffirmRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.router_data.request.minor_refund_amount,
            reference_id: Some(item.router_data.request.refund_id.clone()),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmRefundResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub amount: Option<MinorUnit>,
}

impl TryFrom<ResponseRouterData<AffirmRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<AffirmRefundResponse, Self>) -> Result<Self, Self::Error> {
        // A successful (HTTP 2xx) refund event indicates the refund was accepted.
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: RefundStatus::Success,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== RSYNC =====
// GET /api/v1/transactions/{id}?expand=events
// Inspect amount_refunded / events[type=refund] to derive refund completion.

#[derive(Debug, Deserialize, Serialize)]
pub struct AffirmRSyncResponse {
    pub id: String,
    pub status: AffirmTransactionStatus,
    pub amount_refunded: Option<MinorUnit>,
    pub events: Option<Vec<AffirmEvent>>,
}

impl TryFrom<ResponseRouterData<AffirmRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<AffirmRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let connector_refund_id = item.router_data.request.connector_refund_id.clone();

        // The refund is complete when a matching refund event is present in the
        // transaction's events array, or the transaction is marked refunded.
        let refund_event_present = item
            .response
            .events
            .as_ref()
            .map(|events| events.iter().any(|event| event.event_type == "refund"))
            .unwrap_or(false);

        let refund_status =
            if refund_event_present || item.response.status == AffirmTransactionStatus::Refunded {
                RefundStatus::Success
            } else {
                RefundStatus::Pending
            };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}
