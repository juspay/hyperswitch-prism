use common_enums::{Currency, PayoutStatus};
use common_utils::types::{StringMajorUnit, StringMajorUnitForConnector};
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payouts::{
        payout_method_data::{Bank, PayoutMethodData, SepaBankTransfer, Wallet},
        payouts_types::{
            PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutTransferRequest,
            PayoutTransferResponse,
        },
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils::convert_amount,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

const MIFINITY_CONNECTOR: &str = "mifinity";
// MiFinity's account-to-account transfer requires a description of 1-25 chars.
const DEFAULT_DESCRIPTION: &str = "Payout";
const MAX_DESCRIPTION_LEN: usize = 25;

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
    pub fn get_source_account(
        &self,
    ) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
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

type MifinityPayoutRouterData =
    RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>;

/// MiFinity requires a 1-25 character description. Resolve it from the request,
/// falling back to a default, and truncate to stay within the limit.
fn resolve_description(req: &MifinityPayoutRouterData) -> String {
    let mut description = req
        .resource_common_data
        .description
        .clone()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| DEFAULT_DESCRIPTION.to_string());
    description.truncate(MAX_DESCRIPTION_LEN);
    description
}

/// Dispatching request body for the MiFinity PayoutTransfer flow. Serializes as
/// the underlying request (untagged) so each payout method produces its own body.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub enum MifinityPayoutRequest {
    Acct2Acct(MifinityAcct2AcctRequest),
    Pab(MifinityPabRequest),
}

impl TryFrom<&MifinityPayoutRouterData> for MifinityPayoutRequest {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(req: &MifinityPayoutRouterData) -> Result<Self, Self::Error> {
        match req.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Wallet(Wallet::Mifinity(_))) => {
                Ok(Self::Acct2Acct(MifinityAcct2AcctRequest::try_from(req)?))
            }
            Some(PayoutMethodData::Bank(Bank::Sepa(_))) => {
                Ok(Self::Pab(MifinityPabRequest::try_from(req)?))
            }
            Some(_) | None => Err(IntegrationError::connector_feature_not_supported(
                MIFINITY_CONNECTOR,
                "the selected payout method (MiFinity supports the MiFinity wallet and SEPA bank transfer only)",
                Default::default(),
            )
            .into()),
        }
    }
}

/// Request body for the MiFinity account-to-account transfer endpoint
/// (`POST /api/payments/acct2acct`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityAcct2AcctRequest {
    /// Merchant account debited for the transfer (from the connector config).
    pub source_account: Secret<String>,
    /// Recipient MiFinity wallet: email address or MiFinity account number.
    pub destination_account: Secret<String>,
    pub money: MifinityMoney,
    /// Transfer description (1-25 characters).
    pub description: String,
    /// Caller-assigned unique correlation id, echoed back and used for sync.
    pub trace_id: String,
}

impl TryFrom<&MifinityPayoutRouterData> for MifinityAcct2AcctRequest {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(req: &MifinityPayoutRouterData) -> Result<Self, Self::Error> {
        let auth = MifinityAuthType::try_from(&req.connector_config)?;
        let source_account = auth.get_source_account()?;

