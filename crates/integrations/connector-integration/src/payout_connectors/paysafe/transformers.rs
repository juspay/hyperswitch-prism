use super::PaysafePayoutsRouterData;
use crate::types::ResponseRouterData;
use domain_types::{
    connector_flow::{PayoutCreateRecipient, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
    payouts::{
        payout_method_data::{GiftCardPayout, PayoutMethodData},
        payouts_types::{
            PayoutCreateRecipientRequest, PayoutCreateRecipientResponse, PayoutFlowData,
            PayoutTransferRequest, PayoutTransferResponse,
        },
    },
    router_data_v2::RouterDataV2,
};
use error_stack::Report;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

// ===== SHARED TYPES =====

fn unsupported_payout_method_error(flow: &str, supported: &str) -> Report<IntegrationError> {
    IntegrationError::NotSupported {
        message: "Payout method is not supported".to_string(),
        connector: "Paysafe",
        context: IntegrationErrorContext {
            additional_context: Some(format!(
                "Paysafe {flow} - only the {supported} payout method is supported"
            )),
            suggested_action: Some(format!("Use a {supported} payout method")),
            doc_url: None,
        },
    }
    .into()
}

fn missing_field(field_name: &'static str, flow: &str) -> Report<IntegrationError> {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            additional_context: Some(format!("Paysafe {flow} - missing required field")),
            suggested_action: None,
            doc_url: None,
        },
    }
    .into()
}

/// Payout connector metadata threaded from `PayoutCreateRecipient` into
/// `PayoutTransfer`: the payment-handle token minted for the standalone credit.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct PaysafePaymentHandleMeta {
    payment_handle_token: Secret<String>,
}

fn to_payment_handle_meta(
    connector_meta: Option<serde_json::Value>,
) -> Result<PaysafePaymentHandleMeta, Report<IntegrationError>> {
    let json = connector_meta
        .ok_or_else(|| missing_field("payout_connector_metadata", "Payout Transfer"))?;
    serde_json::from_value(json).map_err(|_| {
        IntegrationError::InvalidDataFormat {
            field_name: "payout_connector_metadata",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Paysafe Payout Transfer - failed to parse payout_connector_metadata"
                        .to_string(),
                ),
                suggested_action: None,
                doc_url: None,
            },
        }
        .into()
    })
}

/// `transactionType` for a payment handle that only funds a matching standalone
/// credit (the Paysafe payout rail for paysafecard consumers).
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum PaysafeTransactionType {
    StandaloneCredit,
}

/// `paymentType` on the payment handle / standalone credit.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
enum PaysafePaymentType {
    Paysafecard,
}

// ===== PAYOUT CREATE RECIPIENT (mint the payment handle) =====

/// `POST v1/paymenthandles` body for paysafecard payouts. The handle carries the
/// `paymentHandleToken` that the later standalone credit references.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentHandleRequest {
    merchant_ref_num: String,
    transaction_type: PaysafeTransactionType,
    currency_code: common_enums::Currency,
    amount: i64,
    payment_type: PaysafePaymentType,
    dup_check: bool,
    paysafecard: PaysafePaysafecardPayout,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_ip: Option<String>,
}

/// The paysafecard LPM block on the payment handle. `consumer_id` is the
/// "my paysafecard" account id of the recipient (required by PaymentHUB; unlike
/// a payments handle there is no funding card inside).
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaysafecardPayout {
    consumer_id: Secret<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafePayoutsRouterData<
            RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
            T,
        >,
    > for PaysafePaymentHandleRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PaysafePayoutsRouterData<
            RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item.router_data;
        let gift_card = match item.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::GiftCard(GiftCardPayout::PaySafeCard(card))) => card,
            _ => {
                return Err(unsupported_payout_method_error(
                    "Payout Create Recipient",
                    "gift card (paysafecard account)",
                ))
            }
        };

        let consumer_id = gift_card.consumer_id.clone().ok_or_else(|| {
            missing_field("payout_method_data.consumer_id", "Payout Create Recipient")
        })?;

        let merchant_ref_num = item.request.merchant_payout_id.clone().unwrap_or(
            item.resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        Ok(Self {
            merchant_ref_num,
            transaction_type: PaysafeTransactionType::StandaloneCredit,
            currency_code: item.request.source_currency,
            amount: item.request.amount.get_amount_as_i64(),
            payment_type: PaysafePaymentType::Paysafecard,
            dup_check: true,
            paysafecard: PaysafePaysafecardPayout { consumer_id },
            customer_ip: None,
        })
    }
}

