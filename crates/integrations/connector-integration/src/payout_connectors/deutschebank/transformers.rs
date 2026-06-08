use crate::types::ResponseRouterData;
use common_enums::PayoutStatus;
use common_utils::types::FloatMajorUnitForConnector;
use domain_types::{
    connector_flow::{PayoutEligibility, PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payouts::{
        payout_method_data::{Bank, PayoutMethodData, SepaBankTransfer},
        payouts_types::{
            PayoutEligibilityRequest, PayoutEligibilityResponse, PayoutFlowData, PayoutGetRequest,
            PayoutGetResponse, PayoutTransferRequest, PayoutTransferResponse,
        },
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    utils,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

const DEUTSCHEBANK_BICFI: &str = "DEUTDEDDXXX";
const VOP_ID_SEPARATOR: char = '|';

// ===== AUTH TYPE =====

#[derive(Debug, Clone)]
pub struct DeutschebankAuthType {
    pub customer_identifier: Secret<String>,
    pub consumer_identifier: Secret<String>,
    pub key_id: Secret<String>,
    pub signing_private_key: Secret<String>,
    pub client_certificate: Secret<String>,
    pub client_certificate_key: Secret<String>,
    pub server_ca_bundle: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for DeutschebankAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Deutschebank {
                customer_identifier,
                consumer_identifier,
                key_id,
                signing_private_key,
                client_certificate,
                client_certificate_key,
                server_ca_bundle,
                ..
            } => Ok(Self {
                customer_identifier: customer_identifier.to_owned(),
                consumer_identifier: consumer_identifier.to_owned(),
                key_id: key_id.to_owned(),
                signing_private_key: signing_private_key.to_owned(),
                client_certificate: client_certificate.to_owned(),
                client_certificate_key: client_certificate_key.to_owned(),
                server_ca_bundle: server_ca_bundle.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeutschebankErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
    #[serde(rename = "errorCode")]
    pub error_code: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "errors")]
    pub errors: Option<Vec<DeutschebankErrorEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeutschebankErrorEntry {
    pub code: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
}

// ===== VoP (Verification of Payee) =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DeutschebankVopMatchStatus {
    Mtch,
    Cmtc,
    Noap,
    Nmtc,
}

impl From<DeutschebankVopMatchStatus> for PayoutStatus {
    fn from(value: DeutschebankVopMatchStatus) -> Self {
        match value {
            DeutschebankVopMatchStatus::Mtch | DeutschebankVopMatchStatus::Cmtc => {
                Self::RequiresFulfillment
            }
            DeutschebankVopMatchStatus::Noap | DeutschebankVopMatchStatus::Nmtc => Self::Ineligible,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DeutschebankVopRequest {
    pub payee: DeutschebankVopPayee,
    #[serde(rename = "payeeAccount")]
    pub payee_account: DeutschebankVopIbanAccount,
    pub debtor: DeutschebankVopDebtor,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankVopPayee {
    pub name: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankVopIbanAccount {
    pub iban: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankVopDebtor {
    #[serde(rename = "debtorAccount")]
    pub debtor_account: DeutschebankVopIbanAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeutschebankVopResponse {
    #[serde(
        rename = "payeeNameMatch",
        alias = "match",
        alias = "result",
        alias = "matchStatus"
    )]
    pub match_status: Option<DeutschebankVopMatchStatus>,
    #[serde(
        rename = "additionalInfo",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_info: Option<String>,
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    > for DeutschebankVopRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let payee_iban = extract_payee_iban(req.request.payout_method_data.as_ref())?;
        let debtor_iban = extract_debtor_iban(req.request.source_bank_data.as_ref())?;
        let payee_name = req
            .request
            .customer
            .as_ref()
            .and_then(|c| c.name.clone())
            .map(Secret::new)
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "customer.name",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Payee name is required for Deutsche Bank VoP check".to_string(),
                        ),
                        ..Default::default()
                    },
                })
            })?;

        Ok(Self {
            payee: DeutschebankVopPayee { name: payee_name },
            payee_account: DeutschebankVopIbanAccount { iban: payee_iban },
            debtor: DeutschebankVopDebtor {
                debtor_account: DeutschebankVopIbanAccount { iban: debtor_iban },
            },
        })
    }
}

pub fn derive_vop_id(merchant_id: &str, connector_request_reference_id: &str) -> String {
    let salted = format!("{merchant_id}:{connector_request_reference_id}");
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, salted.as_bytes()).to_string()
}

