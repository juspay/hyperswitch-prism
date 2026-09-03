use common_enums::{Currency, PayoutStatus};
use common_utils::types::{StringMajorUnit, StringMajorUnitForConnector};
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payouts::{
        payout_method_data::{CardPayout, PayoutMethodData},
        payouts_types::{
            PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
            PayoutTransferResponse,
        },
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils::convert_amount,
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

const MIFINITY_CONNECTOR: &str = "mifinity";
// Fallback date of birth used when the payout request does not carry the
// cardholder DOB, which MiFinity's PayMyCard endpoint requires (YYYY-MM-DD).
const DEFAULT_DOB: &str = "1990-01-01";

/// Auth material resolved from the connector configuration for the Mifinity payout connector.
pub struct MifinityAuthType {
    pub key: Secret<String>,
    pub source_account: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for MifinityAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Mifinity {
                key,
                destination_account_number,
                ..
            } => Ok(Self {
                key: key.clone(),
                source_account: destination_account_number.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "MiFinity payouts require ConnectorSpecificConfig::Mifinity with the merchant `key` supplied via x-connector-config."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }
            .into()),
        }
    }
}

impl MifinityAuthType {
    pub fn get_source_account(&self) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
        self.source_account.clone().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "destination_account_number",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "MiFinity payouts require the merchant source account (mapped from `destination_account_number` in the connector config) to debit funds from."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }
            .into()
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MifinityMoney {
    pub amount: StringMajorUnit,
    pub currency: Currency,
}

/// Request body for the MiFinity PayMyCard (PMC) card payout endpoint.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityPmcRequest {
    pub money: MifinityMoney,
    pub source_account: Secret<String>,
    pub trace_id: String,
    pub card_name: Secret<String>,
    pub card_number: Secret<String>,
    pub expiry_date: Secret<String>,
    pub card_holder_country_code: String,
    pub card_holder_nationality: String,
    pub dob: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_state: Option<Secret<String>>,
}