        let destination_account = match req.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Wallet(Wallet::Mifinity(data))) => {
                data.destination_account.clone()
            }
            Some(_) | None => {
                return Err(IntegrationError::connector_feature_not_supported(
                    MIFINITY_CONNECTOR,
                    "the selected payout method (MiFinity account-to-account transfer supports the MiFinity wallet only)",
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

        Ok(Self {
            source_account,
            destination_account,
            money: MifinityMoney {
                amount,
                currency: req.request.destination_currency,
            },
            description: resolve_description(req),
            trace_id: req
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

/// Request body for the MiFinity PayAnyBank (PAB) endpoint
/// (`POST /api/payments/pab`), used for SEPA bank payouts.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityPabRequest {
    /// Merchant account debited for the transfer (from the connector config).
    pub source_account: Secret<String>,
    /// Caller-assigned unique correlation id, echoed back and used for sync.
    pub trace_id: String,
    /// Transfer description (1-25 characters).
    pub description: String,
    pub money: MifinityMoney,
    pub bank_payee: MifinityBankPayee,
}

/// Recipient bank details for a PAB payout.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityBankPayee {
    /// Recipient bank country (ISO 3166-1 alpha-2).
    pub country: String,
    pub currency: Currency,
    pub description: String,
    pub fields: MifinityBankFields,
}

/// MiFinity's typed bank-field bag for SEPA payouts.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MifinityBankFields {
    #[serde(rename = "IBAN")]
    pub iban: Secret<String>,
    #[serde(rename = "BIC", skip_serializing_if = "Option::is_none")]
    pub bic: Option<Secret<String>>,
    #[serde(rename = "BANK_NAME", skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<String>,
    #[serde(rename = "CUSTOMER_NAME")]
    pub customer_name: Secret<String>,
    #[serde(rename = "CUSTOMER_ADDRESS")]
    pub customer_address: Secret<String>,
    #[serde(rename = "CUSTOMER_CITY")]
    pub customer_city: Secret<String>,
    #[serde(rename = "CUSTOMER_ZIP")]
    pub customer_zip: Secret<String>,
}

impl TryFrom<&MifinityPayoutRouterData> for MifinityPabRequest {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(req: &MifinityPayoutRouterData) -> Result<Self, Self::Error> {
        let auth = MifinityAuthType::try_from(&req.connector_config)?;
        let source_account = auth.get_source_account()?;

        let sepa: &SepaBankTransfer = match req.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Bank(Bank::Sepa(sepa))) => sepa,
            Some(_) | None => {
                return Err(IntegrationError::connector_feature_not_supported(
                    MIFINITY_CONNECTOR,
                    "the selected payout method (MiFinity PayAnyBank supports SEPA bank transfers only)",
                    Default::default(),
                )
                .into());
            }
        };

        // Recipient address (customerAddress/City/Zip) is NOT carried on the
        // SEPA method data — it comes from the payout's billing address.
        let billing = req
            .request
            .address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.address.as_ref());

        let missing =
            |field: &'static str, ctx: &'static str| IntegrationError::MissingRequiredField {
                field_name: field,
                context: IntegrationErrorContext {
                    additional_context: Some(ctx.to_string()),
                    ..Default::default()
                },
            };

        let country = billing
            .and_then(|d| d.country)
            .map(|c| c.to_string())
            .ok_or_else(|| {
                missing(
                    "address.billing_address.address.country",
                    "MiFinity PayAnyBank requires the recipient bank country (ISO 3166-1).",
                )
            })?;

        // Recipient name: prefer the SEPA account holder, else the billing name.
        let customer_name = sepa
            .account_holder_name
            .clone()
            .or_else(|| billing.and_then(|d| d.get_optional_full_name()))
            .ok_or_else(|| {
                missing(
                    "account_holder_name",
                    "MiFinity PayAnyBank requires the recipient name (account_holder_name or billing name).",
                )
            })?;

        let customer_address = billing.and_then(|d| d.line1.clone()).ok_or_else(|| {
            missing(
                "address.billing_address.address.line1",
                "MiFinity PayAnyBank requires the recipient street address (billing line1).",
            )
        })?;

        let customer_city = billing.and_then(|d| d.city.clone()).ok_or_else(|| {
            missing(
                "address.billing_address.address.city",
                "MiFinity PayAnyBank requires the recipient city (billing city).",
            )
        })?;

        let customer_zip = billing.and_then(|d| d.zip.clone()).ok_or_else(|| {
            missing(
                "address.billing_address.address.zip",
                "MiFinity PayAnyBank requires the recipient postal code (billing zip).",
            )
        })?;

        let amount = convert_amount(
            &StringMajorUnitForConnector,
            req.request.amount,
            req.request.destination_currency,
        )?;

        let description = resolve_description(req);

        Ok(Self {
            source_account,
            trace_id: req
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            description: description.clone(),
            money: MifinityMoney {
                amount,
                currency: req.request.destination_currency,
            },
            bank_payee: MifinityBankPayee {
                country,
                currency: req.request.destination_currency,
                description,
                fields: MifinityBankFields {
                    iban: sepa.iban.clone(),
                    bic: sepa.bic.clone(),
                    bank_name: sepa.bank_name.map(|b| b.to_string()),
                    customer_name,
                    customer_address,
                    customer_city,
                    customer_zip,
                },
            },
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

/// One entry from a MiFinity payout response payload (shared by acct2acct and PAB).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MifinityPayoutPayload {
    pub transaction_id: String,
    pub transaction_reference: Option<String>,
    pub trace_id: Option<String>,
    pub date_posted: Option<String>,
    pub source_money: Option<MifinityMoneyResponse>,
    pub destination_money: Option<MifinityMoneyResponse>,
    pub status: Option<String>,
}

/// Response body for the MiFinity payout endpoints (acct2acct and PAB share this shape).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MifinityPayoutResponse {
    pub payload: Vec<MifinityPayoutPayload>,
}

impl TryFrom<ResponseRouterData<MifinityPayoutResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MifinityPayoutResponse, Self>,
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