pub fn build_eligibility_response(
    vop_body: DeutschebankVopResponse,
    vop_id: String,
    http_code: u16,
) -> Result<PayoutEligibilityResponse, error_stack::Report<ConnectorError>> {
    let match_status =
        vop_body
            .match_status
            .ok_or_else(|| ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;
    let payout_status = PayoutStatus::from(match_status);
    let is_eligible = matches!(
        match_status,
        DeutschebankVopMatchStatus::Mtch | DeutschebankVopMatchStatus::Cmtc
    );

    let connector_payout_id = if is_eligible { Some(vop_id) } else { None };

    Ok(PayoutEligibilityResponse {
        merchant_payout_id: None,
        payout_status,
        connector_payout_id,
        payout_eligible: Some(is_eligible),
        status_code: http_code,
    })
}

impl TryFrom<ResponseRouterData<PayoutEligibilityResponse, Self>>
    for RouterDataV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayoutEligibilityResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(item.response),
            ..item.router_data
        })
    }
}

// ===== Payment Status =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DeutschebankPaymentStatus {
    Accp,
    Pdng,
    Rjct,
}

impl From<DeutschebankPaymentStatus> for PayoutStatus {
    fn from(value: DeutschebankPaymentStatus) -> Self {
        match value {
            DeutschebankPaymentStatus::Accp => Self::Success,
            DeutschebankPaymentStatus::Pdng => Self::Pending,
            DeutschebankPaymentStatus::Rjct => Self::Failure,
        }
    }
}

// ===== Initiate Payment =====

