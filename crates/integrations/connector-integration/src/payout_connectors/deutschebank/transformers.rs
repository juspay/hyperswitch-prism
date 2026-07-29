use super::DeutschebankPayoutsRouterData;
use crate::types::ResponseRouterData;
use common_enums::PayoutStatus;
use common_utils::types::{FloatMajorUnit, FloatMajorUnitForConnector};
use domain_types::{
    connector_flow::{PayoutEligibility, PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
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
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

const DEUTSCHEBANK_BICFI: &str = "DEUTDEDDXXX";
const VOP_ID_SEPARATOR: char = '|';

const SEPA_PAYMENT_METHOD_CODE: &str = "TRF";
const SEPA_SERVICE_LEVEL_CODE: &str = "SEPA";

const SEPA_SINGLE_TRANSACTION_COUNT: &str = "1";

// ===== AUTH TYPE =====

#[derive(Debug, Clone)]
pub struct DeutschebankAuthType {
    pub customer_identifier: Secret<String>,
    pub key_id: Secret<String>,
    pub signing_private_key: Secret<String>,
    pub client_certificate: Secret<String>,
    pub client_certificate_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for DeutschebankAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Deutschebank {
                customer_identifier,
                key_id,
                signing_private_key,
                client_certificate_bundle,
                ..
            } => {
                let (cert, key) = split_pem_bundle(client_certificate_bundle.peek())?;
                Ok(Self {
                    customer_identifier: customer_identifier.to_owned(),
                    key_id: key_id.to_owned(),
                    signing_private_key: signing_private_key.to_owned(),
                    client_certificate: Secret::new(cert),
                    client_certificate_key: key,
                })
            }
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Deutsche Bank CSEAL requires ConnectorSpecificConfig::Deutschebank"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Configure the merchant connector account with the Deutsche Bank \
                             CSEAL fields."
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeutschebankErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub errors: Option<Vec<DeutschebankErrorEntry>>,

    #[serde(flatten)]
    pub additional: std::collections::BTreeMap<String, serde_json::Value>,
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

impl<T: PaymentMethodDataTypes + Debug + Send + Sync + 'static + Serialize>
    TryFrom<
        DeutschebankPayoutsRouterData<
            RouterDataV2<
                PayoutEligibility,
                PayoutFlowData,
                PayoutEligibilityRequest,
                PayoutEligibilityResponse,
            >,
            T,
        >,
    > for DeutschebankVopRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: DeutschebankPayoutsRouterData<
            RouterDataV2<
                PayoutEligibility,
                PayoutFlowData,
                PayoutEligibilityRequest,
                PayoutEligibilityResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data;
        let payee_iban = extract_payee_iban(req.request.payout_method_data.as_ref())?;
        let debtor_iban = extract_debtor_iban(req.request.source_bank_data.as_ref())?;
        let payee_name = extract_customer_name(
            req.request.customer.as_ref(),
            "Payee name is required for Deutsche Bank VoP check",
        )?;

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
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(http_code),
                    additional_context: Some(
                        "Deutsche Bank VoP response missing `payeeNameMatch`".to_string(),
                    ),
                },
            })?;
    let payout_status = PayoutStatus::from(match_status);
    let is_eligible = matches!(
        match_status,
        DeutschebankVopMatchStatus::Mtch | DeutschebankVopMatchStatus::Cmtc
    );

    let connector_payout_id = is_eligible.then_some(vop_id);

    Ok(PayoutEligibilityResponse {
        merchant_payout_id: None,
        payout_status,
        connector_payout_id,
        payout_eligible: Some(is_eligible),
        status_code: http_code,
    })
}