fn build_expiry_date(card: &CardPayout) -> Secret<String> {
    let month = card.expiry_month.peek().clone();
    let year = card.expiry_year.peek().clone();
    // MiFinity expects MM/YY.
    let yy = if year.len() > 2 {
        year[year.len() - 2..].to_string()
    } else {
        year
    };
    Secret::new(format!("{month}/{yy}"))
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for MifinityPmcRequest
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
        let auth = MifinityAuthType::try_from(&req.connector_config)?;
        let source_account = auth.get_source_account()?;

        let card = match req.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Card(card)) => card,
            Some(_) | None => {
                return Err(IntegrationError::connector_feature_not_supported(
                    MIFINITY_CONNECTOR,
                    "the selected payout method (MiFinity PayMyCard supports card payouts only)",
                    Default::default(),
                )
                .into());
            }
        };

        let amount = convert_amount(
            &StringMajorUnitForConnector,
            req.request.amount,
            req.request.destination_currency,
        )?;

        // Cardholder name: prefer the name on the card, else the billing name.
        let card_name = card
            .card_holder_name
            .clone()
            .or_else(|| {
                req.request
                    .address
                    .as_ref()
                    .and_then(|a| a.billing_address.as_ref())
                    .and_then(|b| b.address.as_ref())
                    .and_then(|d| match (d.first_name.as_ref(), d.last_name.as_ref()) {
                        (Some(first), Some(last)) => Some(Secret::new(format!(
                            "{} {}",
                            first.clone().expose(),
                            last.clone().expose()
                        ))),
                        (Some(name), None) | (None, Some(name)) => Some(name.clone()),
                        (None, None) => None,
                    })
            })
            .or_else(|| {
                req.request
                    .customer
                    .as_ref()
                    .and_then(|c| c.name.clone())
                    .map(Secret::new)
            })
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "card_holder_name",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "MiFinity PayMyCard requires a cardholder name (card_holder_name or billing name)."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let billing_details = req
            .request
            .address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.address.as_ref());

        let country_code = billing_details
            .and_then(|d| d.country)
            .map(|c| c.to_string())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "address.billing_address.address.country",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "MiFinity PayMyCard requires the cardholder country code (ISO 3166-1)."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let card_holder_street = billing_details.and_then(|d| d.line1.clone());
        let card_holder_city = billing_details
            .and_then(|d| d.city.clone())
            .map(|c| c.expose());
        let card_holder_state = billing_details.and_then(|d| d.state.clone());

        Ok(Self {
            money: MifinityMoney {
                amount,
                currency: req.request.destination_currency,
            },
            source_account,
            trace_id: req
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            card_name,
            card_number: Secret::new(card.card_number.get_card_no()),
            expiry_date: build_expiry_date(card),
            card_holder_country_code: country_code.clone(),
            card_holder_nationality: country_code,
            dob: DEFAULT_DOB.to_string(),
            description: req.resource_common_data.description.clone(),
            card_holder_street,
            card_holder_city,
            card_holder_state,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MifinityMoneyResponse {
    pub amount: Option<serde_json::Value>,
    pub currency: Option<String>,
    #[serde(alias = "presentationAmount", alias = "displayable")]
    pub presentation_amount: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityPmcPayload {
    pub transaction_id: String,
    pub transaction_reference: Option<String>,
    pub trace_id: Option<String>,
    pub date_posted: Option<String>,
    pub source_money: Option<MifinityMoneyResponse>,
    pub destination_money: Option<MifinityMoneyResponse>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MifinityPmcResponse {
    pub payload: Vec<MifinityPmcPayload>,
}

impl TryFrom<ResponseRouterData<MifinityPmcResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MifinityPmcResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // A synchronous 200 with a populated payload confirms the payout was
        // accepted/initiated. Final settlement (PROCESSED_BY_ACQUIRER) is
        // confirmed asynchronously via callback or the status-sync endpoint.
        let payout = item.response.payload.first();
        let payout_status = PayoutStatus::Initiated;
        let connector_payout_id = payout.map(|p| p.transaction_id.clone());

        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status,
                connector_payout_id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MifinityErrorDetail {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub form_object_name: Option<String>,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MifinityErrorResponse {
    pub errors: Vec<MifinityErrorDetail>,
}

// ===== PAYOUT GET / STATUS SYNC (GET /api/transactions/status/{traceId}) =====

/// One entry from the MiFinity transaction-status endpoint payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityStatusPayload {
    pub transaction_reference: Option<String>,
    /// Numeric status code (see [`map_mifinity_status`]).
    pub transaction_status: Option<i32>,
    pub transaction_status_description: Option<String>,
    pub transaction_last_updated: Option<String>,
    pub trace_id: Option<String>,
}

/// Response body for the MiFinity transaction-status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MifinityStatusResponse {
    pub payload: Vec<MifinityStatusPayload>,
}

/// Maps MiFinity's numeric `transactionStatus` code to a payout status.
///
/// | Code | Description            | Payout Status |
/// |------|------------------------|---------------|
/// | 1    | RECEIVED               | Pending       |
/// | 2    | INTERNAL_ERROR         | Failure       |
/// | 3    | SUBMITTED              | Pending       |
/// | 5    | PROCESSED_BY_ACQUIRER  | Success       |
/// | 6    | REJECTED               | Failure       |
/// | 7    | IN_PROGRESS            | Pending       |
/// | 8    | ON_HOLD_KYC            | Pending       |
fn map_mifinity_status(code: Option<i32>) -> PayoutStatus {
    match code {
        Some(5) => PayoutStatus::Success,
        Some(2) | Some(6) => PayoutStatus::Failure,
        // 1 RECEIVED, 3 SUBMITTED, 7 IN_PROGRESS, 8 ON_HOLD_KYC and any
        // unknown/absent code are treated as non-terminal (still pending).
        _ => PayoutStatus::Pending,
    }
}

impl TryFrom<ResponseRouterData<MifinityStatusResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MifinityStatusResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let entry = item.response.payload.first();
        let payout_status = map_mifinity_status(entry.and_then(|p| p.transaction_status));
        let connector_payout_id = entry
            .and_then(|p| p.transaction_reference.clone())
            .or_else(|| item.router_data.request.connector_payout_id.clone());

        Ok(Self {
            response: Ok(PayoutGetResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status,
                connector_payout_id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