#[derive(Debug, Serialize)]
pub struct DeutschebankSepaPaymentRequest {
    #[serde(rename = "customerCreditTransferInitiation")]
    pub customer_credit_transfer_initiation: DeutschebankCustomerCreditTransferInitiation,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankCustomerCreditTransferInitiation {
    #[serde(rename = "groupHeader")]
    pub group_header: DeutschebankGroupHeader,
    #[serde(rename = "paymentInformation")]
    pub payment_information: Vec<DeutschebankPaymentInformation>,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankGroupHeader {
    #[serde(rename = "messageIdentification")]
    pub message_identification: String,
    #[serde(rename = "creationDateTime")]
    pub creation_date_time: String,
    #[serde(rename = "controlSum")]
    pub control_sum: common_utils::types::FloatMajorUnit,
    #[serde(rename = "numberOfTransactions")]
    pub number_of_transactions: String,
    #[serde(rename = "initiatingParty")]
    pub initiating_party: DeutschebankParty,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankParty {
    pub name: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankPaymentInformation {
    #[serde(rename = "paymentInformationIdentification")]
    pub payment_information_identification: String,
    #[serde(rename = "paymentMethod")]
    pub payment_method: &'static str,
    #[serde(rename = "batchBooking")]
    pub batch_booking: bool,
    #[serde(rename = "controlSum")]
    pub control_sum: common_utils::types::FloatMajorUnit,
    #[serde(rename = "numberOfTransactions")]
    pub number_of_transactions: String,
    #[serde(rename = "paymentTypeInformation")]
    pub payment_type_information: DeutschebankPaymentTypeInformation,
    #[serde(rename = "requestedExecutionDate")]
    pub requested_execution_date: DeutschebankExecutionDate,
    pub debtor: DeutschebankParty,
    #[serde(rename = "debtorAccount")]
    pub debtor_account: DeutschebankAccount,
    #[serde(rename = "debtorAgent")]
    pub debtor_agent: DeutschebankAgent,
    #[serde(rename = "creditTransferTransactionInformation")]
    pub credit_transfer_transaction_information: Vec<DeutschebankCreditTransfer>,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankPaymentTypeInformation {
    #[serde(rename = "serviceLevel")]
    pub service_level: DeutschebankCode,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankCode {
    pub code: &'static str,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankExecutionDate {
    pub date: String,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankAccount {
    pub identification: DeutschebankIbanIdentification,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankIbanIdentification {
    pub iban: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankAgent {
    #[serde(rename = "financialInstitutionIdentification")]
    pub financial_institution_identification: DeutschebankBic,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankBic {
    pub bicfi: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankCreditTransfer {
    pub purpose: DeutschebankPurpose,
    #[serde(rename = "paymentIdentification")]
    pub payment_identification: DeutschebankPaymentIdentification,
    pub amount: DeutschebankAmountWrapper,
    pub creditor: DeutschebankParty,
    #[serde(rename = "creditorAccount")]
    pub creditor_account: DeutschebankAccount,
    #[serde(rename = "creditorAgent")]
    pub creditor_agent: DeutschebankAgent,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankPurpose {
    pub proprietary: String,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankPaymentIdentification {
    #[serde(rename = "endToEndIdentification")]
    pub end_to_end_identification: String,
    #[serde(rename = "instructionIdentification")]
    pub instruction_identification: String,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankAmountWrapper {
    #[serde(rename = "instructedAmount")]
    pub instructed_amount: DeutschebankInstructedAmount,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankInstructedAmount {
    pub currency: common_enums::Currency,
    pub value: common_utils::types::FloatMajorUnit,
}

pub struct DeutschebankSepaPaymentBuilt {
    pub request: DeutschebankSepaPaymentRequest,
    pub end_to_end_id: String,
    pub debtor_iban: Secret<String>,
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for DeutschebankSepaPaymentBuilt
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
        let creditor_iban = extract_payee_iban(req.request.payout_method_data.as_ref())?;
        let creditor_bic = extract_payee_bic(req.request.payout_method_data.as_ref())?;
        let debtor_iban = extract_debtor_iban(req.request.source_bank_data.as_ref())?;
        let creditor_name = req
            .request
            .customer
            .as_ref()
            .and_then(|c| c.name.clone())
            .map(Secret::new)
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "customer.name",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Creditor name is required for Deutsche Bank SEPA payment".to_string(),
                        ),
                        ..Default::default()
                    },
                })
            })?;

        let reference = req
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let end_to_end_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            format!("dbank-e2e:{reference}").as_bytes(),
        )
        .simple()
        .to_string()
        .to_uppercase();
        let message_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            format!("dbank-msg:{reference}").as_bytes(),
        )
        .simple()
        .to_string();
        let creation_date_time = current_iso_utc_seconds();
        let execution_date = current_iso_date();

        let amount = utils::convert_amount(
            &FloatMajorUnitForConnector,
            req.request.amount,
            req.request.destination_currency,
        )?;
        let purpose_code = req
            .resource_common_data
            .description
            .clone()
            .unwrap_or_else(|| "PAYOUT".to_string());

        let request = DeutschebankSepaPaymentRequest {
            customer_credit_transfer_initiation: DeutschebankCustomerCreditTransferInitiation {
                group_header: DeutschebankGroupHeader {
                    message_identification: message_id.clone(),
                    creation_date_time,
                    control_sum: amount,
                    number_of_transactions: "1".to_string(),
                    initiating_party: DeutschebankParty {
                        name: creditor_name.clone(),
                    },
                },
                payment_information: vec![DeutschebankPaymentInformation {
                    payment_information_identification: message_id.clone(),
                    payment_method: "TRF",
                    batch_booking: false,
                    control_sum: amount,
                    number_of_transactions: "1".to_string(),
                    payment_type_information: DeutschebankPaymentTypeInformation {
                        service_level: DeutschebankCode { code: "SEPA" },
                    },
                    requested_execution_date: DeutschebankExecutionDate {
                        date: execution_date,
                    },
                    debtor: DeutschebankParty {
                        name: creditor_name.clone(),
                    },
                    debtor_account: DeutschebankAccount {
                        identification: DeutschebankIbanIdentification {
                            iban: debtor_iban.clone(),
                        },
                        currency: Some(req.request.source_currency),
                    },
                    debtor_agent: DeutschebankAgent {
                        financial_institution_identification: DeutschebankBic {
                            bicfi: Secret::new(DEUTSCHEBANK_BICFI.to_string()),
                        },
                    },
                    credit_transfer_transaction_information: vec![DeutschebankCreditTransfer {
                        purpose: DeutschebankPurpose {
                            proprietary: purpose_code,
                        },
                        payment_identification: DeutschebankPaymentIdentification {
                            end_to_end_identification: end_to_end_id.clone(),
                            instruction_identification: message_id.clone(),
                        },
                        amount: DeutschebankAmountWrapper {
                            instructed_amount: DeutschebankInstructedAmount {
                                currency: req.request.destination_currency,
                                value: amount,
                            },
                        },
                        creditor: DeutschebankParty {
                            name: creditor_name,
                        },
                        creditor_account: DeutschebankAccount {
                            identification: DeutschebankIbanIdentification {
                                iban: creditor_iban,
                            },
                            currency: None,
                        },
                        creditor_agent: DeutschebankAgent {
                            financial_institution_identification: DeutschebankBic {
                                bicfi: creditor_bic,
                            },
                        },
                    }],
                }],
            },
        };

        Ok(Self {
            request,
            end_to_end_id,
            debtor_iban,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeutschebankSepaPaymentResponse {
    #[serde(rename = "customerPaymentStatusReport")]
    pub customer_payment_status_report: Option<DeutschebankCustomerPaymentStatusReport>,
    #[serde(rename = "transactionStatus", alias = "status")]
    pub top_level_status: Option<DeutschebankPaymentStatus>,
}

impl DeutschebankSepaPaymentResponse {
    pub fn extract_status(&self) -> Option<DeutschebankPaymentStatus> {
        self.customer_payment_status_report
            .as_ref()
            .and_then(|r| r.original_payment_information_and_status.first())
            .and_then(|p| p.transaction_information_and_status.first())
            .and_then(|t| t.transaction_status)
            .or(self.top_level_status)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeutschebankCustomerPaymentStatusReport {
    #[serde(rename = "originalPaymentInformationAndStatus", default)]
    pub original_payment_information_and_status:
        Vec<DeutschebankOriginalPaymentInformationAndStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeutschebankOriginalPaymentInformationAndStatus {
    #[serde(rename = "transactionInformationAndStatus", default)]
    pub transaction_information_and_status: Vec<DeutschebankTransactionInformationAndStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeutschebankTransactionInformationAndStatus {
    #[serde(rename = "transactionStatus")]
    pub transaction_status: Option<DeutschebankPaymentStatus>,
    #[serde(rename = "originalEndToEndIdentification")]
    pub original_end_to_end_identification: Option<String>,
}

pub fn encode_connector_payout_id(end_to_end_id: &str, debtor_iban: &Secret<String>) -> String {
    format!("{end_to_end_id}{VOP_ID_SEPARATOR}{}", debtor_iban.peek())
}

pub fn decode_connector_payout_id(
    value: &str,
) -> Result<(String, Secret<String>), error_stack::Report<IntegrationError>> {
    let (end_to_end_id, iban) = value.split_once(VOP_ID_SEPARATOR).ok_or_else(|| {
        error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "connector_payout_id",
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "Expected `<endToEndId>{VOP_ID_SEPARATOR}<debtorIban>` for Deutsche Bank status enquiry"
                )),
                ..Default::default()
            },
        })
    })?;
    Ok((end_to_end_id.to_string(), Secret::new(iban.to_string())))
}

// ===== Status Enquiry =====

#[derive(Debug, Serialize)]
pub struct DeutschebankStatusRequest {
    #[serde(rename = "debtorAccount")]
    pub debtor_account: DeutschebankStatusDebtorAccount,
    #[serde(rename = "endToEndIdentification")]
    pub end_to_end_identification: String,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankStatusDebtorAccount {
    pub identification: DeutschebankIbanIdentification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeutschebankStatusResponse {
    #[serde(rename = "customerPaymentStatusReport")]
    pub customer_payment_status_report: Option<DeutschebankCustomerPaymentStatusReport>,
    #[serde(rename = "transactionStatus", alias = "status")]
    pub top_level_status: Option<DeutschebankPaymentStatus>,
}

impl DeutschebankStatusResponse {
    pub fn extract_status(&self) -> Option<DeutschebankPaymentStatus> {
        self.customer_payment_status_report
            .as_ref()
            .and_then(|r| r.original_payment_information_and_status.first())
            .and_then(|p| p.transaction_information_and_status.first())
            .and_then(|t| t.transaction_status)
            .or(self.top_level_status)
    }
}

impl TryFrom<&RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>>
    for DeutschebankStatusRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> Result<Self, Self::Error> {
        let connector_payout_id = req.request.connector_payout_id.as_deref().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "connector_payout_id (carrying endToEndId|debtorIban) is required for \
                         Deutsche Bank status enquiry"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?;
        let (end_to_end_id, debtor_iban) = decode_connector_payout_id(connector_payout_id)?;

        Ok(Self {
            debtor_account: DeutschebankStatusDebtorAccount {
                identification: DeutschebankIbanIdentification { iban: debtor_iban },
            },
            end_to_end_identification: end_to_end_id,
        })
    }
}

impl TryFrom<ResponseRouterData<DeutschebankSepaPaymentResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DeutschebankSepaPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_status = item
            .response
            .extract_status()
            .map(PayoutStatus::from)
            .unwrap_or(PayoutStatus::Pending);
        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: None,
                payout_status,
                connector_payout_id: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<DeutschebankStatusResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DeutschebankStatusResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_status = item
            .response
            .extract_status()
            .map(PayoutStatus::from)
            .unwrap_or(PayoutStatus::Pending);
        Ok(Self {
            response: Ok(PayoutGetResponse {
                merchant_payout_id: None,
                payout_status,
                connector_payout_id: item.router_data.request.connector_payout_id.clone(),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== Helpers =====

fn extract_payee_iban(
    payout_method_data: Option<&PayoutMethodData>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    match payout_method_data {
        Some(PayoutMethodData::Bank(Bank::Sepa(SepaBankTransfer { iban, .. }))) => Ok(iban.clone()),
        _ => Err(error_stack::report!(IntegrationError::NotSupported {
            message: "Deutsche Bank only supports SEPA bank payouts".to_string(),
            connector: "Deutschebank",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Provide `payout_method_data.bank.sepa.iban` for the payee account".to_string(),
                ),
                ..Default::default()
            },
        })),
    }
}

fn extract_payee_bic(
    payout_method_data: Option<&PayoutMethodData>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    match payout_method_data {
        Some(PayoutMethodData::Bank(Bank::Sepa(SepaBankTransfer { bic: Some(bic), .. }))) => {
            Ok(bic.clone())
        }
        Some(PayoutMethodData::Bank(Bank::Sepa(_))) => Err(error_stack::report!(
            IntegrationError::MissingRequiredField {
                field_name: "payout_method_data.bank.sepa.bic",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Deutsche Bank SEPA requires the creditor agent BIC".to_string(),
                    ),
                    ..Default::default()
                },
            }
        )),
        _ => Err(error_stack::report!(IntegrationError::NotSupported {
            message: "Deutsche Bank only supports SEPA bank payouts".to_string(),
            connector: "Deutschebank",
            context: Default::default(),
        })),
    }
}

fn extract_debtor_iban(
    source_bank_data: Option<&Bank>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    match source_bank_data {
        Some(Bank::Sepa(SepaBankTransfer { iban, .. })) => Ok(iban.clone()),
        _ => Err(
            error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "source_bank_data.sepa.iban",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Deutsche Bank requires `source_bank_data.sepa.iban` for the debtor account"
                        .to_string(),
                ),
                ..Default::default()
            },
        }),
        ),
    }
}

fn current_iso_utc_seconds() -> String {
    use time::macros::format_description;
    let fmt = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    time::OffsetDateTime::now_utc()
        .format(&fmt)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn current_iso_date() -> String {
    use time::format_description::well_known::Iso8601;
    let date = time::OffsetDateTime::now_utc().date();
    date.format(&Iso8601::DATE)
        .unwrap_or_else(|_| "1970-01-01".to_string())
}