impl TryFrom<ResponseRouterData<DeutschebankVopResponse, Self>>
    for RouterDataV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DeutschebankVopResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let vop_id = derive_vop_id(
            item.router_data
                .resource_common_data
                .merchant_id
                .get_string_repr(),
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        );
        let response = build_eligibility_response(item.response, vop_id, item.http_code)?;
        Ok(Self {
            response: Ok(response),
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
#[serde(rename_all = "camelCase")]
pub struct DeutschebankGroupHeader {
    pub message_identification: String,
    pub creation_date_time: String,
    pub control_sum: FloatMajorUnit,
    pub number_of_transactions: String,
    pub initiating_party: DeutschebankParty,
}

#[derive(Debug, Serialize)]
pub struct DeutschebankParty {
    pub name: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeutschebankPaymentInformation {
    pub payment_information_identification: String,
    pub payment_method: &'static str,
    pub batch_booking: bool,
    pub control_sum: FloatMajorUnit,
    pub number_of_transactions: String,
    pub payment_type_information: DeutschebankPaymentTypeInformation,
    pub requested_execution_date: DeutschebankExecutionDate,
    pub debtor: DeutschebankParty,
    pub debtor_account: DeutschebankAccount,
    pub debtor_agent: DeutschebankAgent,
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
    pub value: FloatMajorUnit,
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
        let creditor_name = extract_customer_name(
            req.request.customer.as_ref(),
            "Creditor name is required for Deutsche Bank SEPA payment",
        )?;

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
        let creation_date_time = current_iso_utc_seconds()?;
        let execution_date = sepa_execution_date()?;

        let amount = utils::convert_amount(
            &FloatMajorUnitForConnector,
            req.request.amount,
            req.request.destination_currency,
        )?;
        let purpose_code = req
            .resource_common_data
            .description
            .clone()
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "description",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Deutsche Bank uses `description` as the SEPA proprietary \
                                 purpose code"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Set `description` on the payout request.".to_string(),
                        ),
                        doc_url: None,
                    },
                })
            })?;

        let request = DeutschebankSepaPaymentRequest {
            customer_credit_transfer_initiation: DeutschebankCustomerCreditTransferInitiation {
                group_header: DeutschebankGroupHeader {
                    message_identification: message_id.clone(),
                    creation_date_time,
                    control_sum: amount,
                    number_of_transactions: SEPA_SINGLE_TRANSACTION_COUNT.to_string(),
                    initiating_party: DeutschebankParty {
                        name: creditor_name.clone(),
                    },
                },
                payment_information: vec![DeutschebankPaymentInformation {
                    payment_information_identification: message_id.clone(),
                    payment_method: SEPA_PAYMENT_METHOD_CODE,
                    batch_booking: false,
                    control_sum: amount,
                    number_of_transactions: SEPA_SINGLE_TRANSACTION_COUNT.to_string(),
                    payment_type_information: DeutschebankPaymentTypeInformation {
                        service_level: DeutschebankCode {
                            code: SEPA_SERVICE_LEVEL_CODE,
                        },
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

impl<T: PaymentMethodDataTypes + Debug + Send + Sync + 'static + Serialize>
    TryFrom<
        DeutschebankPayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            T,
        >,
    > for DeutschebankSepaPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: DeutschebankPayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(DeutschebankSepaPaymentBuilt::try_from(&item.router_data)?.request)
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
                suggested_action: Some(
                    "Pass the exact `connector_payout_id` returned by Transfer; do not modify it."
                        .to_string(),
                ),
                doc_url: None,
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

impl<T: PaymentMethodDataTypes + Debug + Send + Sync + 'static + Serialize>
    TryFrom<
        DeutschebankPayoutsRouterData<
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
            T,
        >,
    > for DeutschebankStatusRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: DeutschebankPayoutsRouterData<
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data;
        let connector_payout_id = req.request.connector_payout_id.as_deref().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "connector_payout_id (carrying endToEndId|debtorIban) is required for \
                         Deutsche Bank status enquiry"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Pass the `connector_payout_id` returned by Transfer.".to_string(),
                    ),
                    doc_url: None,
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
        let built =
            DeutschebankSepaPaymentBuilt::try_from(&item.router_data)
                .change_context(ConnectorError::ResponseDeserializationFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "rebuilding SEPA endToEndId + debtor IBAN to encode connector_payout_id"
                            .to_string(),
                    ),
                },
            })?;
        let compound = encode_connector_payout_id(&built.end_to_end_id, &built.debtor_iban);

        let payout_status = item
            .response
            .extract_status()
            .map(PayoutStatus::from)
            .unwrap_or(PayoutStatus::Pending);
        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: None,
                payout_status,
                connector_payout_id: Some(compound),
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
                suggested_action: Some("Set `payout_method_data.bank.sepa.iban`.".to_string(),),
                doc_url: None,
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
                    suggested_action: Some("Set `payout_method_data.bank.sepa.bic`.".to_string(),),
                    doc_url: None,
                },
            }
        )),
        _ => Err(error_stack::report!(IntegrationError::NotSupported {
            message: "Deutsche Bank only supports SEPA bank payouts".to_string(),
            connector: "Deutschebank",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Provide `payout_method_data.bank.sepa` with iban + bic".to_string(),
                ),
                suggested_action: Some("Use SEPA bank payout method.".to_string(),),
                doc_url: None,
            },
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
                suggested_action: Some(
                    "Set `source_bank_data.sepa.iban`.".to_string(),
                ),
                doc_url: None,
            },
        }),
        ),
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` UTC timestamp used for both the CSEAL
/// `x-apiConsumer-request-timestamp` header and the SEPA `creationDateTime`.
pub(super) fn current_iso_utc_seconds() -> Result<String, error_stack::Report<IntegrationError>> {
    use time::macros::format_description;
    let fmt = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    time::OffsetDateTime::now_utc().format(&fmt).change_context(
        IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                additional_context: Some(
                    "formatting current UTC datetime for Deutsche Bank request".to_string(),
                ),
                suggested_action: Some("Retry the request; report if persistent.".to_string()),
                doc_url: None,
            },
        },
    )
}

fn sepa_execution_date() -> Result<String, error_stack::Report<IntegrationError>> {
    use time::format_description::well_known::Iso8601;
    (time::OffsetDateTime::now_utc().date() + time::Duration::days(1))
        .format(&Iso8601::DATE)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                additional_context: Some(
                    "formatting D+1 date for SEPA requestedExecutionDate".to_string(),
                ),
                suggested_action: Some("Retry the request; report if persistent.".to_string()),
                doc_url: None,
            },
        })
}

// ============================================================================
// PEM bundle handling for mTLS material
// ============================================================================

#[derive(Debug)]
struct PemBlock<'a> {
    label: &'a str,
    body: &'a str,
}

const PRIVATE_KEY_LABELS: &[&str] = &["PRIVATE KEY", "RSA PRIVATE KEY", "EC PRIVATE KEY"];
const CERTIFICATE_LABEL: &str = "CERTIFICATE";

fn parse_pem_blocks(input: &str) -> Vec<PemBlock<'_>> {
    const BEGIN_PREFIX: &str = "-----BEGIN ";
    const MARKER_SUFFIX: &str = "-----";

    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(begin_rel) = input[cursor..].find(BEGIN_PREFIX) {
        let block_start = cursor + begin_rel;
        let label_start = block_start + BEGIN_PREFIX.len();
        let Some(label_end_rel) = input[label_start..].find(MARKER_SUFFIX) else {
            break;
        };
        let label_end = label_start + label_end_rel;
        let label = &input[label_start..label_end];

        let end_marker = format!("-----END {label}-----");
        let Some(end_rel) = input[label_end..].find(end_marker.as_str()) else {
            break;
        };
        let block_end = label_end + end_rel + end_marker.len();

        blocks.push(PemBlock {
            label,
            body: &input[block_start..block_end],
        });
        cursor = block_end;
    }
    blocks
}

pub(super) fn split_pem_bundle(
    bundle: &str,
) -> Result<(String, Secret<String>), error_stack::Report<IntegrationError>> {
    let blocks = parse_pem_blocks(bundle);

    let certs: Vec<&str> = blocks
        .iter()
        .filter(|b| b.label == CERTIFICATE_LABEL)
        .map(|b| b.body)
        .collect();
    let keys: Vec<&str> = blocks
        .iter()
        .filter(|b| PRIVATE_KEY_LABELS.contains(&b.label))
        .map(|b| b.body)
        .collect();

    let problem = match (certs.is_empty(), keys.len()) {
        (false, 1) => None,
        (true, _) => Some("missing CERTIFICATE block".to_string()),
        (_, 0) => Some("missing PRIVATE KEY block".to_string()),
        (_, n) => Some(format!(
            "found {n} private-key blocks; exactly one is required"
        )),
    };
    if let Some(detail) = problem {
        return Err(error_stack::report!(
            IntegrationError::InvalidConnectorConfig {
                config: "client_certificate_bundle",
                context: IntegrationErrorContext {
                    additional_context: Some(detail),
                    suggested_action: Some(
                        "Concatenate the PEM certificate (chain) and its single PEM private \
                         key into `client_certificate_bundle`."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            }
        ));
    }

    let cert_chain = format!("{}\n", certs.join("\n"));
    let key_pem = keys.first().map(|k| format!("{k}\n")).unwrap_or_default();
    Ok((cert_chain, Secret::new(key_pem)))
}

fn extract_customer_name(
    customer: Option<&domain_types::payouts::payouts_types::PayoutCustomer>,
    purpose_description: &'static str,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    customer
        .and_then(|c| c.name.as_ref())
        .map(|n| Secret::new(n.clone()))
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "customer.name",
                context: IntegrationErrorContext {
                    additional_context: Some(purpose_description.to_string()),
                    suggested_action: Some(
                        "Set `customer.name` on the payout request.".to_string()
                    ),
                    doc_url: None,
                },
            })
        })
}