/// The standalone-credit pending state PaymentHUB reports for a minted handle.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeHandleStatus {
    Initiated,
    Ready,
    Payable,
    Completed,
    Cancelled,
    Failed,
    Expired,
}

/// `POST v1/paymenthandles` response.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentHandleResponse {
    pub id: String,
    pub status: PaysafeHandleStatus,
    pub payment_handle_token: Secret<String>,
}

impl TryFrom<ResponseRouterData<PaysafePaymentHandleResponse, Self>>
    for RouterDataV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafePaymentHandleResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_status = match item.response.status {
            PaysafeHandleStatus::Failed => common_enums::PayoutStatus::Failure,
            _ => common_enums::PayoutStatus::RequiresFulfillment,
        };

        let payout_connector_metadata = Some(Secret::new(serde_json::json!({
            "payment_handle_token": item.response.payment_handle_token,
        })));

        Ok(Self {
            response: Ok(PayoutCreateRecipientResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status,
                // The handle id coordinates with the standalone credit's sync/webhooks
                // only loosely; the transfer's own id is the authoritative reference.
                connector_payout_id: Some(item.response.id),
                status_code: item.http_code,
                payout_connector_metadata,
            }),
            ..item.router_data
        })
    }
}

// ===== PAYOUT TRANSFER (fund the standalone credit) =====

/// `POST v1/standalonecredits` body. The handle minted by the create-recipient
/// leg is referenced through its `paymentHandleToken`; the credit then credits
/// the paysafecard account itself, with no further authorization needed.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeStandaloneCreditRequest {
    merchant_ref_num: String,
    amount: i64,
    currency_code: common_enums::Currency,
    payment_type: PaysafePaymentType,
    dup_check: bool,
    settle_with_auth: bool,
    payment_handle_token: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer_ip: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafePayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            T,
        >,
    > for PaysafeStandaloneCreditRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PaysafePayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item.router_data;
        match item.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::GiftCard(_)) => {}
            _ => {
                return Err(unsupported_payout_method_error(
                    "Payout Transfer",
                    "gift card (paysafecard account)",
                ))
            }
        }

        let handle_meta = to_payment_handle_meta(
            item.request
                .payout_connector_metadata
                .clone()
                .map(|secret| secret.expose()),
        )?;

        let merchant_ref_num = item.request.merchant_payout_id.clone().unwrap_or(
            item.resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        Ok(Self {
            merchant_ref_num,
            amount: item.request.amount.get_amount_as_i64(),
            currency_code: item.request.destination_currency,
            payment_type: PaysafePaymentType::Paysafecard,
            dup_check: true,
            // The standalone credit settles on creation; a separate auth leg is
            // not part of the payout flow.
            settle_with_auth: false,
            payment_handle_token: handle_meta.payment_handle_token,
            customer_ip: None,
        })
    }
}

/// Status of a standalone credit as reported by PaymentHUB.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeTransactionRequestStatus {
    Received,
    Initiated,
    Pending,
    Failed,
    Cancelled,
    Expired,
    Completed,
}

impl From<PaysafeTransactionRequestStatus> for common_enums::PayoutStatus {
    fn from(item: PaysafeTransactionRequestStatus) -> Self {
        match item {
            PaysafeTransactionRequestStatus::Received
            | PaysafeTransactionRequestStatus::Initiated => Self::Initiated,
            PaysafeTransactionRequestStatus::Pending => Self::Pending,
            PaysafeTransactionRequestStatus::Completed => Self::Success,
            PaysafeTransactionRequestStatus::Failed => Self::Failure,
            PaysafeTransactionRequestStatus::Cancelled => Self::Cancelled,
            PaysafeTransactionRequestStatus::Expired => Self::Expired,
        }
    }
}

/// `POST v1/standalonecredits` response.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeStandaloneCreditResponse {
    pub id: String,
    pub status: PaysafeTransactionRequestStatus,
    /// Embedded error block surfaced by PaymentHUB on some non-`FAILED` statuses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

impl TryFrom<ResponseRouterData<PaysafeStandaloneCreditResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeStandaloneCreditResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: common_enums::PayoutStatus::from(item.response.status),
                connector_payout_id: Some(item.response.id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
